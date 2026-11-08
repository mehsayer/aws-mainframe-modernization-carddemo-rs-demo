# Progress Log — CardDemo COBOL → Rust

Metrics come from `rust/tools/usage.py`, which reads the Claude Code session transcripts.

| When (UTC+1) | Phase | Event | Active h (cum.) | Output tokens (cum.) |
|---|---|---|---:|---:|
| 2026-11-05 17:31 | 0 | Session 1 start; repo survey, plan presented | 0.00 | 0 |
| 2026-11-05 17:39 | 0 | Plan approved (scope: base app; TUI; SQLite; GnuCOBOL available) | 0.14 | 9.7k |
| 2026-11-05 17:52 | 1 | Requirements read-through of all 30 base programs; requirements.pdf v1.0 built (26 pp, 162 FR, 12 NFR, 7 KD) | see usage.py | see usage.py |

## Decisions and findings
- Known defects found in the original and kept for fidelity: KD-01 (INTCALC never writes back the last account), KD-02/03 (TRANREPT totals), KD-04 (statement table limits), KD-05 (CBACT01C carry-over), KD-07 (card update resets CVV), KD-08 (phone blank check).
- COTRN02C and CORPT00C end the task on every screen send, so only the first error is reported.
- Record layouts are generated from the copybooks (`tools/copybook.py`), and all 13 lengths match the documented sizes.
| 2026-11-05 ~18:40 | 2-3 | cobol/vsam crates; all batch programs + job runner; full batch cycle test, export/import round-trip byte-identical | | |
| 2026-11-05 ~19:10 | 4 | cics runtime (BMS parser, maps, XCTL/RETURN), 12 of 17 online programs with terminal-level tests | | |

## Findings to fold into requirements / test suite
- KD-09 CBIMPORT.jcl lacks a CARDOUT DD (the program opens CARD-OUTPUT).
- KD-10 COUSR00C/COTRN00C: ENDBR without RESP after STARTBR NOTFND → AEIP abend (search key beyond the last record).
- KD-11 User/transaction list: PF7 on page 1 sets NEXT-PAGE-YES; a following PF8 blanks the rows and reports "You have reached the bottom of the page...".
- Note: the sign-on password field is FSET with INITIAL '________'; if not erased it is transmitted, giving "Wrong Password".
- Note: TRANSACT contains the blank initialisation record from DALYTRAN.PS.INIT (first row of the transaction list).
- Note: account/card number inputs have no JUSTIFY: users must type all 11/16 digits.
- Deviation: EXPORT.DATA KSDS (KEYS(4 28)) is represented as a fixed-length sequential file in write order.
| 2026-11-05 18:32 | 4 | carddemo binary: ratatui TUI, CLI (reset/job/batch/online/script), internal reader for CR00 reports | | |
| 2026-11-05 18:35 | 1,6 | requirements v1.1; manual test suite PDF (40 cases, all FR groups covered) | | |
| 2026-11-05 18:45 | 8 | progress presentation PDF (17 slides) | 1.1 (session) | 367k (session) |

Session 1 totals (tools/usage.py): ≈1.1 h active, 194 turns, 212 tool calls, 367k output tokens, 90.9M cache-read tokens.
Per-phase breakdown: `python3 rust/tools/phase_metrics.py`.

## Session 2 (2026-11-06)

| When (UTC+1) | Phase | Event |
|---|---|---|
| 11:05 | 7 | gmp-devel installed by the user; GnuCOBOL 3.2 compiles the original batch programs |
| 11:28 | 7 | `rust/difftest/run.py`: 13 batch programs, all outputs identical after fixes (commit fa42553) |
| 11:31 | 7 | clippy clean (all targets); multi-session tests (commit 401d457) |
| 11:40 | 6b | manual test suite executed, 41/41 pass; execution log filled (commit cc5fb5d) |
| 11:42 | 8 | deck updated with session 2 results and metrics |

Findings:
- Fidelity gaps fixed (found by the differential test): storage without VALUE must be binary zeros; INITIALIZE
  must skip FILLER/REDEFINES; CBSTM03A HTML used a 1-space delimiter (original: 2); CBCUS01C/CBACT01C missed a
  DISPLAY; CBIMPORT missed the "Errors Written" line.
- Fidelity gaps fixed (found by executing the suite): COTRN00C browse key for a non-numeric search (X'00', not
  spaces); account messages: two spaces before `Resp:` and cut at 75 characters.
- KD-12: CBEXPORT/CBIMPORT name a WORKING-STORAGE field as RECORD KEY; they do not compile as shipped.
- Source note: CUSTREC.cpy/CSLKPCDY.cpy contain tabs (width 4 intended).
- Accepted GnuCOBOL differences (normalised in the harness): DISPLAY of signed fields, INITIALIZE sign, X'00'
  in SYSOUT, real hundredths under COB_CURRENT_DATE.

Totals after session 2 (tools/usage.py): ≈1.6 h active, 374 turns, 399 tool calls, 504k output tokens,
132.2M cache-read tokens.

## Session 3 (2026-11-06)

- Repository restructured: `cobol/` (original AWS CardDemo, unchanged: app, diagrams, samples, scripts, README)
  and `rust/` (the translation, formerly `carddemo-rs/`). All paths updated; tests, difftest and suite re-verified.
- `demo/demo.py`: standalone COBOL batch run, Rust batch run, file-by-file comparison, online screens.
- `docs/DEMO.md` rewritten for non-technical audiences (one after the other, side by side, online, proof).
- `CLAUDE.md`: layout, commands, code-review scope (`rust/` only), translation rules; root `README.md`.
- Presentation rewritten for senior management (AI-assisted modernization pilot and proposal), including what
  the original application does, CLAUDE.md, human oversight and Remote Control.

Totals after session 3 (tools/usage.py): ≈1.9 h active, ≈0.56M output tokens.

### Code review (2026-11-06, `/code-review medium`, scope `rust/` per CLAUDE.md)
All 10 findings fixed, with regression tests (`rust/crates/online/tests/review_fixes.rs`, vsam and edit unit tests):
- KD-13 (new defect of the original): COACTUPC's ACCT-UPDATE-RECORD has no ADDR-ZIP; group id lands at offset 102.
- Online writes now start from binary zeros (account/customer/card update, user add, transaction add, bill payment).
- Bill payment: DUPKEY on CXACAIX keeps the first card; NOTFND keeps zeros; STARTBR failure ends in AEIP like KD-10.
- Units of work: begin/commit/rollback errors are reported; tasks/steps do not run unprotected; the abend
  message only says "backed out" when it is true. File operations are atomic (savepoint).
- SYNCPOINT from PF3 goes through the task (releases READ UPDATE holds); floating-sign edit pictures fixed.
Re-verified: 21 test suites, clippy clean, difftest identical, manual suite 41/41.

## Session 4 (2026-11-06): the original online programs on a CICS stand-in

- `rust/cobridge/cicsprep.py` (CICS translator), own DFH copybooks, CEEDAYS in C; `rust/crates/cobridge` runs the
  17 original COBOL online programs on the project's CICS layer (libcob FFI, CANCEL after each task, zero-filled
  32 KB commarea).
- All online tests run on the COBOL programs too (`CARDDEMO_REGION=cobol`); `rust/difftest/online.py` compares
  every screen: 338/338 identical (2 accepted compiler-dependent cases).
- Found by it and fixed in Rust: 11 fidelity gaps (literals with double/leading spaces, "No input received"
  overwrites, protection and cursor rules, F-key attributes, X'00' dates, text length, CVV bytes) plus KD-14 and
  KD-15 reproduced. New defects of the original: KD-14 (admin menu), KD-15 (no-change test), KD-16 (reads past
  EIBCALEN).
- `carddemo online --cobol`, `demo.py online-cobol` / `online-compare`, DEMO.md side by side, TC-DIF-01 (42/42),
  NFR-15, deck updated.

Totals after session 4 (tools/usage.py): ≈3.1 h active, ≈0.8M output tokens.

### Code review (2026-11-06, `/code-review high`): all 10 findings fixed
- The bridge loads libcob at run time (dlopen): every build works without GnuCOBOL; no rpath problems.
- `COB_CURRENT_DATE` only with `CARDDEMO_NOW` (GnuCOBOL freezes the clock otherwise).
- COBOL stdout *and* stderr go to the sysout file (opened once, `cobridge::set_sysout`); the TUI restores the
  terminal if libcob exits.
- demo.py: always `cargo build` (no stale binary), online-compare shows progress and errors and builds only what
  it needs, module directory rule shared with the runtime and cicsprep (`CARDDEMO_COBOL_MODULES`).
- execute.py runs the scripted suite once (inside online.py, on Rust and COBOL); TC-DIF-01 logged as
  "Pass (step 2)", the manual step 1 is stated as not executed.

### Code review (2026-11-06, `/code-review xhigh`): all 15 findings fixed
- cicsprep never clears a user-chosen module directory (replaces only its own 19 files); paths outside the
  repository work; one module-directory rule (empty = unset, relative made absolute) in cicsprep/Rust, imported
  by demo.py and online.py.
- Exit handler restores stdout/stderr before the terminal and only while the TUI is on screen; panics in the
  bridge become an ASRA abend instead of crossing into libcob.
- Clock frozen for COBOL only when CARDDEMO_NOW parses (`online::common::fixed_now`).
- CARDDEMO_LIBCOB is exact (no silent fallback); loader errors are reported; no fd leak; libc crate constants.
- COBOL DISPLAY output in test runs goes to a sysout file; online.py's parser is the plain one, clears old
  results first, and has `--rust-only`; execute.py uses it as the single fallback; the suite summary reads
  "41 of 42 passed" plus the partly manual TC-DIF-01.

### Code review (2026-11-06, second `/code-review xhigh`): all 14 findings fixed
- A panic inside an EXEC CICS call no longer runs ratatui's panic hook (which restored the terminal while the UI
  kept running): the bridge wraps the hook once and only logs such panics to the sysout file; the whole CDCICS
  body is inside the catch, so no panic can cross into libcob.
- The sysout redirect is re-entrant (depth counter; only the outermost call saves and restores the descriptors).
- Online tests write the COBOL sysout to `cobol-sysout.txt`, so online.py no longer counts it as a test
  (manual_suite 34, signon 2 again); online.py fails loudly if it cannot clear its output; execute.py removes an
  old results.json first and checks online.py's exit code.
- Unit tests: the stdout redirect test is alone in its binary and compares device/inode; the module-directory
  test is a separate test binary and restores CARDDEMO_COBOL_MODULES.
- `check()` reports an unusable CARDDEMO_COBOL_MODULES; when libcob ends the process under the TUI, the exit
  handler names the sysout file; one clock (`cobol::clock`) for batch, online and the bridge; libc is a
  workspace dependency.
- Gate in CLAUDE.md and the deck: the manual suite reads "41 of 42 passed" plus TC-DIF-01 "Pass (step 2)".

### Code review (2026-11-06, third `/code-review xhigh`): all 12 findings fixed
- The bridge's panic hook really comes first now: the first COBOL call runs before `ratatui::init()`, which
  wrapped it, so the TUI installs it again after init (`cobridge::install_panic_hook`). A test
  (cobridge/tests/panic_hook.rs) checks both install orders; it fails without the second install.
- A panic in a bridge call sets EIBRESP/EIBRESP2 to 0 before the ASRA abend (HANDLE ABEND LABEL sees clean
  values); the hook logs thread, location and (RUST_BACKTRACE) a backtrace, and never panics itself.
- Exit handler: `restore_stdio()` tells whether a COBOL call was running, so the sysout file is named exactly
  when libcob ended the process (also before the UI starts), and never for a Rust panic; the path comes from
  cobridge (`sysout_path`). No second redirect while one is active (no depth counter).
- Module directory: one `try_modules_dir` for `modules_dir()` and `check()`; cicsprep makes the value
  absolute like Rust (no symlink resolution), so both name the same path.
- execute.py: a failed comparison run is kept in rust/difftest/out-online-failed and TC-DIF-01 logged as
  Fail; "Not run" only when cobc is missing.
- The bridge calls `cobol::clock` directly.

### Code review (2026-11-07, fourth `/code-review xhigh`): all 15 findings fixed
Most findings were in the previous rounds' own fixes, so this round removes layers instead of adding one:
- One panic hook, owned by the TUI: the TUI sets up raw mode and the alternate screen itself (ratatui's
  `init()` would add its hook) and its hook skips the terminal restore while `cobridge::in_cdcics()`. The
  bridge installs no hook any more; a bridge panic is printed by the standard hook (into the sysout file,
  with the usual backtrace rules). `tui_end()` restores once, whoever comes first (normal end, panic hook,
  exit handler), with ratatui's non-printing `try_restore`. Test in carddemo (fails without the check).
- The whole CDCICS body, including the ASRA handling of a panicked command, runs inside the guard; if that
  handling panics too, `run()` ends the task with ASRA.
- One locked sysout state (file, path, saved descriptors): starting and ending a redirect cannot interleave;
  `restore_stdio()` returns the sysout path when a redirect was active (the exit handler's message).
- Online tests: the COBOL sysout is `CARDDEMO_COBOL_SYSOUT` (online.py: next to the screen-log directory,
  not in it), else `target/tmp/cobol-sysout-<test>.log`, no shared /tmp file.
- execute.py: an old out-online-failed is removed first; without cobc the comparison is not attempted
  (TC-DIF-01 "Not run"); a failed comparison always leaves out-online-failed/output.txt.
- `as_cdcics_call` only with cobridge's `test-hooks` feature (carddemo's tests); no unused clock re-export.
- Checked by hand: the TUI (Rust and --cobol) in a pseudo-terminal enters and leaves the alternate screen
  once and exits cleanly.

### Code review (2026-11-07, fifth `/code-review xhigh`): all 11 findings fixed, sysout redesigned
- No more descriptor juggling per program call: `set_sysout` points the C streams `stdout`/`stderr`, which
  libcob uses for DISPLAY and its error messages, at the sysout file, once per process (one stream, switched
  with `freopen`: an `fclose`d stream made glibc abort at exit, "invalid stdio handle", since libcob keeps
  it). Rust output (TUI, libtest, panic messages) is not redirected at all. libcob writes a few messages
  straight to fd 2 (warning prefixes, its signal handler), so the TUI points fd 2 at the sysout file while
  it is on and puts it back in `tui_end()`. Removed: SysoutRedirect, saved descriptors, restore_stdio,
  the "ended in COBOL" inference; `cobridge::in_cobol()` says it directly.
- TUI: a guard from `tui_start()` takes the UI off on every way out, also when the setup fails after raw
  mode (checked with stdout on /dev/full: terminal restored; before, it stayed raw). The panic hook keeps
  the UI only for a panic the bridge will catch (`bridge_catches_panic`: the first one of a command; a
  second one means an abort) and puts that message into the sysout file instead of over the screen.
- Bridge panics tested through the real CDCICS entry point (test-hooks `panic_in_next_command`): the task
  ends with ASRA, the UI stays, the message is logged. The ASRA fallback sets the abend directly.
- online.py: a test run that ends early (libcob runtime error) gets a report section with the end of the
  COBOL sysout file (checked with CSUTLDTC.so removed: "module 'CSUTLDTC' not found").
- execute.py: docstring states when out-online-failed is used; an unreadable results.json is a failed run.
- `modules_dir()` back to one function; unused chrono dependencies removed.
- Checked in a pseudo-terminal: normal runs (Rust, --cobol) and a libcob runtime error under the TUI (exit 1,
  UI left once, terminal restored, message naming the sysout file, libcob's message in it).

### Code review (2026-11-07, sixth `/code-review xhigh`): all 13 findings fixed; platforms Linux and Windows
- Platforms (decided by the user): Linux and Windows; macOS and musl dropped (NFR-09 updated). The sysout
  capture (glibc's assignable `stdout`/`stderr`) and the COBOL bridge are Linux-only; on Windows cobridge
  builds and reports "Linux only". The Windows build had been broken since the bridge started using dlopen;
  it now builds and links with MinGW (`x86_64-pc-windows-gnu`, command in CLAUDE.md).
- The terminal handling moved to `crates/carddemo/src/term.rs`: the exit handler takes the UI off on any
  exit while it is on (the libcob message only when `cobridge::in_cobol()`); fatal-signal handlers (TERM,
  HUP, INT, QUIT, ABRT, SEGV, BUS, FPE, ILL; not ignored ones) restore the terminal with async-signal-safe
  calls and pass the signal on to libcob's or the default handler, so `kill` no longer leaves a raw terminal
  and libcob's "termination (signal)" reaches it. `cobridge::start()` initialises libcob before the UI;
  the UI starts before the first transaction. dup/dup2 failures are errors.
- `cobridge::note_panic()` (renamed: a hook calls it once per panic); `write_sysout` survives a poisoned
  lock; `send_to_sysout` does the dup2 under the lock; one flush of the sysout stream per task;
  `set_sysout` with the same path does nothing.
- execute.py keeps the --rust-only output (out-online/output.txt) and points failing cases at it; it still
  removes an old results.json itself (online.py may fail before it clears its output). online.py reports
  build errors as such.
- Tests clean up their files; one sample-data helper (`batch::repro::sample_store`, `sample_seed_dir`).
- Checked in a pseudo-terminal (Rust and --cobol): normal runs, SIGTERM and SIGHUP (terminal restored,
  libcob's message shown), a libcob runtime error, a setup failure.
- Windows checked under Wine 11.21: the whole test suite passes (the COBOL smoke tests are Linux-only),
  `carddemo.exe reset`/`batch` run, `online --cobol` refuses with "Linux only"; the full nightly batch from
  the same sample data gives the same job log, the same 8 output files byte for byte and identical contents
  in all 11 VSAM datasets as the Linux build.

### Code review (2026-11-07, seventh `/code-review xhigh`): all 7 findings fixed
- online.py: a test run counts as built when cargo started the test binary (cargo's "error: test failed"
  no longer makes a crash look like a build error); only "libcob:" lines this run added to the sysout file
  are shown. The classification is a function with unit tests (rust/difftest/test_online.py).
- The exit handler is installed before libcob starts (`term::install()`): a failing `cob_init` (broken
  COB_RUNTIME_CONFIG) is reported on the terminal instead of exiting silently.
- Fault signals (SEGV, BUS, FPE, ILL) raised by the kernel are not re-raised: the handler restores the
  previous action and returns, so the fault recurs with its real details and Rust's stack-overflow message
  appears. Signals that were sent are raised again.
- A panic the hook misjudged (`note_panic`) no longer breaks the session: `term::ensure_on` puts the UI back
  before the next screen if it was taken off while the program goes on.
- `cobridge::init()` returns a Result (no panic inside a Once); `run()` turns it into an APCT abend.
- New tests/terminal.rs (carddemo): the real binary in a pseudo-terminal - quit, SIGTERM/SIGHUP, failed
  setup, stack overflow (debug trigger), COBOL init failure, libcob runtime error under the UI. Each of the
  two new cases fails with the reviewed bug put back. Windows tests under Wine pass as well.

### Code review (2026-11-07, eighth `/code-review xhigh`, last round for now): all 13 findings fixed
- Signal handler restores the terminal crossterm uses (stdin if a terminal, else /dev/tty); a three-state UI
  flag (off/on/ending) closes the windows before raw mode and during the restore; every signal whose
  default action ends the process is handled (19, not the real-time ones); a bridge panic that cannot
  unwind leaves a note that the SIGABRT handler prints.
- `ensure_on` kept, now tested (tests/terminal.rs: UI taken off mid-session comes back); the cobridge test
  seams are debug-build-only (no `test-hooks` feature; release binaries contain none); `start()` is the one
  initialisation; the helper is `in_cobol_call`; sysout line-buffered and close-on-exec (tested against the
  interleaving); COBOL-region tests stop with check()'s message; terminal tests wait for the screen
  instead of sleeping, with default signal handling; online.py: rustc's `-->` at any indent, no dead offset.

### Release 1.0.0: downloadable demos for Windows and Linux
- `carddemo` without arguments (a double click on Windows) shows a demo menu: online screens, nightly
  batch, transaction report (default range up to today: posting stamps today's date), output-file viewer
  (records split, HTML statements in the browser), reset, and on Linux with GnuCOBOL 3.2 the original COBOL
  screens from the shipped modules. Data next to the program; sample data found in `data/EBCDIC` beside it.
- `rust/tools/package-release.sh` builds `carddemo-1.0.0-windows-x86_64.zip` (only Windows system DLLs) and
  `carddemo-1.0.0-linux-x86_64.tar.gz` (glibc >= 2.34; the 2.39 symbols are weak), each with the sample
  data, `docs/demo-quickstart.pdf` (new), LICENSE and NOTICE. DEMO.md, the deck and the README point to it.
- Checked from the unpacked packages: Linux in a pseudo-terminal (batch, report, viewer, online screens,
  original COBOL screens), Windows under Wine (the same without COBOL); tests/terminal.rs runs the menu in
  the release layout.

### 2026-11-07: README, docs clean-up, Windows icon, code review (high effort), release 1.0.0 updated
- README: where the COBOL fork comes from (aws-samples/aws-mainframe-modernization-carddemo at 59cc6c2), what the
  Rust project is and achieved, the documents (Markdown and PDF); AWS's original README kept below it.
- Docs: the finished online-COBOL plan removed (stale, unreferenced); deck figures updated (5.9 h, 1.2M output
  tokens, requirements 28 pages / about 170 FRs). Rust: debug prints removed, BMS maps read from cobol/ (the
  copies in rust/assets removed), module docs, long lines wrapped.
- Windows: carddemo.exe has an icon (a card with a terminal prompt) and version information (build.rs, windres).
- A high-effort review agent (read-only) reported 1 high, 4 medium and 12 low findings. Fixed: the signal
  handlers are put back on every UI start (libcob's start replaced them when the COBOL screens followed the
  Rust screens in the demo menu); the text-screen cursor no longer underflows; the help line fits 80 columns;
  `online --cobol` finds the modules shipped next to the program; Rust 1.88 declared as the minimum (checked).
  The low-priority findings are left for review by the user.
- Release 1.0.0 rebuilt with stripped programs (Linux 4.8 MB, Windows 6.1 MB) and the icon, tested from the
  packages on Linux (pty, incl. the original COBOL screens) and Windows (Wine).

### 2026-11-07: architecture and API documents, derived from the code
- `docs/architecture/cobol.md` -> `architecture-cobol.pdf` (13 pages): the original's subsystems, transactions and
  programs with their screens and file access, navigation and menu tables, the commarea (`COCOM01Y`) with
  offsets, CICS resources, job streams, batch program interfaces (DD names, organisation, keys, records),
  called routines, VSAM clusters and alternate indexes, sequential data sets and GDGs, record layouts,
  relationships, external interfaces.
- `docs/architecture/rust.md` -> `architecture-rust.pdf` (9 pages): crates and dependencies, the public API of
  each crate, the COBOL bridge (translation scheme, EIB layout), runtime flows, the COBOL-to-Rust mapping,
  storage, verification, build and release.
- Both state on the title page and in their first chapter that they were derived from the source code (commit
  49ee1a5), with extraction scripts for the CSD, `EXEC CICS` commands, `SELECT ... ASSIGN`, `DEFINE CLUSTER` and
  JCL steps, and that the code is authoritative where they differ. Claims were checked against the code; for
  example `CARDAIX` is defined but used by no `EXEC CICS` command, and the batch runner records return codes
  but does not evaluate the JCL's `COND`.

### 2026-11-08: paragraph coverage, differential fuzzing, CI
- `rust/difftest/coverage.py`: the comparisons enter 551 of 599 COBOL paragraphs (92%). The other 48 each have a
  reason checked in the source (abend paths after file errors, dead code); `crates/online/tests/coverage_sessions.rs`
  added 4 sessions for the gaps. The script fails on an uncovered paragraph without a reason.
- `rust/difftest/fuzz.py`: random DALYTRAN files through `run.py` (3 seeds of 400 records) and random keystroke
  sessions through `online.py` (14 seeds of about 400 screens): all identical once the bugs below were fixed.
- Found by fuzzing and fixed in Rust (regression tests in `review_fixes.rs`):
  - COCRDSLC: `*` in the account field counts as not supplied (the literal is padded for the comparison).
  - COUSR03C: delete goes on after a failed read, so INVREQ gives "Unable to Update User...".
  - COACTUPC 3203-SHOW-UPDATED-VALUES: a money field that is not valid shows what was typed.
  - COACTUPC POSITION CURSOR: the order of the original's EVALUATE, not screen order.
  - COACTUPC PF12: 3202-SHOW-ORIGINAL-VALUES clears WS-NON-KEY-FLAGS, but the SSN flags are outside that group.
  - CSUTLDPY (COACTUPC): EDIT-MONTH and EDIT-DAY rewrite the date as digits before CEEDAYS, a month with a leading
    space is valid, and a day goes through an unsigned item, so its sign is dropped.
  - COCRDUPC POSITION CURSOR: the BLANK flags count as well as NOT-OK.
- Compiler-dependent, not fixed: COBIL00C's unchecked account id (FR-BIL-01), the menu OPTION and a lone `0` in search
  fields are left out of the fuzzing vocabulary; the transaction amount of the X'00' record also shows on transaction
  add (accepted difference 1 of `online.py`).
- Online comparison now 399 screens of 57 sessions.
- `.github/workflows/ci.yml`: Linux, Windows and the COBOL comparisons (GnuCOBOL 3.2 built from source and cached).

### 2026-11-08: low-priority review findings
Fixed: the exit key in demo.py's hint (Ctrl+Q); package-release.sh takes the modules from `cicsprep.modules_dir()` and no
longer rewrites the tracked quick-start PDF; the file viewer uses each data set's record length (and a line-end byte
inside a record, as in EXPORT.DATA, no longer makes it a text file) and shows no extra last line; one list of nightly jobs;
an explicit `--home carddemo-data` is honoured by the demo menu; terminal tests ignore `CARDDEMO_*`/`COB_*` from the
developer's shell, except where GnuCOBOL's parts are (`CARDDEMO_LIBCOB`, `CARDDEMO_COBOL_MODULES`); `lookups.rs` no longer
claims to be generated; rust/README's command list; the quick-start guide no longer says a wrong libcob version is
detected; `ast.literal_eval` instead of `eval` in run.py.
Left on purpose (user's decision): the hard-coded transcript folder of usage.py and phase_metrics.py, and more unit tests
for the online screen's key handling and drawing. The release downloads are rebuilt after the last round of code changes.
