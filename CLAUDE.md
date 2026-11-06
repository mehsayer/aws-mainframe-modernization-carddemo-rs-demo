# CardDemo: COBOL → Rust

A faithful translation of AWS CardDemo (CICS/VSAM/JCL/COBOL mainframe sample) into Rust.
Same screens, messages, record bytes and reports as the original, including its defects.

## Layout

| Path | Contents | Status |
|---|---|---|
| `cobol/` | The original AWS CardDemo, unchanged: `app/cbl` programs, `app/cpy` copybooks, `app/bms` screens, `app/jcl` jobs, `app/data` sample data | **Read-only reference** |
| `rust/` | The translation: Cargo workspace (`crates/cobol`, `vsam`, `cics`, `online`, `batch`, `carddemo`, `cobridge`), `difftest/` (GnuCOBOL comparisons), `cobridge/` (CICS translator for the original online programs), `tools/` (generators, metrics) | The product |
| `demo/` | `demo.py`: COBOL and Rust batch and online screens, side by side, and their comparisons | Demo tooling |
| `docs/` | Requirements, manual test suite, demo guide, presentation (Markdown sources + built PDFs) | Deliverables |

## Commands

```sh
cd rust && cargo build --release && cargo test --workspace     # build + all tests
cd rust && cargo clippy --workspace --all-targets               # must stay warning-free
python3 rust/difftest/run.py          # compile cobol/ with GnuCOBOL, compare all batch outputs with Rust
python3 rust/cobridge/cicsprep.py     # translate + compile the original online programs for the CICS bridge
python3 rust/difftest/online.py       # run the online tests on Rust and on the original COBOL, compare screens
                                      #   (--rust-only without GnuCOBOL; results in difftest/out-online/)
CARDDEMO_REGION=cobol cargo test --release -p online --tests -- --test-threads=1   # online tests on COBOL
python3 docs/tests/execute.py         # execute the manual test suite (42 cases) -> docs/tests/results.json
python3 demo/demo.py all              # demo: COBOL run, Rust run, compare
sh docs/build-all.sh                  # rebuild requirements / test suite / presentation PDFs
sh rust/tools/regen-records.sh        # regenerate crates/cobol/src/copybooks.rs from cobol/app/cpy
```

## Code review

When asked to review code (a diff, a branch, a PR, "review this"), **review only `rust/`**.
- `cobol/` is the original source and the specification. Never report findings against it and never
  propose changes to it; read it only to check that the Rust code matches it.
- A behaviour that differs from the COBOL is a Rust bug, even if the COBOL behaviour looks wrong.
  Behaviour that is wrong in the original is kept on purpose and documented as `KD-nn`
  (`docs/requirements/*.md`); do not report those as findings.
- `docs/` and `demo/` changes: check only that commands and facts in them are still correct.

## Translation rules (learned the hard way)

- **Read COBOL raw.** Never through grep filters or tools that collapse spaces: literals such as
  `' Cross ref file.  Resp:'` contain double spaces that matter. Check a literal with `cat -A`.
- **Storage without VALUE is binary zeros** (WORKING-STORAGE and FD records). `INITIALIZE` skips
  FILLER and REDEFINES items. Generated `Record::new()` starts from spaces; use
  `Record::from_bytes(&[0; Record::LEN])` where the original area has no VALUE.
- **Statement order matters**: a later `SEND MAP` or message move overwrites an earlier one in the same
  task; reproduce the order exactly. One Rust function per COBOL paragraph, named after it.
- **STRING INTO a shorter field truncates** (e.g. WS-RETURN-MSG is 75 bytes, the screen field 78).
- Every behaviour change must keep `rust/difftest/run.py` at "all outputs identical", `rust/difftest/online.py`
  at "all screens identical" and the manual suite at 42/42. If a test-case text disagrees with the COBOL, the
  COBOL wins: fix the case text. The online tests can run on the original programs, so check doubtful
  behaviour there instead of guessing.
- **CICS gives each task fresh WORKING-STORAGE**; the bridge CANCELs every COBOL program after its task.
  Invalid data moved into numeric fields is compiler-dependent: document it, do not "fix" it.
- `rust/crates/cobridge` loads GnuCOBOL's libcob at run time (dlopen; `CARDDEMO_LIBCOB` names the exact
  library, with no fallback), so every build works without GnuCOBOL; only running the COBOL programs needs
  it. The module directory rule lives in `cicsprep.modules_dir()` (Python users import it) and
  `cobridge::modules_dir()`; cicsprep only ever replaces its own module files there. COBOL DISPLAY
  output and libcob messages go to the file given to `cobridge::set_sysout` (the TUI uses
  `<home>/cobol-sysout.log`). `COB_CURRENT_DATE` is set only when `CARDDEMO_NOW` freezes the Rust clock.

## Conventions

- Work on branch `rust-port`; commit per finished step; do not push unless asked.
- Requirements IDs: `FR-*`, `NFR-*`, known defects `KD-nn` (KD-06 withdrawn). New findings go into
  `docs/requirements/` and `docs/progress-log.md`.
- Rebuild the affected PDF after changing its Markdown source.
