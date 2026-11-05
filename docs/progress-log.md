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
