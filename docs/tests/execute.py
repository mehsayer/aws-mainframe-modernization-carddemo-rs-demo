#!/usr/bin/env python3
"""Execute the manual test suite and record the results in results.json for the execution log.

- The online and batch cases run as scripted terminal sessions (rust/crates/online/tests/manual_suite.rs).
  They run once, inside rust/difftest/online.py, which runs them on the Rust programs and on the original
  COBOL programs and compares every screen; the Rust results are the execution result, the COBOL run is
  noted. Without GnuCOBOL, or if the comparison fails, the suite runs on Rust only; a failed comparison
  run's output is kept in rust/difftest/out-online-failed and TC-DIF-01 is logged as Fail.
- TC-BAT-08 runs the batch comparison (rust/difftest/run.py).
- TC-DIF-01: step 2 is the online comparison; step 1 (an interactive side-by-side session) is manual and is
  not executed by this script, which the log says.

  python3 docs/tests/execute.py        # then: python3 docs/tests/build.py
"""
import datetime, json, os, re, shutil, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
RS = os.path.join(HERE, "..", "..", "rust")
sys.path.insert(0, HERE)
from cases import CASES  # noqa: E402

ONLINE = os.path.join(RS, "difftest", "online.py")
OUT_ONLINE = os.path.join(RS, "difftest", "out-online")
RESULTS = os.path.join(OUT_ONLINE, "results.json")
FAILED = os.path.join(RS, "difftest", "out-online-failed")  # a failed comparison run, kept for diagnosis


failure = ""  # why the comparison run gave no result


def run_online(*args):
    """Run online.py; its results.json, or None if this run did not write one. online.py writes results.json
    last (also when screens differ, with exit code 1), and an earlier run's file is removed first, so a file
    found afterwards is this run's."""
    if os.path.exists(RESULTS):
        os.remove(RESULTS)
    r = subprocess.run([sys.executable, ONLINE, *args], cwd=RS, capture_output=True, text=True)
    global failure
    res = json.load(open(RESULTS)) if os.path.exists(RESULTS) else None
    if res is None:
        failure = f"did not complete (exit code {r.returncode})"
    elif r.returncode not in (0, 1) or (r.returncode == 0) != bool(res.get("ok")):
        failure, res = f"exit code {r.returncode} does not match its results", None
    if res is None:
        sys.stderr.write(f"online.py {' '.join(args)} {failure}:\n{r.stderr[-2000:]}\n")
        if not args and os.path.isdir(OUT_ONLINE):
            # the --rust-only fallback clears out-online: keep this run's report and screen logs
            shutil.rmtree(FAILED, ignore_errors=True)
            shutil.move(OUT_ONLINE, FAILED)
            with open(os.path.join(FAILED, "stderr.txt"), "w") as f:
                f.write(r.stdout + r.stderr)
    return res


online = run_online()
if online is not None:
    shutil.rmtree(FAILED, ignore_errors=True)  # an earlier failure's output no longer applies
no_cobol = shutil.which("cobc") is None
if online is None:  # no GnuCOBOL, or the comparison failed: the same tests on Rust only (same parser and format)
    rust_run = run_online("--rust-only")
    if rust_run is None:
        sys.exit("the scripted test suite could not run")
    tests = rust_run["tests"].get("manual_suite", {})
else:
    tests = online["tests"].get("manual_suite", {})
rust_tests, cobol_tests = tests.get("rust", {}), tests.get("cobol", {})

d = subprocess.run([sys.executable, os.path.join(RS, "difftest", "run.py")], cwd=RS, capture_output=True, text=True)
rows = len(re.findall(r"^\| .* \| (identical|\*\*)", d.stdout, re.M))

today = datetime.date.today().isoformat()
res = {}
for c in CASES:
    cid = c[0]
    if cid == "TC-BAT-08":
        res[cid] = {"result": "Pass" if d.returncode == 0 else "Fail",
                    "notes": f"difftest/run.py: {rows} comparisons, " + ("all identical" if d.returncode == 0 else "differences, see report")}
        continue
    if cid == "TC-DIF-01":
        if online is None and no_cobol:
            res[cid] = {"result": "Not run", "notes": "online.py could not run: GnuCOBOL (cobc) not installed"}
        elif online is None:
            res[cid] = {"result": "Fail", "notes": f"online.py {failure}; its output is in rust/difftest/out-online-failed"}
        else:
            res[cid] = {"result": "Pass (step 2)" if online["ok"] else "Fail",
                        "notes": f"step 2: {online['identical']}/{online['screens']} screens identical; "
                                 "step 1 (interactive side by side) is manual, not run by the script"}
        continue
    key = cid.lower().replace("-", "_")
    t = rust_tests.get(key)
    if t is None:
        res[cid] = {"result": "Not run", "notes": ""}
        continue
    note = t.get("note") or "scripted terminal"
    cb = cobol_tests.get(key)
    if cb:
        note += "; same case on the original COBOL: " + ("passes" if cb["result"] == "ok" else "FAILS")
    res[cid] = {"result": "Pass" if t["result"] == "ok" else "Fail", "notes": note}
json.dump({"date": today, "tester": "Claude Code (scripted)", "results": res},
          open(os.path.join(HERE, "results.json"), "w"), indent=1)
n = {"Pass": sum(1 for v in res.values() if v["result"] == "Pass"),
     "Pass (partly manual, automated part passed)": sum(1 for v in res.values() if v["result"].startswith("Pass (")),
     "Fail": sum(1 for v in res.values() if v["result"] == "Fail"),
     "Not run": sum(1 for v in res.values() if v["result"] == "Not run")}
print(f"{len(res)} cases: {n}")
sys.exit(0 if n["Fail"] == 0 and n["Not run"] == 0 else 1)
