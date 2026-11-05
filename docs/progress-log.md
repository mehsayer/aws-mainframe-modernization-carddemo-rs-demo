# Progress Log — CardDemo COBOL → Rust

Metrics come from `carddemo-rs/tools/usage.py`, which reads the Claude Code session transcripts.

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
Per-phase breakdown: `python3 carddemo-rs/tools/phase_metrics.py`.
