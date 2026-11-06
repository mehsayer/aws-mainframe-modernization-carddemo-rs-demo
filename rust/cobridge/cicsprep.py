#!/usr/bin/env python3
"""CICS translator and build for running the ORIGINAL CardDemo online programs with GnuCOBOL.

Like IBM's CICS translator, it replaces every `EXEC CICS ... END-EXEC` with a CALL. The call goes through
a procedure pointer the bridge stores in the EIB (EIB-CDCICS), so the Rust runtime needs no exported
symbols. The original sources are not modified; the output goes to rust/cobridge/build/.

Call layout:  CALL EIB-CDCICS USING DFHEIBLK
                   BY CONTENT 'Dn' 'descriptor piece 1' ... 'descriptor piece n'
                   <argument 1> <argument 2> ...
The descriptor is the command and its options in source order, e.g. `READ DATASET=1 RIDFLD=2 INTO=3
LENGTH=4 KEYLENGTH=5 UPDATE RESP`; `NAME=k` means the option's value is argument k (1-based after the
descriptor). Data areas are passed BY REFERENCE, literals and LENGTH OF ... BY CONTENT.
After the call:  IF RETURN-CODE NOT = 0 GOBACK END-IF      (XCTL, RETURN, ABEND, abend conditions)
                 MOVE EIBRESP TO <RESP>  /  MOVE EIBRESP2 TO <RESP2>

  python3 rust/cobridge/cicsprep.py          # translate + compile everything into build/modules
"""
import os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
APP = ROOT / "cobol" / "app"
BUILD = HERE / "build"
SRC_OUT, CPY_OUT = BUILD / "src", BUILD / "cpy"


def modules_dir():
    """Where the compiled modules go and where the runtime loads them from (same rule as
    cobridge::modules_dir): CARDDEMO_COBOL_MODULES, else rust/cobridge/build/modules."""
    return Path(os.environ.get("CARDDEMO_COBOL_MODULES") or BUILD / "modules")


MOD_OUT = modules_dir()
COBC = ["cobc", "-std=ibm", "-fsign=EBCDIC", "-I", str(CPY_OUT)]

# CICS response codes (DFHRESP)
RESP = {"NORMAL": 0, "ERROR": 1, "RDATT": 2, "WRBRK": 3, "EOF": 4, "EODS": 5, "EOC": 6, "INBFMH": 7,
        "ENDINPT": 8, "NONVAL": 9, "NOSTART": 10, "TERMIDERR": 11, "FILENOTFOUND": 12, "NOTFND": 13,
        "DUPREC": 14, "DUPKEY": 15, "INVREQ": 16, "IOERR": 17, "NOSPACE": 18, "NOTOPEN": 19,
        "ENDFILE": 20, "ILLOGIC": 21, "LENGERR": 22, "QZERO": 23, "SIGNAL": 24, "QBUSY": 25,
        "ITEMERR": 26, "PGMIDERR": 27, "TRANSIDERR": 28, "ENDDATA": 29, "INVTSREQ": 30, "EXPIRED": 31,
        "RETPAGE": 32, "RTEFAIL": 33, "RTESOME": 34, "TSIOERR": 35, "MAPFAIL": 36, "INVERRTERM": 37,
        "INVMPSZ": 38, "IGREQID": 39, "OVERFLOW": 40, "INVLDC": 41, "NOSTG": 42, "JIDERR": 43,
        "QIDERR": 44, "NOJBUFSP": 45, "DSSTAT": 46, "SELNERR": 47, "FUNCERR": 48, "UNEXPIN": 49,
        "NOPASSBKRD": 50, "NOPASSBKWR": 51, "SYSIDERR": 53, "ISCINVREQ": 54, "ENQBUSY": 55,
        "ENVDEFERR": 56, "IGREQCD": 57, "SESSIONERR": 58, "SYSBUSY": 59, "SESSBUSY": 60,
        "NOTALLOC": 61, "CBIDERR": 62, "INVEXITREQ": 63, "INVPARTNSET": 64, "INVPARTN": 65,
        "PARTNFAIL": 66, "USERIDERR": 69, "NOTAUTH": 70, "VOLIDERR": 71, "SUPPRESSED": 72,
        "RESIDERR": 75, "NOSPOOL": 80, "TERMERR": 81, "ROLLEDBACK": 82, "END": 83, "DISABLED": 84,
        "ALLOCERR": 85, "STRELERR": 86, "OPENERR": 87, "SPOLBUSY": 88, "SPOLERR": 89, "NODEIDERR": 90,
        "TASKIDERR": 91, "TCIDERR": 92, "DSNNOTFOUND": 93, "LOADING": 94, "MODELIDERR": 95,
        "OUTDESCRERR": 96, "PARTNERIDERR": 97, "PROFILEIDERR": 98, "NETNAMEIDERR": 99,
        "LOCKED": 100, "RECORDBUSY": 101, "UOWNOTFOUND": 102, "UOWLNOTFOUND": 103}

# options whose value is a paragraph name (not passed as an argument)
LABEL_OPTS = {"LABEL"} | set(RESP)


def card_lines(path):
    return path.read_text(encoding="latin-1").expandtabs(4).splitlines()


def is_comment(l):
    return len(l) > 6 and l[6] in "*/"


def tokenize(text):
    """Split the inside of EXEC CICS ... END-EXEC into words and NAME(arg) pairs (balanced parentheses)."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text[i].isspace() or text[i] == ",":
            i += 1
            continue
        m = re.match(r"[A-Za-z0-9-]+", text[i:])
        if not m:
            raise ValueError(f"cannot parse EXEC CICS near: {text[i:i + 40]!r}")
        word = m.group(0).upper()
        i += len(word)
        while i < n and text[i] == " ":
            i += 1
        if i < n and text[i] == "(":
            depth, j, q = 0, i, None
            while j < n:
                c = text[j]
                if q:
                    if c == q:
                        q = None
                elif c in "'\"":
                    q = c
                elif c == "(":
                    depth += 1
                elif c == ")":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            out.append((word, " ".join(text[i + 1:j].split())))
            i = j + 1
        else:
            out.append((word, None))
    return out


def arg_phrase(v):
    """How to pass an option value."""
    if re.fullmatch(r"'[^']*'|\"[^\"]*\"", v) or re.fullmatch(r"[+-]?\d+", v):
        return "BY CONTENT " + v
    if v.upper().startswith("LENGTH OF "):
        return "BY CONTENT " + v
    return "BY REFERENCE " + v


def translate_exec(text, labels):
    """COBOL statements for one EXEC CICS block (the text between EXEC CICS and END-EXEC).

    `labels` lists the program's HANDLE CONDITION / HANDLE ABEND targets: label k is reached when the
    bridge returns RETURN-CODE 1 + k (CICS branches there instead of returning to the statement)."""
    toks = tokenize(text)
    cmd = toks[0][0]
    # RECEIVE MAP without INTO / SEND MAP without FROM use the program's symbolic map (map name + I / O)
    opts = {k: v for k, v in toks[1:]}
    if cmd in ("RECEIVE", "SEND") and "MAP" in opts and not ({"INTO", "SET", "FROM", "MAPONLY"} & set(opts)):
        m = opts["MAP"]
        if not re.fullmatch(r"'[A-Z0-9#@$]+'", m):
            raise ValueError(f"MAP({m}) without INTO/FROM: map name must be a literal")
        toks.append(("INTO" if cmd == "RECEIVE" else "FROM", m.strip("'") + ("I" if cmd == "RECEIVE" else "O")))
    desc, args, after = [cmd], [], []
    for name, val in toks[1:]:
        if name in ("RESP", "RESP2"):
            desc.append(name)
            after.append(f"MOVE {'EIBRESP' if name == 'RESP' else 'EIBRESP2'} TO {val}")
        elif val is None:
            desc.append(name)
        elif name in LABEL_OPTS:
            desc.append(f"{name}:{1 + labels.index(val)}")
        else:
            args.append(arg_phrase(val))
            desc.append(f"{name}={len(args)}")
    d = " ".join(desc)
    pieces = [d[i:i + 40] for i in range(0, len(d), 40)]
    stmts = ["CALL EIB-CDCICS USING DFHEIBLK",
             f"BY CONTENT 'D{len(pieces)}'"] + [f"'{p}'" for p in pieces] + args + ["END-CALL"]
    stmts += [f"IF RETURN-CODE = {1 + k} GO TO {lab} END-IF" for k, lab in enumerate(labels)]
    stmts += ["IF RETURN-CODE NOT = 0 GOBACK END-IF"] + after
    return stmts


def emit(stmts, indent=12):
    """Lay statements out in area B (columns 12-72), breaking long phrases at blanks."""
    out = []
    for s in stmts:
        line = " " * indent + s
        while len(line) > 72:
            cut = line.rfind(" ", indent + 4, 72)
            if cut <= indent:
                raise ValueError(f"cannot fit: {s}")
            out.append(line[:cut])
            line = " " * (indent + 4) + line[cut + 1:]
        out.append(line)
    return out


def translate(src: Path) -> str:
    lines = card_lines(src)
    out, block, block_indent, in_exec = [], [], 12, False
    labels = []
    for m in re.finditer(r"HANDLE\s+(?:CONDITION|ABEND)\b(.*?)END-EXEC", " ".join(l[7:72] for l in lines if not is_comment(l)), re.S):
        for name, val in tokenize(m.group(1)):
            if name in LABEL_OPTS and val and val not in labels:
                labels.append(val)
    has_commarea = any(re.search(r"\b01\s+DFHCOMMAREA\b", l[7:72]) for l in lines if not is_comment(l))
    for l in lines:
        if is_comment(l) or len(l) <= 6:
            out.append(l)
            continue
        code = l[7:72] if len(l) > 7 else ""
        if not in_exec:
            m = re.search(r"\bEXEC\s+CICS\b", code, re.I)
            if m:
                in_exec = True
                block_indent = 7 + m.start()
                head = code[:m.start()]
                if head.strip():
                    out.append(l[:7] + head.rstrip())
                block = [code[m.end():]]
                code = ""
            else:
                code = re.sub(r"DFHRESP\s*\(\s*([A-Z0-9]+)\s*\)", lambda m: str(RESP[m.group(1)]), code)
                if re.search(r"^\s*PROCEDURE\s+DIVISION\b", code) and "USING" not in code:
                    code = code.replace("PROCEDURE DIVISION", "PROCEDURE DIVISION USING DFHEIBLK DFHCOMMAREA", 1)
                elif re.search(r"^\s*PROCEDURE\s+DIVISION\s+USING\s+DFHCOMMAREA", code):
                    code = code.replace("USING DFHCOMMAREA", "USING DFHEIBLK DFHCOMMAREA", 1)
                if re.match(r"^\s*LINKAGE\s+SECTION\.", code):
                    out.append(l[:7] + code.rstrip())
                    out.append("       COPY DFHEIBLK.")
                    if not has_commarea:
                        out.append("       01  DFHCOMMAREA PIC X.")
                    continue
                if re.match(r"^\s*PROCEDURE\s+DIVISION", code) and not any("COPY DFHEIBLK" in o for o in out):
                    out.append("       LINKAGE SECTION.")
                    out.append("       COPY DFHEIBLK.")
                    if not has_commarea:
                        out.append("       01  DFHCOMMAREA PIC X.")
                out.append((l[:7] + code).rstrip())
                continue
        else:
            block.append(code)
        joined = " ".join(block)
        e = re.search(r"\bEND-EXEC\b", joined, re.I)
        if in_exec and e:
            in_exec = False
            tail = joined[e.end():].strip()
            out += emit(translate_exec(joined[:e.start()], labels), max(12, block_indent))
            if tail:
                out.append(" " * max(12, block_indent) + tail)
    if in_exec:
        raise ValueError(f"{src.name}: unterminated EXEC CICS")
    return "\n".join(out) + "\n"


def online_programs():
    return sorted(p for p in (APP / "cbl").glob("CO*.cbl") if p.stem.endswith("C") and p.stem != "COBSWAIT")


def build():
    for d in (SRC_OUT, CPY_OUT, MOD_OUT):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    for d in ("cpy", "cpy-bms"):
        for f in (APP / d).iterdir():
            (CPY_OUT / f.name).write_text(f.read_text(encoding="latin-1").expandtabs(4), encoding="latin-1")
    for f in (HERE / "copy").iterdir():
        shutil.copy(f, CPY_OUT / f.name)
    errors = 0
    for p in online_programs():
        out = SRC_OUT / f"{p.stem}.cbl"
        out.write_text(translate(p), encoding="latin-1")
        r = subprocess.run(COBC + ["-m", str(out), "-o", str(MOD_OUT / f"{p.stem}.so")], capture_output=True, text=True)
        if r.returncode:
            errors += 1
            sys.stderr.write(f"--- {p.stem}\n{r.stderr}")
    # CSUTLDTC (date check, no CICS) from the original source; CEEDAYS (Language Environment) in C
    r = subprocess.run(COBC + ["-m", str(APP / "cbl" / "CSUTLDTC.cbl"), "-o", str(MOD_OUT / "CSUTLDTC.so")],
                       capture_output=True, text=True)
    errors += bool(r.returncode)
    sys.stderr.write(r.stderr)
    r = subprocess.run(["cobc", "-m", str(HERE / "ceedays.c"), "-o", str(MOD_OUT / "CEEDAYS.so")], capture_output=True, text=True)
    errors += bool(r.returncode)
    sys.stderr.write(r.stderr)
    n = len(list(MOD_OUT.glob("*.so")))
    print(f"{n} modules in {MOD_OUT.relative_to(ROOT)}" + (f", {errors} failed" if errors else ""))
    return errors == 0


if __name__ == "__main__":
    sys.exit(0 if build() else 1)
