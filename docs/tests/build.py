#!/usr/bin/env python3
"""Render docs/tests/cases.py into manual-test-suite.md and build the PDF."""
import os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from cases import CASES, R  # noqa: E402

def esc(s):
    return s.replace("|", "\\|")

reqs_src = "".join(open(os.path.join(HERE, "../requirements", f)).read()
                   for f in sorted(os.listdir(os.path.join(HERE, "../requirements"))) if f.endswith(".md"))
all_groups = sorted(set(re.findall(r"^\| (FR-[A-Z]+(?:-[A-Z]+)?)-\d+", reqs_src, re.M)))

out = ["""---
title: "CardDemo (Rust) — Manual Test Suite"
subtitle: "Test cases derived from the Software Requirements Specification v1.2"
date: "2026-11-06 — version 1.1"
author: "Illia Burov (prepared with Claude Code)"
toc: true
toc-depth: 2
geometry: margin=2cm
fontsize: 10pt
mainfont: "Noto Serif"
sansfont: "Noto Sans"
monofont: "JetBrains Mono"
colorlinks: true
header-includes:
  - \\usepackage{newunicodechar}
  - \\newunicodechar{→}{\\ensuremath{\\rightarrow}}
---

# How to use this suite

**Environment.** Build `carddemo` (`cargo build --release` in `rust/`), set `CARDDEMO_HOME` to an empty
directory and run `carddemo reset` before each test unless the precondition says otherwise.
Online tests run in `carddemo online`. Keys: Enter, F1–F12 = PF1–PF12, Shift+F1–F12 = PF13–PF24,
Esc = CLEAR, Tab between fields, Ctrl+E = ERASE EOF.

**Users.** `USER0001` / `PASSWORD` (regular user), `ADMIN001` / `PASSWORD` (administrator).
Account and card numbers have no right-justification in the maps, so type all 11 / 16 digits.

**Recording results.** For each step, compare the screen or output with *Expected result* and mark the case
Pass / Fail in the execution log at the end. Messages must match **exactly**, including spaces and punctuation.

**Original behaviour.** Some expected results reproduce known defects of the original COBOL application
(marked KD-nn). They are expected results, not failures.
"""]

groups = {}
for c in CASES:
    groups.setdefault(c[0].split("-")[1], []).append(c)
titles = {"SGN": "Sign-on", "MEN": "Main menu", "ADM": "Admin menu", "USR": "User administration",
          "ACV": "Account view", "ACU": "Account update", "CDL": "Card list", "CDV": "Card detail",
          "CDU": "Card update", "TRL": "Transaction list", "TRV": "Transaction view", "TRA": "Transaction add",
          "BIL": "Bill payment", "RPT": "Reports", "BAT": "Batch"}
for g, cs in groups.items():
    out.append(f"\n# {titles.get(g, g)}\n")
    for cid, title, reqs, pre, steps in cs:
        out.append(f"\n## {cid} — {title}\n")
        out.append(f"**Requirements:** {reqs}  \n**Precondition:** {pre}\n")
        out.append("| # | Action | Expected result |\n|--:|-----------------|---------------------------|")
        for i, (a, e) in enumerate(steps, 1):
            out.append(f"| {i} | {esc(a)} | {esc(e)} |")

# traceability: requirement group -> test cases
def groups_of(reqs):
    found = set()
    for tok in re.split(r"[,;]\s*", reqs):
        m = re.match(r"(FR-[A-Z]+(?:-[A-Z]+)?)-", tok.strip())
        if m:
            found.add(m.group(1))
    return found

cover = {g: [] for g in all_groups}
for cid, _, reqs, _, _ in CASES:
    for g in groups_of(reqs):
        cover.setdefault(g, []).append(cid)
out.append("\n# Requirements coverage\n")
out.append("| Requirement group | Test cases |\n|---|---|")
missing = []
for g in all_groups:
    tcs = cover.get(g, [])
    if not tcs:
        missing.append(g)
    out.append(f"| {g} | {', '.join(tcs) if tcs else '**not covered**'} |")
out.append(f"\n{len(CASES)} test cases, {sum(len(c[4]) for c in CASES)} steps; "
           f"{len(all_groups) - len(missing)} of {len(all_groups)} functional requirement groups covered.\n")

out.append("\n# Execution log\n")
log = None
if os.path.exists(os.path.join(HERE, "results.json")):
    import json
    log = json.load(open(os.path.join(HERE, "results.json")))
    res = log["results"]
    passed = sum(1 for v in res.values() if v["result"] == "Pass")
    out.append(f"Executed {log['date']} by {log['tester']}: the online and batch cases run through the same "
               "3270 screen model the terminal UI uses (`rust/crates/online/tests/manual_suite.rs`, one test "
               "per case, every step asserted); TC-BAT-08 runs the GnuCOBOL differential test. Re-run with "
               f"`python3 docs/tests/execute.py`. **{passed} of {len(res)} passed.**\n")
out.append("| Test case | Date | Tester | Result | Notes |\n|---|---|---|---|---|")
for c in CASES:
    if log and c[0] in log["results"]:
        e = log["results"][c[0]]
        out.append(f"| {c[0]} | {log['date']} | {log['tester']} | {e['result']} | {esc(e['notes'])} |")
    else:
        out.append(f"| {c[0]} | | | | |")

md = os.path.join(HERE, "manual-test-suite.md")
open(md, "w").write("\n".join(out) + "\n")
print(f"{len(CASES)} cases; uncovered groups: {missing or 'none'}")
subprocess.run(["pandoc", md, "--pdf-engine=xelatex", "--columns=50", "-o",
                os.path.join(HERE, "../manual-test-suite.pdf")], check=True)
