#!/usr/bin/env python3
"""Online differential test: the ORIGINAL COBOL screen programs (on the CICS bridge) against the Rust port.

The online test suites (the 40 scripted manual cases and the other terminal journeys) run twice, once on
the Rust programs and once on the original COBOL programs, with the same keystrokes and the same data.
Every screen the terminal shows is recorded (text, map, cursor, each field's contents, colour and
attributes) and the two recordings are compared screen by screen. The tests' own assertions (messages,
data written to the files) must pass in both runs.

  python3 rust/difftest/online.py      # needs GnuCOBOL; builds the COBOL modules first
"""
import os, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RS = HERE.parent
OUT = HERE / "out-online"
# test targets that drive the terminal through tests/common::region() (the others build their own regions)
TARGETS = ["manual_suite", "signon", "users", "cards_accounts", "transactions", "review_fixes"]


def run(target, region, logdir):
    env = dict(os.environ, CARDDEMO_SCREEN_LOG=str(logdir))
    if region == "cobol":
        env["CARDDEMO_REGION"] = "cobol"
    r = subprocess.run(["cargo", "test", "--release", "-q", "-p", "online", "--test", target, "--", "--test-threads=1"],
                       cwd=RS, env=env, capture_output=True, text=True)
    m = re.search(r"test result: \w+\. (\d+) passed; (\d+) failed", r.stdout)
    return (int(m.group(1)), int(m.group(2))) if m else (0, -1)


# Accepted differences: invalid data moved into a numeric field, where the result depends on the compiler.
# (field-line regex for the Rust run, for the COBOL run, explanation)
ACCEPTED = [
    (r'^  TAMT\d{3} "\+00000000\.00" ', r'^  TAMT\d{3} "\+[ .0]{11}" ',
     "Transaction list, amount of the all-X'00' initialisation record: MOVE of X'00' 'digits' to +99999999.99. "
     "IBM COBOL uses the low half of each byte (0), as the Rust port does; GnuCOBOL leaves the integer digits blank."),
    (r'^  OPTION "[^0-9 "][^"]*" ', r'^  OPTION "00" ',
     "Menu option typed as letters: MOVE of the alphanumeric WS-OPTION-X to the numeric WS-OPTION and back to the "
     "screen. GnuCOBOL converts it to 00; IBM COBOL copies the bytes and repairs only the last zone; the Rust port "
     "redisplays the text."),
]


def screens(path):
    """Screen states: MAP/CURSOR line, TEXT line and the named-field lines (the rendered 24x80 text is derived
    from these fields plus the BMS constants, which both runs take from the same map definitions)."""
    if not path.exists():
        return []
    out = []
    for scr in path.read_text(encoding="latin-1").split("----\n")[:-1]:
        out.append([l for l in scr.splitlines() if l.startswith(("MAP ", "TEXT ")) or re.match(r'^  [A-Z][A-Z0-9]* "', l)])
    return out


def accepted(a, c):
    """True when two differing field lines are a documented compiler difference; records which one."""
    for i, (ra, rc, _) in enumerate(ACCEPTED):
        if re.match(ra, a) and re.match(rc, c):
            USED.add(i)
            return True
    return False


USED = set()


def same_screen(x, y):
    if len(x) != len(y):
        return False
    return all(u == v or accepted(u, v) for u, v in zip(x, y))


def main():
    b = subprocess.run([sys.executable, str(RS / "cobridge" / "cicsprep.py")], capture_output=True, text=True)
    if b.returncode:
        sys.exit(b.stdout + b.stderr)
    shutil.rmtree(OUT, ignore_errors=True)
    rows, details, total_screens, same_screens, ok = [], [], 0, 0, True
    for target in TARGETS:
        res = {}
        for region in ("rust", "cobol"):
            d = OUT / target / region
            d.mkdir(parents=True)
            res[region] = run(target, region, d)
        tests = sorted({p.name for p in (OUT / target / "rust").glob("*.log")} | {p.name for p in (OUT / target / "cobol").glob("*.log")})
        t_same = 0
        for name in tests:
            a, c = screens(OUT / target / "rust" / name), screens(OUT / target / "cobol" / name)
            total_screens += max(len(a), len(c))
            same = sum(1 for x, y in zip(a, c) if same_screen(x, y))
            same_screens += same
            if len(a) == len(c) and same == len(a):
                t_same += 1
                continue
            ok = False
            i = next((k for k, (x, y) in enumerate(zip(a, c)) if not same_screen(x, y)), min(len(a), len(c)))
            lines = [f"### {target} / {name[:-4]}: screen {i + 1} of {len(a)} (Rust) / {len(c)} (COBOL)", "```"]
            xa = a[i] if i < len(a) else ["<no screen>"]
            xc = c[i] if i < len(c) else ["<no screen>"]
            for u, v in zip(xa + [""] * (len(xc) - len(xa)), xc + [""] * (len(xa) - len(xc))):
                if u != v:
                    lines += [f"Rust : {u.rstrip()}", f"COBOL: {v.rstrip()}"]
            details.append("\n".join(lines[:40] + ["```"]))
        rp, rf = res["rust"]
        cp, cf = res["cobol"]
        ok &= rf == 0 and cf == 0
        rows.append(f"| {target} | {len(tests)} | {rp}/{rp + rf} | {cp}/{cp + cf} | {t_same}/{len(tests)} |")
    md = ["# Online differential test: original COBOL (CICS bridge) vs Rust", "",
          "Same keystrokes, same data; every screen compared (text, map, cursor, field contents, colours, attributes).", "",
          "| Test file | Tests | Pass (Rust) | Pass (COBOL) | Identical screen sequences |", "|---|--:|--:|--:|--:|"] + rows
    notes = [f"Accepted difference {k + 1}: {ACCEPTED[k][2]}" for k in sorted(USED)]
    md += ["", f"Screens compared: {total_screens}; identical: {same_screens}" +
           (f" (including {len(notes)} accepted compiler difference(s), below)." if notes else "."), ""] + notes + ["",
           f"**Overall: {'all screens identical, all tests pass on both' if ok else 'differences found'}**", ""] + details
    (OUT / "report.md").write_text("\n".join(md) + "\n")
    print("\n".join(md))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
