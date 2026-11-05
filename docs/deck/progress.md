---
title: "CardDemo: COBOL → Rust"
subtitle: "Project progress report: session 1"
author: "Prepared with Claude Code (Opus 5.5)"
date: "2026-11-05"
theme: metropolis
aspectratio: 169
fontsize: 9pt
mainfont: "Noto Sans"
monofont: "JetBrains Mono"
header-includes:
  - \usepackage{pgfplots}
  - \pgfplotsset{compat=1.18}
  - \usepackage{newunicodechar}
  - \newunicodechar{→}{\ensuremath{\rightarrow}}
  - \newunicodechar{≈}{\ensuremath{\approx}}
  - \newunicodechar{×}{\ensuremath{\times}}
  - \metroset{block=fill}
---

# Status at a glance

## Where we are

::: columns
:::: {.column width="52%"}
**Done in session 1**

- Requirements specification (PDF, v1.1)
- Rust workspace, 6 crates, **all 30 base programs**
  - 17 CICS online programs + terminal UI
  - 13 batch programs + JCL job runner
- Manual test suite (PDF, 40 cases, 100% FR-group coverage)
- Live-demo script, CLI, one-command reset
::::
:::: {.column width="45%"}
**Open**

- Differential tests against GnuCOBOL: *blocked*, the `gmp-devel` headers are missing
- Optional modules (IMS/DB2/MQ): out of scope by decision
- Presentation of session 2 results
::::
:::

# Scope and approach

## Scope (agreed 2026-11-05)

| In scope | Out of scope |
|---|---|
| Base app: COBOL, CICS, VSAM, JCL, assembler | Authorisations (IMS / DB2 / MQ) |
| 20.6k lines COBOL, 2.8k copybook, 4.5k BMS | Transaction types (DB2) |
| Terminal UI (3270 style), SQLite, fixed-width I/O | MQ account and date inquiry |

Decisions: faithful translation (same messages, layouts, bytes, **and defects**);
GnuCOBOL for batch differential tests; live demo planned.

## Architecture

| Crate | Replaces | Lines |
|---|---|--:|
| `cobol` | COBOL data semantics: zoned, COMP-3, edit pictures, cp037 + generated copybooks | 1,677 |
| `vsam` | VSAM KSDS/AIX on SQLite: EBCDIC key order, CICS browse rules | 556 |
| `cics` | BMS maps parsed from the original sources; XCTL / RETURN, file control, TDQ | 833 |
| `online` | the 17 CICS programs, one function per COBOL paragraph | 4,283 |
| `batch` | 13 batch programs + JCL jobs (SORT, REPRO, GDG, internal reader) | 1,886 |
| `carddemo` | binary: ratatui TUI, `job`, `batch`, `reset`, `script` | 421 |

About **9.7k lines of Rust** (plus 1.1k generated) replace about **27k lines** of COBOL, copybooks and BMS.

# Plan versus actual

## Phases

| # | Phase | Plan | Actual |
|--:|---|---|---|
| 0 | Inventory | call graph, CRUD matrix | done, scripted extraction |
| 1 | Requirements PDF | FR / NFR / traceability | 26 pp, 165 FR, 12 NFR, 10 KD |
| 2 | Foundation crates | types, records, store | done; records generated from copybooks |
| 3 | Batch | 13 programs + jobs | done; export/import round trip byte-identical |
| 4 | Online + TUI | 17 programs | done; scripted terminal journeys pass |
| 5 | Optional modules | — | dropped (scope decision) |
| 6 | Manual test PDF | per requirement | 40 cases, 122 steps |
| 7 | Hardening, demo | runbook | demo script, README; diff tests open |
| 8 | Progress deck | this document | — |

## Effort by phase (output tokens)

\begin{center}
\begin{tikzpicture}
\begin{axis}[
  xbar, width=12cm, height=6.4cm, bar width=7pt,
  xlabel={output tokens (thousands)}, xmin=0,
  symbolic y coords={Docs v1.1+tests,Binary/TUI 4f,Cards+accounts 4e,Transactions 4d,Users 4c,Sign-on/menus 4b,CICS runtime 4a,Batch misc 3d,Statements 3c,Interest/report 3b,Posting 3a,VSAM 2b,COBOL types 2a,Analysis+reqs 0-1},
  ytick=data, yticklabel style={font=\scriptsize},
  nodes near coords, nodes near coords style={font=\tiny}, enlarge y limits=0.06]
\addplot coordinates {(71.0,Analysis+reqs 0-1) (31.8,COBOL types 2a) (13.9,VSAM 2b) (16.4,Posting 3a) (20.5,Interest/report 3b) (15.0,Statements 3c) (18.3,Batch misc 3d) (22.9,CICS runtime 4a) (12.1,Sign-on/menus 4b) (16.8,Users 4c) (35.3,Transactions 4d) (63.2,Cards+accounts 4e) (16.8,Binary/TUI 4f) (14.3,Docs v1.1+tests)};
\end{axis}
\end{tikzpicture}
\end{center}

## Time and tokens (from the Claude Code transcripts)

| Metric | Value |
|---|--:|
| Active session time (gaps > 10 min excluded) | ≈ 1.1 h |
| Assistant turns / tool calls | 194 / 212 |
| Output tokens | 367k |
| Input tokens (uncached) | 0.4k |
| Prompt cache writes / reads | 0.76M / 90.9M |
| Peak context window fill | ≈ 80% of 1M |
| Commits on branch `rust-port` | 15 |

Measured with `carddemo-rs/tools/usage.py` and `tools/phase_metrics.py`. Cache reads dominate: every turn re-reads the conversation.

# Quality and fidelity

## Testing

- **Unit tests:** numerics (zoned/COMP-3/binary), edit pictures, NUMVAL, cp037, VSAM browse rules, BMS parser, CEEDAYS dates
- **Record tests:** generated layouts decode the original EBCDIC sample data
- **Batch:** the full nightly cycle (TRANBKP → POSTTRAN → INTCALC → COMBTRAN → TRANREPT → CREASTMT); CBEXPORT → CBIMPORT is byte-identical
- **Online:** scripted 3270 journeys through all 17 programs (sign-on, admin, lists, updates, payments, report submission)
- **Result:** all tests pass (`cargo test --workspace`)
- **Pending:** byte-level comparison with the original batch programs under GnuCOBOL

## Defects found in the original (kept on purpose)

| ID | Where | Effect |
|---|---|---|
| KD-01 | INTCALC | the last account's interest is never written back |
| KD-02/03 | TRANREPT | last amount counted twice; no final account total |
| KD-07 | card update | the CVV is reset to 000 |
| KD-08 | account update | the "blank phone" test checks the area code twice |
| KD-10 | user/tran list | search past the last key → AEIP abend |
| KD-11 | user/tran list | PF7 then PF8 on page 1 → empty page |

Also: the COACTVW map blanks most of the `Tran:` header, and the sample data fails the
COACTUPC state/zip check for 48 of 50 customers.

## Lessons learned

- **Read the source, never a filtered view.** A space-collapsing reader and a grep filter both
  produced wrong requirements (caught and fixed: KD-06 was withdrawn).
- **COBOL semantics hide in ordering.** For example `SET NEXT-PAGE-NO` *before* the commarea MOVE.
- **The 3270 matters:** FSET fields, nulls versus spaces, overlapping map fields.
- **Generate from the originals:** copybooks → Rust records, BMS → screens, CSLKPCDY → lookups.

# Next steps

## Session 2 plan

1. Install `gmp-devel` → GnuCOBOL differential tests for POSTTRAN, INTCALC, TRANREPT, CREASTMT
   (and verify the NEXT SENTENCE semantics in CBTRN03C)
2. Execute the manual test suite; record results in the execution log
3. Hardening: clippy, multi-terminal locking test, EBCDIC export of the export file
4. Live demo rehearsal (`docs/DEMO.md`)
5. Updated deck with measured results of session 2

## Demo

```text
carddemo reset          # 5 ms
carddemo online         # sign on as USER0001 / PASSWORD
carddemo batch          # nightly cycle in about 20 ms
```

Branch `rust-port`: `docs/requirements.pdf`, `docs/manual-test-suite.pdf`, `docs/DEMO.md`.
