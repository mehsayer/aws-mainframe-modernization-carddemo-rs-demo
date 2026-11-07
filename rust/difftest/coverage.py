#!/usr/bin/env python3
"""COBOL paragraph coverage of the differential tests: which paragraphs of the original programs the
comparisons (run.py for batch, online.py for the screens) actually execute.

The original programs are compiled with GnuCOBOL's paragraph trace (`-ftrace`) and each COBOL run writes a
trace file (`COB_SET_TRACE`, `COB_TRACE_FILE`); the paragraphs and sections of each program come from its
source after the preprocessor (`cobc -E`, so paragraphs from copybooks count too). A paragraph is covered
when the trace shows it entered, by PERFORM, GO TO or falling through. The comparisons themselves run as
usual and must still report all outputs and screens identical.

  python3 difftest/coverage.py              # run both comparisons with the trace, then report
  python3 difftest/coverage.py --no-run     # report on the traces of the last run
Report: difftest/out-coverage/coverage.md (and coverage.json).
"""
import json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RS = HERE.parent
APP = RS.parent / "cobol" / "app"
OUT = HERE / "out-coverage"
TRACES = OUT / "traces"

# program -> (source, include directories): the batch programs as run.py compiles them, the online programs
# as cicsprep translates them (same paragraphs as the originals: only EXEC CICS blocks are replaced)
BATCH = ["CBACT01C", "CBACT02C", "CBACT03C", "CBACT04C", "CBCUS01C", "CBTRN01C", "CBTRN02C", "CBTRN03C",
         "CBSTM03A", "CBSTM03B", "CBEXPORT", "CBIMPORT"]
ONLINE = ["COSGN00C", "COMEN01C", "COADM01C", "COACTVWC", "COACTUPC", "COCRDLIC", "COCRDSLC", "COCRDUPC",
          "COTRN00C", "COTRN01C", "COTRN02C", "CORPT00C", "COBIL00C", "COUSR00C", "COUSR01C", "COUSR02C",
          "COUSR03C", "CSUTLDTC"]


def source(prog):
    if prog in ONLINE and prog != "CSUTLDTC":
        return RS / "cobridge" / "build" / "src" / f"{prog}.cbl", [RS / "cobridge" / "build" / "cpy"]
    src = next(p for p in (APP / "cbl").iterdir() if p.stem.upper() == prog)
    return src, [HERE / "build" / "cpy", RS / "cobridge" / "build" / "cpy"]


# Paragraphs that no comparison can enter, with the reason (checked in the COBOL source). A paragraph not
# entered and not listed here is "unexplained": a gap in the tests, reported, and the script exits with 1.
ABEND_PATH = "only after a file error: the abend path (DISPLAY of the file status, CEE3ABD)"
EXPLAINED = {
    "9999-ABEND-PROGRAM": ABEND_PATH, "9910-DISPLAY-IO-STATUS": ABEND_PATH,
    "Z-ABEND-PROGRAM": ABEND_PATH, "Z-DISPLAY-IO-STATUS": ABEND_PATH,
    "0000-MAIN-EXIT": "never reached: the main paragraph always ends with RETURN or GO TO COMMON-RETURN",
    "ABEND-ROUTINE": "only after an abend inside the program (EXEC CICS HANDLE ABEND LABEL)",
    "SEND-LONG-TEXT": "never performed (a debugging aid of the original)",
}
EXPLAINED_IN = {
    ("COACTVWC", "SEND-PLAIN-TEXT"): "only in the 'UNEXPECTED DATA SCENARIO' branch (WHEN OTHER)",
    ("COCRDSLC", "SEND-PLAIN-TEXT"): "only in the 'UNEXPECTED DATA SCENARIO' branch (WHEN OTHER)",
    ("COCRDLIC", "SEND-PLAIN-TEXT"): "never performed",
    ("COACTUPC", "1230-EDIT-ALPHANUM-REQD"): "never performed (dead code of the original)",
    ("COACTUPC", "1240-EDIT-ALPHANUM-OPT"): "never performed (dead code of the original)",
    ("COCRDSLC", "9150-GETCARD-BYACCT"): "never performed (dead code of the original)",
}


def reason(prog, para):
    """Why a paragraph cannot be entered, or None. NAME-EXIT shares the reason of NAME."""
    for name in (para, para[:-5] if para.endswith("-EXIT") else para):
        if (prog, name) in EXPLAINED_IN or name in EXPLAINED:
            return EXPLAINED_IN.get((prog, name)) or EXPLAINED[name]
    return None


LABEL = re.compile(r"^\s*([A-Z0-9][A-Z0-9-]*)( SECTION)?\.\s*$")


def paragraphs(prog):
    """The paragraph and section names of a program, in source order (from its preprocessed source)."""
    src, incs = source(prog)
    pre = OUT / "preprocessed" / f"{prog}.i"  # a file: cobc -E crashes writing to stdout (-o -)
    pre.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cobc", "-E", "-std=ibm", *sum([["-I", str(i)] for i in incs], []), str(src), "-o", str(pre)],
                   capture_output=True, check=True)
    text = pre.read_text(encoding="latin-1")
    names, in_proc, area_a = [], False, False
    for line in text.splitlines():
        if "PROCEDURE DIVISION" in line.upper():
            in_proc = True
            continue
        if line.startswith("#area_a"):
            area_a = True  # the preprocessor marks lines starting in area A: labels in the procedure division
            continue
        if line.startswith("#"):
            continue
        m = LABEL.match(line.upper()) if in_proc and area_a else None
        if m and m.group(1) not in names:
            names.append(m.group(1))
        if line.strip():
            area_a = False
    return names


TRACE = re.compile(r"Program-Id:\s+(\S+)\s+(?:Paragraph|Section):\s+(\S+)")


def entered():
    """(program, paragraph) pairs entered in any trace, and where (batch / online test target)."""
    seen = {}
    for t in sorted(TRACES.glob("*.trace")):
        where = "online" if t.name.startswith("online-") else "batch"
        for m in TRACE.finditer(t.read_text(encoding="latin-1")):
            seen.setdefault((m.group(1).upper(), m.group(2).upper()), set()).add(where)
    return seen


def run_comparisons():
    TRACES.mkdir(parents=True, exist_ok=True)
    for t in TRACES.glob("*.trace"):
        t.unlink()
    for script in ("run.py", "online.py"):
        print(f"  {script} --coverage ...", file=sys.stderr, flush=True)
        r = subprocess.run([sys.executable, str(HERE / script), "--coverage", str(TRACES)], capture_output=True, text=True)
        tail = (r.stdout + r.stderr).strip().splitlines()[-1:] or [""]
        print(f"    exit {r.returncode}: {tail[0]}", file=sys.stderr)
        if r.returncode:
            sys.exit(f"{script} failed: the comparison must pass before coverage means anything")


def main():
    if "--no-run" not in sys.argv:
        run_comparisons()
    seen = entered()
    rows, result, total, covered = [], {}, 0, 0
    for prog in BATCH + ONLINE:
        names = paragraphs(prog)
        hit = [n for n in names if (prog, n) in seen]
        miss = [n for n in names if (prog, n) not in seen]
        total += len(names)
        covered += len(hit)
        result[prog] = {"paragraphs": len(names), "covered": len(hit), "uncovered": miss}
        pct = 100 * len(hit) / len(names) if names else 100
        rows.append(f"| {prog} | {'online' if prog in ONLINE else 'batch'} | {len(hit)}/{len(names)} | {pct:.0f}% |")
    OUT.mkdir(parents=True, exist_ok=True)
    md = ["# COBOL paragraph coverage of the differential tests", "",
          "Paragraphs and sections of the original programs entered while `difftest/run.py` (batch) and "
          "`difftest/online.py` (screens) compare them with the Rust translation (GnuCOBOL `-ftrace`).", "",
          f"**Overall: {covered}/{total} paragraphs ({100 * covered / total:.0f}%)**", "",
          "| Program | Part | Covered | % |", "|---|---|--:|--:|", *rows, "", "## Paragraphs not covered", ""]
    unexplained = [(p, n) for p, r in result.items() for n in r["uncovered"] if not reason(p, n)]
    md += ["Each with the reason no comparison can enter it (checked in the COBOL source):", "",
           "| Program | Paragraph | Reason |", "|---|---|---|"]
    md += [f"| {p} | `{n}` | {reason(p, n)} |" for p, r in result.items() for n in r["uncovered"] if reason(p, n)]
    if unexplained:
        md += ["", "### Not covered and not explained: gaps in the tests", ""]
        md += [f"- {p} `{n}`" for p, n in unexplained]
    (OUT / "coverage.md").write_text("\n".join(md) + "\n")
    (OUT / "coverage.json").write_text(json.dumps({"covered": covered, "total": total, "programs": result,
                                                   "unexplained": unexplained}, indent=1))
    print(f"COBOL paragraph coverage: {covered}/{total} ({100 * covered / total:.0f}%), "
          f"{len(unexplained)} not covered without a reason; report: {OUT / 'coverage.md'}")
    sys.exit(1 if unexplained else 0)


if __name__ == "__main__":
    main()
