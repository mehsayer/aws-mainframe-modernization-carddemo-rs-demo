#!/usr/bin/env python3
"""Differential fuzzing: random input for the original COBOL programs and the Rust translation, every output
compared, as in the other difftests.

batch   Random daily transaction files (DALYTRAN) for the full batch comparison (`run.py --dalytran`):
        DALYVRFY, POSTTRAN and everything after it (INTCALC, TRANREPT, CREASTMT, export/import) run on data
        the fuzzed postings changed. The records mix valid transactions with every reject reason of
        CBTRN02C: card not in the cross reference (100), over the credit limit (102), after the account's
        expiry (103), unknown transaction categories, large, zero and negative amounts, duplicate ids.
online  Random keystroke sessions (`crates/online/tests/fuzz_sessions.rs`) on the Rust and the COBOL
        programs, every screen compared (`online.py --only fuzz_sessions`).

Numeric fields always get valid digits: invalid data moved into a numeric item is compiler-dependent
(CLAUDE.md), so it would only produce expected differences.

  python3 difftest/fuzz.py batch [--seeds N] [--first S] [--records R]    # default 3 seeds from 1, 400 records
  python3 difftest/fuzz.py online [--seeds N] [--first S] [--sessions K] [--steps T]
A seed whose comparison differs is kept in difftest/out-fuzz/<kind>-seed-<S>/ (input, report); exit 1.
"""
import random, re, shutil, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
RS = HERE.parent
SEED = RS.parent / "cobol" / "app" / "data" / "EBCDIC"
OUT = HERE / "out-fuzz"


def arg(name, default):
    return int(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default


def ebcdic_records(name, n):
    b = (SEED / name).read_bytes()
    return [b[i:i + n].decode("cp037") for i in range(0, len(b) - n + 1, n)]


def zoned(value_cents, digits):
    """S9(n)V99 DISPLAY with the sign over-punched in the last digit, as text in code page 037."""
    s = f"{abs(value_cents):0{digits}d}"[-digits:]
    last = int(s[-1])
    sign = ("{ABCDEFGHI" if value_cents >= 0 else "}JKLMNOPQR")[last]
    return s[:-1] + sign


def daily_transactions(rnd, n):
    xref = [r[:16] for r in ebcdic_records("AWS.M2.CARDDEMO.CARDXREF.PS", 50)]
    cats = [(r[:2], r[2:6]) for r in ebcdic_records("AWS.M2.CARDDEMO.TRANCATG.PS", 60)]
    words = "Purchase Refund Payment Fee Interest Return Online Store Fuel Grocery Travel Hotel Cafe Books".split()
    ids, recs = [], []
    for _ in range(n):
        tid = f"{rnd.randrange(10 ** 15, 10 ** 16):016d}"
        if ids and rnd.random() < 0.02:
            tid = rnd.choice(ids)  # duplicate id
        ids.append(tid)
        typ, cat = rnd.choice(cats) if rnd.random() < 0.9 else (f"{rnd.randrange(1, 10):02d}", f"{rnd.randrange(1, 10000):04d}")
        roll = rnd.random()
        card = rnd.choice(xref) if roll < 0.85 else f"{rnd.randrange(10 ** 15, 10 ** 16):016d}"  # 100: unknown card
        # mostly ordinary amounts (posted), some large (over the limit: 102), negative, zero
        amount = rnd.choices([rnd.randrange(-50000, 50000), rnd.randrange(0, 5_000_000), rnd.randrange(-500_000_000, 999_999_999), 0],
                             weights=[70, 15, 10, 5])[0]
        y = rnd.choice([2018, 2020, 2022, 2022, 2022, 2024, 2026])
        ts = f"{y}-{rnd.randrange(1, 13):02d}-{rnd.randrange(1, 29):02d} {rnd.randrange(24):02d}:{rnd.randrange(60):02d}:{rnd.randrange(60):02d}.000000"
        rec = (tid + typ + cat + rnd.choice(["POS TERM", "OPERATOR", "ONLINE", "ATM"]).ljust(10)
               + " ".join(rnd.choice(words) for _ in range(rnd.randrange(1, 6))).ljust(100)[:100]
               + zoned(amount, 11) + f"{rnd.randrange(10 ** 9):09d}"
               + (rnd.choice(words) + " " + rnd.choice(words)).ljust(50)[:50] + rnd.choice(words).ljust(50)[:50]
               + f"{rnd.randrange(10 ** 5):05d}".ljust(10) + card + ts + " " * 26 + " " * 20)
        assert len(rec) == 350, len(rec)
        recs.append(rec.encode("cp037"))
    return b"".join(recs)


def batch():
    first, seeds, n = arg("--first", 1), arg("--seeds", 3), arg("--records", 400)
    print("  building ...", file=sys.stderr, flush=True)
    subprocess.run([sys.executable, str(HERE / "run.py"), "--build-only"], check=True, capture_output=True)
    failed = []
    for seed in range(first, first + seeds):
        d = OUT / f"batch-seed-{seed}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        daly = d / "DALYTRAN.PS"
        daly.write_bytes(daily_transactions(random.Random(seed), n))
        r = subprocess.run([sys.executable, str(HERE / "run.py"), "--no-build", "--dalytran", str(daly)], capture_output=True, text=True)
        verdict = (r.stdout.strip().splitlines() or ["?"])[-1]
        ok = r.returncode == 0 and "all outputs identical" in verdict
        print(f"  batch seed {seed}: {n} daily transactions: {verdict}", flush=True)
        if ok:
            shutil.rmtree(d)
        else:
            failed.append(seed)
            shutil.copytree(HERE / "out", d / "report")
            (d / "output.txt").write_text(r.stdout + r.stderr)
    return failed


def online():
    first, seeds = arg("--first", 1), arg("--seeds", 3)
    failed = []
    for seed in range(first, first + seeds):
        env = {"CARDDEMO_FUZZ_SEED": str(seed), "CARDDEMO_FUZZ_SESSIONS": str(arg("--sessions", 12)),
               "CARDDEMO_FUZZ_STEPS": str(arg("--steps", 30))}
        r = subprocess.run([sys.executable, str(HERE / "online.py"), "--only", "fuzz_sessions"], capture_output=True, text=True,
                           env={**__import__("os").environ, **env})
        m = re.search(r"Screens compared: \d+; identical: \d+[^\n]*", r.stdout + r.stderr)
        print(f"  online seed {seed}: {m.group(0) if m else 'no result'}", flush=True)
        if r.returncode:
            failed.append(seed)
            d = OUT / f"online-seed-{seed}"
            shutil.rmtree(d, ignore_errors=True)
            shutil.copytree(HERE / "out-online", d)
            (d / "output.txt").write_text(r.stdout + r.stderr)
    return failed


if __name__ == "__main__":
    kind = sys.argv[1] if len(sys.argv) > 1 else ""
    if kind not in ("batch", "online"):
        sys.exit(__doc__)
    failed = batch() if kind == "batch" else online()
    print(f"{kind} fuzzing: " + (f"differences for seed(s) {failed}, kept in {OUT}" if failed else "all seeds identical"))
    sys.exit(1 if failed else 0)
