#!/usr/bin/env python3
"""CardDemo live demo: run the nightly batch with the ORIGINAL COBOL programs and with the NEW Rust
programs, then compare what they produced.

    python3 demo/demo.py cobol      # original COBOL programs (compiled with GnuCOBOL)
    python3 demo/demo.py rust       # the Rust translation
    python3 demo/demo.py compare    # are the results the same?
    python3 demo/demo.py online         # the online screens, Rust translation
    python3 demo/demo.py online-cobol   # the same screens, ORIGINAL COBOL programs (CICS bridge)
    python3 demo/demo.py online-compare # replay the scripted terminal sessions on both, compare every screen
    python3 demo/demo.py all            # cobol, rust, compare in one go

Results go to demo/out/cobol and demo/out/rust, with the same file names on both sides:
  posting-log.txt   console output of the daily posting program
  rejected.txt      transactions the posting refused (and why)
  report.txt        the transaction report (TRANREPT)
  statements.txt    customer statements, text version
  statements.html   customer statements, web version (open in a browser)
"""
import os, shutil, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "rust" / "difftest"))
import run as h  # noqa: E402  (the differential-test harness: build + COBOL helpers)

OUT = ROOT / "demo" / "out"
HLQ = "AWS.M2.CARDDEMO"
REPORT = ("2022-01-01", "2022-07-06")
PARM = "2022071800"

BOLD, GREEN, RED, DIM, END = ("\033[1m", "\033[32m", "\033[31m", "\033[2m", "\033[0m") if sys.stdout.isatty() else ("",) * 5


def say(msg):
    print(f"{BOLD}==> {msg}{END}", flush=True)


def show(lines, n=6):
    for l in lines[:n]:
        print(f"    {DIM}{l}{END}")
    if len(lines) > n:
        print(f"    {DIM}... ({len(lines) - n} more lines in the log){END}")


def write_lines(path, lines):
    path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="latin-1")


def ensure_rust():
    """Build carddemo; cargo does nothing when it is up to date, so an old binary is never used."""
    r = subprocess.run(["cargo", "build", "--release", "-q", "-p", "carddemo"], cwd=ROOT / "rust")
    if r.returncode:
        sys.exit("Building the Rust programs failed (see the messages above).")


def ensure_built():
    """The batch demo: the Rust binary and the original batch COBOL programs."""
    ensure_rust()
    if not (h.BUILD / "CBSTM03A").exists():
        say("Compiling the original COBOL batch programs (first time only, about a minute) ...")
        h.build()


def cobol_modules_dir():
    """Same rule as the runtime (cobridge::modules_dir) and cicsprep.py."""
    return Path(os.environ.get("CARDDEMO_COBOL_MODULES") or ROOT / "rust" / "cobridge" / "build" / "modules")


# ------------------------------------------------------------------ COBOL

def cobol():
    ensure_built()
    out = OUT / "cobol"
    shutil.rmtree(out, ignore_errors=True)
    w = out / "work"
    w.mkdir(parents=True)
    t0 = time.time()
    say("COBOL: loading the sample data (the same files the mainframe version ships with)")
    seed = {}
    for name, f in (("ACCTDATA", "ACCTDATA.PS"), ("CARDXREF", "CARDXREF.PS"), ("CUSTDATA", "CUSTDATA.PS"),
                    ("TCATBALF", "TCATBALF.PS"), ("DISCGRP", "DISCGRP.PS"), ("TRANTYPE", "TRANTYPE.PS"),
                    ("TRANCATG", "TRANCATG.PS")):
        src = w / f"{name}.seq"
        src.write_bytes((h.SEED / f"{HLQ}.{f}").read_bytes().translate(h.E2A))
        seed[name] = h.cob_load(w, name, src)
    (w / "empty.seq").write_bytes(b"")
    seed["TRANSACT"] = h.cob_load(w, "TRANSACT", w / "empty.seq")  # TRANBKP: the transaction file starts empty
    daly = w / "DALYTRAN.seq"
    daly.write_bytes((h.SEED / f"{HLQ}.DALYTRAN.PS").read_bytes().translate(h.E2A))

    say("COBOL: daily posting (program CBTRN02C)")
    rc, log, _ = h.cob_run(w, "CBTRN02C", {"DALYTRAN": daly, "TRANFILE": seed["TRANSACT"], "XREFFILE": seed["CARDXREF"],
                                          "DALYREJS": w / "DALYREJS.seq", "ACCTFILE": seed["ACCTDATA"], "TCATBALF": seed["TCATBALF"]})
    write_lines(out / "posting-log.txt", log)
    show([l for l in log if "TRANSACTIONS" in l] + [f"return code {rc}"])
    write_lines(out / "rejected.txt", [r.decode("latin-1").rstrip() for r in h.records(w / "DALYREJS.seq", 430)])

    say("COBOL: interest calculation (program CBACT04C)")
    rc, log, _ = h.cob_run(w, "CBACT04C", {"TCATBALF": seed["TCATBALF"], "XREFFILE": seed["CARDXREF"], "ACCTFILE": seed["ACCTDATA"],
                                          "DISCGRP": seed["DISCGRP"], "TRANSACT": w / "SYSTRAN.seq"}, [PARM])
    show([f"{len(h.records(w / 'SYSTRAN.seq', 350))} interest transactions written, return code {rc}"])

    say("COBOL: combine today's transactions with the interest transactions (JCL job COMBTRAN: SORT + REPRO)")
    trans = h.records(h.cob_unload(w, "TRANSACT", seed["TRANSACT"]), 350) + h.records(w / "SYSTRAN.seq", 350)
    trans.sort(key=lambda r: r[0:16])
    (w / "COMBINED.seq").write_bytes(b"".join(trans))
    (w / "TRANSACT.idx").unlink()
    h.cob_load(w, "TRANSACT", w / "COMBINED.seq")

    say("COBOL: transaction report (program CBTRN03C)")
    daly_sorted = sorted((r for r in trans if REPORT[0].encode() <= r[304:314] <= REPORT[1].encode()), key=lambda r: r[262:278])
    (w / "DALY.seq").write_bytes(b"".join(daly_sorted))
    (w / "DATEPARM.seq").write_bytes(f"{REPORT[0]} {REPORT[1]}".ljust(80).encode())
    rc, log, _ = h.cob_run(w, "CBTRN03C", {"TRANFILE": w / "DALY.seq", "CARDXREF": seed["CARDXREF"], "TRANTYPE": seed["TRANTYPE"],
                                          "TRANCATG": seed["TRANCATG"], "DATEPARM": w / "DATEPARM.seq", "TRANREPT": w / "TRANREPT.seq"})
    write_lines(out / "report.txt", h.lines_of(w / "TRANREPT.seq", 133))

    say("COBOL: customer statements (programs CBSTM03A + CBSTM03B)")
    trx = sorted(trans, key=lambda r: (r[262:278], r[0:16]))
    (w / "TRXFL.seq").write_bytes(b"".join(r[262:278] + r[0:262] + r[278:328] + b" " * 22 for r in trx))
    idx = {"TRXFL": h.cob_load(w, "TRXFL", w / "TRXFL.seq")}
    for n in ("CARDXREF", "CUSTDATA"):
        idx[n] = seed[n]
    rc, log, _ = h.cob_run(w, "CBSTM03A", {"TRNXFILE": idx["TRXFL"], "XREFFILE": idx["CARDXREF"], "ACCTFILE": seed["ACCTDATA"],
                                          "CUSTFILE": idx["CUSTDATA"], "STMTFILE": w / "STMT.seq", "HTMLFILE": w / "HTML.seq"})
    write_lines(out / "statements.txt", h.lines_of(w / "STMT.seq", 80))
    write_lines(out / "statements.html", h.lines_of(w / "HTML.seq", 100))
    finish("COBOL", out, t0)


# ------------------------------------------------------------------ Rust

def rust():
    ensure_rust()
    out = OUT / "rust"
    shutil.rmtree(out, ignore_errors=True)
    home = out / "work"
    t0 = time.time()
    say("Rust: loading the sample data")
    h.rust(home, "reset")
    ds = home / "datasets"
    for job, title in (("TRANBKP", None), ("POSTTRAN", "daily posting"), ("INTCALC", "interest calculation"),
                       ("TRANBKP", None), ("COMBTRAN", "combine transactions"), ("TRANREPT", "transaction report"),
                       ("CREASTMT", "customer statements")):
        if title:
            say(f"Rust: {title} (job {job})")
        text = h.rust(home, "job", job)
        if job == "POSTTRAN":
            log = h.rust_sysout(text)
            write_lines(out / "posting-log.txt", log)
            show([l for l in log if "TRANSACTIONS" in l] + ["return code " + ("4" if "RC=0004" in text else "0")])
    write_lines(out / "rejected.txt", [r.decode("latin-1").rstrip() for r in h.records(ds / f"{HLQ}.DALYREJS.G0001V00", 430)])
    write_lines(out / "report.txt", h.lines_of(ds / f"{HLQ}.TRANREPT.G0001V00"))
    write_lines(out / "statements.txt", h.lines_of(ds / f"{HLQ}.STATEMNT.PS"))
    write_lines(out / "statements.html", h.lines_of(ds / f"{HLQ}.STATEMNT.HTML"))
    finish("Rust", out, t0)


def finish(who, out, t0):
    print(f"{GREEN}{BOLD}{who} run finished in {time.time() - t0:.1f} s.{END} Results in {out.relative_to(ROOT)}/:")
    for f in ("posting-log.txt", "rejected.txt", "report.txt", "statements.txt", "statements.html"):
        p = out / f
        print(f"    {f:<17} {len(p.read_text(encoding='latin-1').splitlines()):>6} lines")


# ------------------------------------------------------------------ compare

def compare():
    a, b = OUT / "cobol", OUT / "rust"
    if not a.exists() or not b.exists():
        sys.exit("Run 'demo.py cobol' and 'demo.py rust' first.")
    say("Comparing the results of the COBOL run and the Rust run, file by file")
    same = True
    for f in ("posting-log.txt", "rejected.txt", "report.txt", "statements.txt", "statements.html"):
        x = (a / f).read_text(encoding="latin-1")  # COBOL console lines are already normalised by the harness
        y = (b / f).read_text(encoding="latin-1")
        ok = x.replace("\x00", " ") == y.replace("\x00", " ")
        same &= ok
        print(f"    {f:<17} {len(y.splitlines()):>6} lines   " + (f"{GREEN}IDENTICAL{END}" if ok else f"{RED}DIFFERENT{END}"))
    if same:
        print(f"{GREEN}{BOLD}The Rust programs produced exactly the same results as the original COBOL programs.{END}")
    else:
        print(f"{RED}{BOLD}Differences found{END}: see  diff demo/out/cobol/<file> demo/out/rust/<file>")
    return same


def online(cobol=False):
    ensure_rust()
    if cobol and not (cobol_modules_dir() / "COSGN00C.so").exists():
        say("Translating and compiling the original COBOL screen programs (first time only) ...")
        subprocess.run([sys.executable, str(ROOT / "rust" / "cobridge" / "cicsprep.py")], check=True)
    # separate data stores, so the two systems can run side by side
    env = dict(os.environ, CARDDEMO_HOME=str(OUT / ("online-cobol" if cobol else "online")), CARDDEMO_SEED=str(h.SEED))
    subprocess.run([str(h.BIN), "reset"], env=env, check=True, capture_output=True)
    which = "the ORIGINAL COBOL programs" if cobol else "the Rust translation"
    print(f"Online screens running {which}.")
    print("Sign on with  USER0001 / PASSWORD  (or ADMIN001 / PASSWORD).  F3 = back, Esc = clear, F3 on the sign-on screen = exit.")
    time.sleep(2)
    os.execve(str(h.BIN), [str(h.BIN), "online"] + (["--cobol"] if cobol else []), env)


def online_compare():
    """online.py builds what it needs (COBOL modules, test programs); its progress and any error message
    go to stderr, which is shown as it happens; the summary comes from its report."""
    say("Replaying the scripted terminal sessions on the Rust and on the original COBOL programs ...")
    r = subprocess.run([sys.executable, str(ROOT / "rust" / "difftest" / "online.py")], stdout=subprocess.PIPE, text=True)
    shown = False
    for l in r.stdout.splitlines():
        if l.startswith(("|", "Screens compared", "**Overall")):
            print("    " + l.replace("**", ""))
            shown = True
    if r.returncode and not shown:
        print(f"{RED}online.py failed{END}: see the messages above")
    elif r.returncode:
        print(f"Details: rust/difftest/out-online/report.md")
    return r.returncode == 0


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "help"
    os.environ.setdefault("CARDDEMO_NOW", h.NOW_RUST)
    if cmd == "cobol":
        cobol()
    elif cmd == "rust":
        rust()
    elif cmd == "compare":
        sys.exit(0 if compare() else 1)
    elif cmd == "online":
        online()
    elif cmd == "online-cobol":
        online(cobol=True)
    elif cmd == "online-compare":
        sys.exit(0 if online_compare() else 1)
    elif cmd == "all":
        cobol(); print(); rust(); print(); sys.exit(0 if compare() else 1)
    else:
        print(__doc__)
