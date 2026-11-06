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
