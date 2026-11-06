#!/usr/bin/env python3
"""Differential test: the original COBOL batch programs (GnuCOBOL) against the Rust port.

For each program the Rust job runs first; the data sets it saw as input are snapshotted
(`carddemo unload`) before the job and its outputs after it. The COBOL program then runs
on the same input snapshot, and its outputs are compared with the Rust outputs.

  python3 difftest/run.py            # build + run all, report in difftest/out/report.md

Requires GnuCOBOL 3.x (cobc) with an indexed-file handler, and `cargo build --release`.
"""
import difflib, os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RS = HERE.parent
APP = RS.parent / "cobol" / "app"
SEED = APP / "data" / "EBCDIC"
BIN = RS / "target" / "release" / "carddemo"
OUT = HERE / "out"
BUILD = HERE / "build"
NOW_RUST = "2022-07-01T12:00:00"
NOW_COB = "2022/07/01 12:00:00"
INTCALC_PARM = "2022071800"
REPORT = ("2022-01-01", "2022-07-06")
# copybooks are compiled from a copy with tabs expanded (CUSTREC/CSLKPCDY contain tabs at width 4)
COBC = ["cobc", "-std=ibm", "-fsign=EBCDIC", "-fstatic-call", "-I", str(BUILD / "cpy")]

# data set -> (record length, key offset, key length, [alternate keys (off, len)])
DS = {
    "ACCTDATA": (300, 0, 11, []),
    "CARDXREF": (50, 0, 16, [(25, 11)]),
    "CARDDATA": (150, 0, 16, []),
    "CUSTDATA": (500, 0, 9, []),
    "TRANSACT": (350, 0, 16, []),
    "TCATBALF": (50, 0, 17, []),
    "DISCGRP": (50, 0, 16, []),
    "TRANTYPE": (60, 0, 2, []),
    "TRANCATG": (60, 0, 6, []),
    "TRXFL": (350, 0, 32, []),
    "EXPORT": (500, 27, 4, []),
}

# --------------------------------------------------------------------------- helpers

def a2e_inverse():
    tbl = eval(open(RS / "crates" / "vsam" / "src" / "a2e.in").read())
    inv = [0] * 256
    for a, e in enumerate(tbl):
        inv[e] = a
    return bytes(inv)

E2A = a2e_inverse()

def sh(cmd, cwd=None, env=None, check=True):
    r = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True)
    if check and r.returncode not in (0, 4):
        sys.stderr.write(r.stdout.decode(errors="replace") + r.stderr.decode(errors="replace"))
        raise SystemExit(f"failed ({r.returncode}): {' '.join(map(str, cmd))}")
    return r

def records(path, n):
    b = Path(path).read_bytes() if Path(path).exists() else b""
    return [b[i:i + n] for i in range(0, len(b), n)]

def lines_of(path, n=None):
    """Report files: the COBOL side writes fixed n-byte records, the Rust side lines."""
    b = Path(path).read_bytes() if Path(path).exists() else b""
    if n and b"\n" not in b:
        ls = [b[i:i + n] for i in range(0, len(b), n)]
    else:
        ls = b.split(b"\n")
        if ls and ls[-1] == b"":
            ls.pop()
    return [l.rstrip(b" ").decode("latin-1") for l in ls]

# Known, accepted platform differences, normalised before comparing:
#  * COB_CURRENT_DATE fixes date and time but GnuCOBOL still returns the real hundredths;
#    the hundredths of timestamps at the run time are masked on both sides.
#  * GnuCOBOL DISPLAYs a signed zoned field as digits + separate sign ("00000002043+");
#    IBM Enterprise COBOL shows the stored bytes, i.e. the sign overpunched ("0000000204C").
#  * SYSOUT is a print data set: X'00' bytes in DISPLAYed records print as blanks (the Rust
#    runtime writes them as spaces); trailing blanks are not significant.
RUN_TS = re.compile(rb"(2022-07-01[- ]12[.:]00[.:]00\.)\d{6}")
OVERPUNCH = {"+": "{ABCDEFGHI", "-": "}JKLMNOPQR"}

def norm_ts(b):
    return RUN_TS.sub(rb"\g<1>000000", b)

def ibm_display(line):
    return re.sub(r"(\d+)([+-])(?![\d.])", lambda m: m.group(1)[:-1] + OVERPUNCH[m.group(2)][int(m.group(1)[-1])], line)

# --------------------------------------------------------------------------- COBOL build

LOADER = """       IDENTIFICATION DIVISION.
       PROGRAM-ID. {prog}.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SEQ-FILE ASSIGN TO SEQFILE
                  ORGANIZATION IS SEQUENTIAL.
           SELECT KSDS-FILE ASSIGN TO KSDSFILE
                  ORGANIZATION IS INDEXED
                  ACCESS MODE IS {access}
                  RECORD KEY IS K-KEY{alt}
                  FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD  SEQ-FILE.
       01  S-REC PIC X({n}).
       FD  KSDS-FILE.
       01  K-REC.
{layout}
       WORKING-STORAGE SECTION.
       01  WS-ST  PIC XX.
       01  WS-EOF PIC X VALUE 'N'.
       01  WS-N   PIC 9(7) VALUE 0.
       PROCEDURE DIVISION.
{body}
           DISPLAY '{prog} ' WS-N ' RECORDS'.
           STOP RUN.
"""

LOAD_BODY = """           OPEN INPUT SEQ-FILE OUTPUT KSDS-FILE.
           PERFORM UNTIL WS-EOF = 'Y'
             READ SEQ-FILE AT END MOVE 'Y' TO WS-EOF
               NOT AT END
                 IF S-REC NOT = SPACES
                   MOVE S-REC TO K-REC
                   WRITE K-REC
                   IF WS-ST NOT = '00' AND WS-ST NOT = '02'
                     DISPLAY 'WRITE STATUS ' WS-ST ' ' S-REC(1:32)
                     MOVE 12 TO RETURN-CODE
                   ELSE ADD 1 TO WS-N END-IF
                 END-IF
             END-READ
           END-PERFORM.
           CLOSE SEQ-FILE KSDS-FILE."""

UNLOAD_BODY = """           OPEN INPUT KSDS-FILE OUTPUT SEQ-FILE.
           IF WS-ST NOT = '00' AND WS-ST NOT = '05'
             DISPLAY 'OPEN STATUS ' WS-ST STOP RUN END-IF.
           PERFORM UNTIL WS-EOF = 'Y'
             READ KSDS-FILE NEXT AT END MOVE 'Y' TO WS-EOF
               NOT AT END
                 MOVE K-REC TO S-REC
                 WRITE S-REC
                 ADD 1 TO WS-N
             END-READ
           END-PERFORM.
           CLOSE SEQ-FILE KSDS-FILE."""


def gen_util(name, kind):
    n, ko, kl, alts = DS[name]
    # lay out the key fields; key and alternate keys never overlap here
    fields, pos, i = [], 0, 0
    keys = sorted([(ko, kl, "K-KEY")] + [(o, l, f"K-ALT{j}") for j, (o, l) in enumerate(alts)])
    for o, l, f in keys:
        if o > pos:
            fields.append(f"           05  FILLER PIC X({o - pos}).")
        fields.append(f"           05  {f} PIC X({l}).")
        pos = o + l
    if pos < n:
        fields.append(f"           05  FILLER PIC X({n - pos}).")
    alt = "".join(f"\n                  ALTERNATE RECORD KEY IS K-ALT{j} WITH DUPLICATES" for j in range(len(alts)))
    prog = f"{kind}{name}"[:30]
    src = LOADER.format(prog=prog, access="SEQUENTIAL" if kind == "UNLD" else "RANDOM", alt=alt, n=n,
                        layout="\n".join(fields), body=LOAD_BODY if kind == "LOAD" else UNLOAD_BODY)
    p = BUILD / f"{prog}.cbl"
    p.write_text(src)
    return prog, p


STUB_C = r"""
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <libcob.h>
/* Language Environment CEE3ABD (abend): report and end the step with a non-zero code. */
int CEE3ABD(unsigned char *abcode, unsigned char *timing) {
    int code = 999;
    if (abcode) code = (abcode[0] << 24 | abcode[1] << 16 | abcode[2] << 8 | abcode[3]);
    printf("CEE3ABD ABEND U%04d\n", code);
    fflush(stdout);
    cob_stop_run(code > 255 ? 255 : code);
    return 0;
}

/* COBDATFT, translated instruction by instruction from cobol/app/asm/COBDATFT.asm.
   Layout (cobol/app/maclib/COCDATFT.mac): type CL1, input CL20, out type CL1, output CL20, msg CL38. */
int COBDATFT(unsigned char *r) {
    unsigned char *intype = r, *inp = r + 1, *outype = r + 21, *out = r + 22, *msg = r + 42;
    if (*intype == '1') {
        if (inp[4] == '-' || *outype == '2') goto err;
        memcpy(out, inp, 4); out[4] = '-'; memcpy(out + 5, inp + 4, 2); out[7] = '-'; memcpy(out + 8, inp + 6, 2);
        return 0;
    }
    if (*intype == '2') {
        if (*outype == '1') goto err;
        memcpy(out, inp, 4); memcpy(out + 4, inp + 5, 2); memcpy(out + 6, inp + 8, 2);
        return 0;
    }
err:
    memcpy(msg, "INVALID INPUT", 13);
    return 0;
}
"""

INTCALC_MAIN = """       IDENTIFICATION DIVISION.
       PROGRAM-ID. RUNACT04.
      * JCL EXEC PGM=CBACT04C,PARM='...': halfword length + PARM text
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  EXTERNAL-PARMS.
           05  PARM-LENGTH         PIC S9(04) COMP VALUE 10.
           05  PARM-DATE           PIC X(10).
       PROCEDURE DIVISION.
           ACCEPT PARM-DATE FROM COMMAND-LINE.
           CALL 'CBACT04C' USING EXTERNAL-PARMS.
           STOP RUN.
"""


def patch_cbstm03a():
    """CBSTM03A walks z/OS control blocks (PSA -> TCB -> TIOT) to list the DD names;
    that has no equivalent outside MVS, so those statements are commented out."""
    src = (APP / "cbl" / "CBSTM03A.CBL").read_text().splitlines()
    out, skip = [], False
    for l in src:
        code = l[6:72] if len(l) > 6 else ""
        if "SET ADDRESS OF PSA-BLOCK" in code:
            skip = True
        if skip and "OPEN OUTPUT STMT-FILE" in code:
            skip = False
        out.append((l[:6] + "*" + l[7:]) if skip and len(l) > 6 else l)
    p = BUILD / "CBSTM03A.cbl"
    p.write_text("\n".join(out) + "\n")
    return p


def patch_export(name):
    """KD-12: CBEXPORT/CBIMPORT name EXPORT-SEQUENCE-NUM (a WORKING-STORAGE field of CVEXPORT) as the
    RECORD KEY of the export file. The key must lie in the file's own record, so the programs do not
    compile as shipped. Patch: the FD record becomes a group with the key at the same offset (27, 4 bytes)."""
    src = (APP / "cbl" / f"{name}.cbl").read_text()
    rec = "EXPORT-OUTPUT-RECORD" if name == "CBEXPORT" else "EXPORT-INPUT-RECORD"
    old = re.search(rf"       01  {rec} +PIC X\(500\)\.", src).group(0)
    src = src.replace(old, f"       01  {rec}.\n           05  FILLER PIC X(27).\n"
                           f"           05  EXP-FD-SEQ-NUM PIC 9(9) COMP.\n           05  FILLER PIC X(469).")
    src = src.replace("RECORD KEY IS EXPORT-SEQUENCE-NUM", "RECORD KEY IS EXP-FD-SEQ-NUM")
    p = BUILD / f"{name}.cbl"
    p.write_text(src)
    return p


def build():
    sh(["cargo", "build", "--release", "-q", "-p", "carddemo"], cwd=RS)
    BUILD.mkdir(parents=True, exist_ok=True)
    (BUILD / "cpy").mkdir(exist_ok=True)
    for f in (APP / "cpy").iterdir():
        (BUILD / "cpy" / f.name).write_text(f.read_text().expandtabs(4))
    stub = BUILD / "stubs.c"
    stub.write_text(STUB_C)
    sh(["cobc", "-c", "-o", str(BUILD / "stubs.o"), str(stub)])
    for name in DS:
        for kind in ("LOAD", "UNLD"):
            prog, p = gen_util(name, kind)
            sh(["cobc", "-x", "-o", str(BUILD / prog), str(p)])
    cbl = APP / "cbl"
    stubo = str(BUILD / "stubs.o")
    sh(COBC + ["-x", "-o", str(BUILD / "CBTRN02C"), str(cbl / "CBTRN02C.cbl"), stubo])
    sh(COBC + ["-x", "-o", str(BUILD / "CBTRN03C"), str(cbl / "CBTRN03C.cbl"), stubo])
    for p in ("CBACT01C", "CBACT02C", "CBACT03C", "CBCUS01C", "CBTRN01C"):
        sh(COBC + ["-x", "-o", str(BUILD / p), str(cbl / f"{p}.cbl"), stubo])
    (BUILD / "RUNACT04.cbl").write_text(INTCALC_MAIN)
    sh(COBC + ["-x", "-o", str(BUILD / "CBACT04C"), str(BUILD / "RUNACT04.cbl"), str(cbl / "CBACT04C.cbl"), stubo])
    for p in ("CBEXPORT", "CBIMPORT"):
        sh(COBC + ["-x", "-o", str(BUILD / p), str(patch_export(p)), stubo])
    sh(COBC + ["-x", "-o", str(BUILD / "CBSTM03A"), str(patch_cbstm03a()), str(cbl / "CBSTM03B.CBL"), stubo])

# --------------------------------------------------------------------------- run

def rust(home, *args):
    env = dict(os.environ, CARDDEMO_HOME=str(home), CARDDEMO_SEED=str(SEED), CARDDEMO_NOW=NOW_RUST)
    return sh([str(BIN), *args], env=env).stdout.decode()

def rust_sysout(text):
    """Program DISPLAY output from `carddemo job` (drop the job runner's own lines)."""
    keep = []
    for l in text.splitlines():
        if re.match(r"^\S+\s+STEP\S*\s+\S+\s+RC=", l) or re.search(r"MAXCC=", l) or re.match(r"^STEP\S+ (IDCAMS|SORT|IEFBR14):", l):
            continue
        keep.append(norm_ts(l.rstrip().encode("latin-1")).decode("latin-1"))
    return keep

def cob_env(work, dds):
    # COB_VARSEQ_FORMAT=3: variable records carry a 2-byte big-endian length (as the Rust port writes them)
    env = dict(os.environ, COB_CURRENT_DATE=NOW_COB, COB_FILE_PATH="", COB_VARSEQ_FORMAT="3")
    for dd, path in dds.items():
        env[f"DD_{dd}"] = str(path)
    return env

def cob_load(work, name, src):
    dst = work / f"{name}.idx"
    r = sh([str(BUILD / f"LOAD{name}")], env=cob_env(work, {"SEQFILE": src, "KSDSFILE": dst}))
    if r.returncode:
        raise SystemExit(r.stdout.decode())
    return dst

def cob_unload(work, name, idx):
    dst = work / f"{name}.after.dat"
    sh([str(BUILD / f"UNLD{name}")], env=cob_env(work, {"SEQFILE": dst, "KSDSFILE": idx}))
    return dst

def cob_run(work, prog, dds, args=()):
    r = subprocess.run([str(BUILD / prog), *args], env=cob_env(work, dds), capture_output=True, cwd=work)
    out = norm_ts(r.stdout).replace(b"\x00", b" ").decode("latin-1").splitlines()
    return r.returncode, [ibm_display(l.rstrip()) for l in out], r.stderr.decode("latin-1")

class Report:
    def __init__(self):
        self.rows, self.details, self.notes = [], [], []

    def cmp_records(self, test, what, rust_path, cob_path, n, accept=None, mask=()):
        """accept = (function on a COBOL record, note): an accepted GnuCOBOL-vs-IBM difference."""
        a, b = [norm_ts(r) for r in records(rust_path, n)], [norm_ts(r) for r in records(cob_path, n)]
        for lo, hi in mask:  # byte ranges excluded from the comparison (both sides)
            a = [r[:lo] + b"~" * (hi - lo) + r[hi:] for r in a]
            b = [r[:lo] + b"~" * (hi - lo) + r[hi:] for r in b]
        verdict = "identical"
        if accept:
            fixed = [accept[0](r) for r in b]
            if fixed != b:
                b = fixed
                self.notes.append(f"{test}, {what}: {accept[1]}")
                verdict = f"identical (note {len(self.notes)})"
        diff = [i for i in range(max(len(a), len(b))) if (a[i] if i < len(a) else None) != (b[i] if i < len(b) else None)]
        self.rows.append((test, what, f"{len(a)} / {len(b)} records", verdict if not diff else f"**{len(diff)} differ**"))
        if diff:
            d = [f"### {test}: {what}", "```"]
            for i in diff[:5]:
                ra = a[i].decode("latin-1") if i < len(a) else "<missing>"
                rb = b[i].decode("latin-1") if i < len(b) else "<missing>"
                d += [f"record {i + 1}", f"  rust : {ra.rstrip()}", f"  cobol: {rb.rstrip()}"]
            d.append("```")
            self.details.append("\n".join(d))
        return not diff

    def cmp_lines(self, test, what, a, b):
        same = a == b
        self.rows.append((test, what, f"{len(a)} / {len(b)} lines", "identical" if same else "**differs**"))
        if not same:
            ud = list(difflib.unified_diff(b, a, "cobol", "rust", lineterm="", n=1))
            self.details.append(f"### {test}: {what}\n```\n" + "\n".join(ud[:60]) + "\n```")
        return same

    def cmp_value(self, test, what, a, b):
        self.rows.append((test, what, f"{a} / {b}", "identical" if a == b else "**differs**"))
        return a == b

    def write(self, path):
        ok = all(r[3].startswith("identical") for r in self.rows)
        md = ["# Differential test: COBOL (GnuCOBOL) vs Rust", "",
              f"Run date {NOW_RUST} (both sides), INTCALC PARM {INTCALC_PARM}, TRANREPT {REPORT[0]}..{REPORT[1]}.", "",
              "| Job / program | Output | Rust / COBOL | Result |", "|---|---|---|---|"]
        md += [f"| {a} | {b} | {c} | {d} |" for a, b, c, d in self.rows]
        md += [""] + [f"Note {i}: {n}" for i, n in enumerate(self.notes, 1)]
        md += ["", f"**Overall: {'all outputs identical' if ok else 'differences found'}**", ""] + self.details
        path.write_text("\n".join(md) + "\n")
        return ok


def main():
    if "--no-build" not in sys.argv:
        print("building COBOL programs ...")
        build()
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    rh = OUT / "rust-home"
    ds = rh / "datasets"
    hlq = "AWS.M2.CARDDEMO"
    rep = Report()

    def snap(tag):
        rust(rh, "unload", str(OUT / "snap" / tag))
        return OUT / "snap" / tag

    def seq_from_seed(name, n, dst):
        b = (SEED / name).read_bytes().translate(E2A)
        dst.write_bytes(b)
        return dst

    rust(rh, "reset")
    seed = snap("seed")

    # ---------------- readers: CBACT01C..03C, CBCUS01C, CBTRN01C (no updates)
    w = OUT / "cob-readers"; w.mkdir()
    idx = {n: cob_load(w, n, seed / f"{n}.dat") for n in ("ACCTDATA", "CARDDATA", "CARDXREF", "CUSTDATA", "TRANSACT")}
    rs = rust(rh, "job", "READACCT")
    rc, out, err = cob_run(w, "CBACT01C", {"ACCTFILE": idx["ACCTDATA"], "OUTFILE": w / "PSCOMP.dat",
                                          "ARRYFILE": w / "ARRYPS.dat", "VBRCFILE": w / "VBPS.dat"})
    t = "READACCT / CBACT01C"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_records(t, "OUTFILE (with COMP-3)", ds / f"{hlq}.ACCTDATA.PSCOMP", w / "PSCOMP.dat", 107)
    def initialize_sign(r):
        # slots 4-5 are only set by INITIALIZE ARR-ARRAY-REC: GnuCOBOL stores unsigned zeros there,
        # IBM INITIALIZE (= MOVE ZERO) a positive sign, X'C0' -> '{'
        r = bytearray(r)
        for o in (79, 98):
            if r[o:o + 1] == b"0":
                r[o] = ord("{")
        return bytes(r)
    rep.cmp_records(t, "ARRYFILE (OCCURS, COMP-3)", ds / f"{hlq}.ACCTDATA.ARRYPS", w / "ARRYPS.dat", 110,
                    (initialize_sign, "GnuCOBOL INITIALIZE leaves signed DISPLAY zeros unsigned ('0'); IBM INITIALIZE "
                     "acts as MOVE ZERO and sets the positive sign ('{' = X'C0'), as the Rust port does. Bytes 80 and 99 normalised."))
    rep.cmp_value(t, "VBRCFILE (variable records)", "identical bytes" if (ds / f"{hlq}.ACCTDATA.VBPS").read_bytes() ==
                  (w / "VBPS.dat").read_bytes() else "differ", "identical bytes")
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)
    for job, prog, dd, name in (("READCARD", "CBACT02C", "CARDFILE", "CARDDATA"), ("READXREF", "CBACT03C", "XREFFILE", "CARDXREF"),
                                ("READCUST", "CBCUS01C", "CUSTFILE", "CUSTDATA")):
        rs = rust(rh, "job", job)
        rc, out, err = cob_run(w, prog, {dd: idx[name]})
        t = f"{job} / {prog}"
        rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
        rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)
    rs = rust(rh, "job", "DALYVRFY")
    daly = seq_from_seed("AWS.M2.CARDDEMO.DALYTRAN.PS", 350, w / "DALYTRAN.dat")
    rc, out, err = cob_run(w, "CBTRN01C", {"DALYTRAN": daly, "CUSTFILE": idx["CUSTDATA"], "XREFFILE": idx["CARDXREF"],
                                          "CARDFILE": idx["CARDDATA"], "ACCTFILE": idx["ACCTDATA"], "TRANFILE": idx["TRANSACT"]})
    t = "DALYVRFY / CBTRN01C"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)

    rust(rh, "job", "TRANBKP")

    # ---------------- POSTTRAN / CBTRN02C
    pre = snap("pre-posttran")
    rs = rust(rh, "job", "POSTTRAN")
    post = snap("post-posttran")
    w = OUT / "cob-posttran"; w.mkdir()
    daly = seq_from_seed("AWS.M2.CARDDEMO.DALYTRAN.PS", 350, w / "DALYTRAN.dat")
    idx = {n: cob_load(w, n, pre / f"{n}.dat") for n in ("TRANSACT", "CARDXREF", "ACCTDATA", "TCATBALF")}
    rc, out, err = cob_run(w, "CBTRN02C", {"DALYTRAN": daly, "TRANFILE": idx["TRANSACT"], "XREFFILE": idx["CARDXREF"],
                                          "DALYREJS": w / "DALYREJS.dat", "ACCTFILE": idx["ACCTDATA"], "TCATBALF": idx["TCATBALF"]})
    t = "POSTTRAN / CBTRN02C"
    rep.cmp_value(t, "return code", 4 if "RC=0004" in rs else (0 if "RC=0000" in rs else "?"), rc)
    for n in ("TRANSACT", "ACCTDATA", "TCATBALF"):
        rep.cmp_records(t, n, post / f"{n}.dat", cob_unload(w, n, idx[n]), DS[n][0])
    rep.cmp_records(t, "DALYREJS", ds / f"{hlq}.DALYREJS.G0001V00", w / "DALYREJS.dat", 430)
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)

    # ---------------- INTCALC / CBACT04C
    pre = post
    rs = rust(rh, "job", "INTCALC")
    post = snap("post-intcalc")
    w = OUT / "cob-intcalc"; w.mkdir()
    idx = {n: cob_load(w, n, pre / f"{n}.dat") for n in ("TCATBALF", "CARDXREF", "ACCTDATA", "DISCGRP")}
    rc, out, err = cob_run(w, "CBACT04C", {"TCATBALF": idx["TCATBALF"], "XREFFILE": idx["CARDXREF"], "ACCTFILE": idx["ACCTDATA"],
                                          "DISCGRP": idx["DISCGRP"], "TRANSACT": w / "SYSTRAN.dat"}, [INTCALC_PARM])
    t = "INTCALC / CBACT04C"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_records(t, "ACCTDATA", post / "ACCTDATA.dat", cob_unload(w, "ACCTDATA", idx["ACCTDATA"]), 300)
    rep.cmp_records(t, "SYSTRAN (interest transactions)", ds / f"{hlq}.SYSTRAN.G0001V00", w / "SYSTRAN.dat", 350)
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)

    # ---------------- TRANREPT / CBTRN03C (after the nightly TRANBKP + COMBTRAN)
    rust(rh, "job", "TRANBKP")
    rust(rh, "job", "COMBTRAN")
    rs = rust(rh, "job", "TRANREPT")
    post = snap("post-tranrept")
    w = OUT / "cob-tranrept"; w.mkdir()
    idx = {n: cob_load(w, n, post / f"{n}.dat") for n in ("CARDXREF", "TRANTYPE", "TRANCATG")}
    parm = w / "DATEPARM.dat"
    parm.write_bytes(f"{REPORT[0]} {REPORT[1]}".ljust(80).encode())
    rc, out, err = cob_run(w, "CBTRN03C", {"TRANFILE": ds / f"{hlq}.TRANSACT.DALY.G0001V00", "CARDXREF": idx["CARDXREF"],
                                          "TRANTYPE": idx["TRANTYPE"], "TRANCATG": idx["TRANCATG"], "DATEPARM": parm,
                                          "TRANREPT": w / "TRANREPT.dat"})
    t = "TRANREPT / CBTRN03C"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_lines(t, "TRANREPT report", lines_of(ds / f"{hlq}.TRANREPT.G0001V00"), lines_of(w / "TRANREPT.dat", 133))
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)

    # ---------------- CREASTMT / CBSTM03A + CBSTM03B
    rs = rust(rh, "job", "CREASTMT")
    post = snap("post-creastmt")
    w = OUT / "cob-creastmt"; w.mkdir()
    idx = {n: cob_load(w, n, post / f"{n}.dat") for n in ("TRXFL", "CARDXREF", "ACCTDATA", "CUSTDATA")}
    rc, out, err = cob_run(w, "CBSTM03A", {"TRNXFILE": idx["TRXFL"], "XREFFILE": idx["CARDXREF"], "ACCTFILE": idx["ACCTDATA"],
                                          "CUSTFILE": idx["CUSTDATA"], "STMTFILE": w / "STATEMNT.PS", "HTMLFILE": w / "STATEMNT.HTML"})
    t = "CREASTMT / CBSTM03A+B"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_lines(t, "STATEMNT.PS (text)", lines_of(ds / f"{hlq}.STATEMNT.PS"), lines_of(w / "STATEMNT.PS", 80))
    rep.cmp_lines(t, "STATEMNT.HTML", lines_of(ds / f"{hlq}.STATEMNT.HTML"), lines_of(w / "STATEMNT.HTML", 100))

    # ---------------- CBEXPORT / CBIMPORT (branch migration) on the post-batch data
    rs = rust(rh, "job", "CBEXPORT")
    w = OUT / "cob-export"; w.mkdir()
    idx = {n: cob_load(w, n, post / f"{n}.dat") for n in ("CUSTDATA", "ACCTDATA", "CARDXREF", "TRANSACT", "CARDDATA")}
    rc, out, err = cob_run(w, "CBEXPORT", {"CUSTFILE": idx["CUSTDATA"], "ACCTFILE": idx["ACCTDATA"], "XREFFILE": idx["CARDXREF"],
                                          "TRANSACT": idx["TRANSACT"], "CARDFILE": idx["CARDDATA"], "EXPFILE": w / "EXPORT.idx"})
    t = "CBEXPORT (patched FD, KD-12)"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    rep.cmp_records(t, "EXPFILE", ds / f"{hlq}.EXPORT.DATA", cob_unload(w, "EXPORT", w / "EXPORT.idx"), 500)
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)
    # inject one record with an unknown type (exercises 2700-PROCESS-UNKNOWN-RECORD / ERROUT on both sides)
    exp = ds / f"{hlq}.EXPORT.DATA"
    recs = records(exp, 500)
    bad = bytearray(recs[-1])
    bad[0:1] = b"Z"
    bad[27:31] = (int.from_bytes(bad[27:31], "big") + 1).to_bytes(4, "big")
    exp.write_bytes(b"".join(recs) + bytes(bad))
    (w / "EXPORT.idx").unlink()
    cob_load(w, "EXPORT", exp)
    rs = rust(rh, "job", "CBIMPORT")
    outs = {"CUSTOUT": ("CUSTDATA.IMPORT", 500), "ACCTOUT": ("ACCTDATA.IMPORT", 300), "XREFOUT": ("CARDXREF.IMPORT", 50),
            "TRNXOUT": ("TRANSACT.IMPORT", 350), "CARDOUT": ("CARDDATA.IMPORT", 150), "ERROUT": ("IMPORT.ERRORS", 132)}
    rc, out, err = cob_run(w, "CBIMPORT", {"EXPFILE": w / "EXPORT.idx", **{dd: w / f"{n}" for dd, (n, _) in outs.items()}})
    t = "CBIMPORT (patched FD, KD-12)"
    rep.cmp_value(t, "return code", 0 if "RC=0000" in rs else "?", rc)
    for dd, (n, l) in outs.items():
        # ERR-TIMESTAMP = FUNCTION CURRENT-DATE: hundredths and UTC offset are host-dependent
        rep.cmp_records(t, dd, ds / f"{hlq}.{n}", w / n, l, mask=[(14, 21)] if dd == "ERROUT" else ())
    rep.cmp_lines(t, "SYSOUT", rust_sysout(rs), out)

    ok = rep.write(OUT / "report.md")
    print((OUT / "report.md").read_text())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
