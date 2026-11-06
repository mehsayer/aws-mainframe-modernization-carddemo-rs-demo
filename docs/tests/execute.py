#!/usr/bin/env python3
"""Execute the manual test suite and record the results in results.json for the execution log.

- The online and batch cases run as scripted terminal sessions (rust/crates/online/tests/manual_suite.rs).
  They run once, inside rust/difftest/online.py, which runs them on the Rust programs and on the original
  COBOL programs and compares every screen; the Rust results are the execution result, the COBOL run is
  noted. Without GnuCOBOL the suite runs on Rust only.
- TC-BAT-08 runs the batch comparison (rust/difftest/run.py).
- TC-DIF-01: step 2 is the online comparison; step 1 (an interactive side-by-side session) is manual and is
  not executed by this script, which the log says.

  python3 docs/tests/execute.py        # then: python3 docs/tests/build.py
"""
import datetime, json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
RS = os.path.join(HERE, "..", "..", "rust")
sys.path.insert(0, HERE)
from cases import CASES  # noqa: E402

o = subprocess.run([sys.executable, os.path.join(RS, "difftest", "online.py")], cwd=RS, capture_output=True, text=True)
online_json = os.path.join(RS, "difftest", "out-online", "results.json")
online = json.load(open(online_json)) if o.returncode in (0, 1) and os.path.exists(online_json) else None

if online and "manual_suite" in online["tests"]:
    rust_tests = online["tests"]["manual_suite"].get("rust", {})
    cobol_tests = online["tests"]["manual_suite"].get("cobol", {})
else:  # no GnuCOBOL: the suite on Rust alone
    r = subprocess.run(["cargo", "test", "--release", "-p", "online", "--test", "manual_suite", "--", "--test-threads=1"],
                       cwd=RS, capture_output=True, text=True)
    out = r.stdout + r.stderr
    rust_tests = {m.group(1): {"result": m.group(2), "note": ""} for m in re.finditer(r"^test (tc_\w+) \.\.\. (ok|FAILED)$", out, re.M)}
    for m in re.finditer(r"thread '(tc_\w+)'.*?panicked at [^\n]*\n([^\n]*)", out, re.S):
        rust_tests.setdefault(m.group(1), {"result": "FAILED"})["note"] = m.group(2).strip()[:120]
    cobol_tests = {}

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
        if online is None:
            res[cid] = {"result": "Not run", "notes": "online.py could not run (GnuCOBOL missing?)"}
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
n = {k: sum(1 for v in res.values() if v["result"].startswith(k)) for k in ("Pass", "Fail", "Not run")}
print(f"{len(res)} cases: {n}")
sys.exit(0 if n["Fail"] == 0 and n["Not run"] == 0 else 1)
