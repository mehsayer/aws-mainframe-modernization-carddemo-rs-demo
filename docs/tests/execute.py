#!/usr/bin/env python3
"""Execute the manual test suite with the scripted terminal (rust/crates/online/tests/manual_suite.rs)
and the differential test (TC-BAT-08), and record the results in results.json for the execution log.

  python3 docs/tests/execute.py        # then: python3 docs/tests/build.py
"""
import datetime, json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
RS = os.path.join(HERE, "..", "..", "rust")
sys.path.insert(0, HERE)
from cases import CASES  # noqa: E402

r = subprocess.run(["cargo", "test", "--release", "-p", "online", "--test", "manual_suite", "--", "--test-threads=1"],
                   cwd=RS, capture_output=True, text=True)
out = r.stdout + r.stderr
status = {m.group(1): m.group(2) for m in re.finditer(r"^test (tc_\w+) \.\.\. (ok|FAILED)$", out, re.M)}
notes = {}
for m in re.finditer(r"thread '(tc_\w+)'.*?panicked at [^\n]*\n([^\n]*)", out, re.S):
    notes[m.group(1)] = m.group(2).strip()[:120]

d = subprocess.run([sys.executable, os.path.join(RS, "difftest", "run.py")], cwd=RS, capture_output=True, text=True)
rows = len(re.findall(r"^\| .* \| (identical|\*\*)", d.stdout, re.M))
o = subprocess.run([sys.executable, os.path.join(RS, "difftest", "online.py")], cwd=RS, capture_output=True, text=True)
om = re.search(r"Screens compared: (\d+); identical: (\d+)", o.stdout)

today = datetime.date.today().isoformat()
res = {}
for c in CASES:
    cid = c[0]
    if cid == "TC-BAT-08":
        res[cid] = {"result": "Pass" if d.returncode == 0 else "Fail",
                    "notes": f"difftest/run.py: {rows} comparisons, " + ("all identical" if d.returncode == 0 else "differences, see report")}
        continue
    if cid == "TC-DIF-01":
        res[cid] = {"result": "Pass" if o.returncode == 0 else "Fail",
                    "notes": f"difftest/online.py: {om.group(2)}/{om.group(1)} screens identical" if om else "online.py did not run"}
        continue
    key = cid.lower().replace("-", "_")
    st = status.get(key)
    res[cid] = {"result": {"ok": "Pass", "FAILED": "Fail"}.get(st, "Not run"),
                "notes": notes.get(key, "scripted terminal" if st == "ok" else "")}
json.dump({"date": today, "tester": "Claude Code (scripted)", "results": res},
          open(os.path.join(HERE, "results.json"), "w"), indent=1)
n = {k: sum(1 for v in res.values() if v["result"] == k) for k in ("Pass", "Fail", "Not run")}
print(f"{len(res)} cases: {n}")
sys.exit(0 if n["Fail"] == 0 and n["Not run"] == 0 else 1)
