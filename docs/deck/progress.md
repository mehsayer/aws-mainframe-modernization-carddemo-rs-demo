---
title: "CardDemo: COBOL → Rust"
subtitle: "Project progress report: sessions 1 and 2"
author: "Prepared with Claude Code (Opus 5.5)"
date: "2026-11-06"
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

## Where we are {.shrink}

::: columns
:::: {.column width="52%"}
**Session 1**

- Requirements specification (PDF)
- Rust workspace, 6 crates, **all 30 base programs**
  (17 CICS online + terminal UI, 13 batch + JCL runner)
- Manual test suite (PDF, 100% FR-group coverage)
- Live-demo script, CLI, one-command reset

**Session 2**

- **Differential test vs. the original COBOL** (GnuCOBOL): 13 batch programs, **all outputs identical**
- **Manual test suite executed: 41 / 41 pass**
- Clippy clean; multi-terminal tests
::::
:::: {.column width="45%"}
**Found and fixed in session 2**

- 9 fidelity gaps in the Rust port, all invisible to the port's own tests (details on the next slides)
- 1 new defect in the original: KD-12
- 8 test-case texts corrected against the COBOL

**Open**

- Optional modules (IMS/DB2/MQ): out of scope by decision
- Live demo rehearsal
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
| 7 | Hardening, demo | runbook | demo script, README; GnuCOBOL diff test (13 programs, identical); clippy; locking tests |
| 6b | Test execution | — | 41 / 41 pass (scripted terminal + difftest) |
| 8 | Progress deck | this document | updated after session 2 |

## Effort by phase (output tokens)

\begin{center}
\begin{tikzpicture}
\begin{axis}[
  xbar, width=12cm, height=6.4cm, bar width=7pt,
  xlabel={output tokens (thousands)}, xmin=0,
  symbolic y coords={S2 test execution,S2 hardening,S2 difftest,Docs v1.1+tests,Binary/TUI 4f,Cards+accounts 4e,Transactions 4d,Users 4c,Sign-on/menus 4b,CICS runtime 4a,Batch misc 3d,Statements 3c,Interest/report 3b,Posting 3a,VSAM 2b,COBOL types 2a,Analysis+reqs 0-1},
  ytick=data, yticklabel style={font=\scriptsize},
  nodes near coords, nodes near coords style={font=\tiny}, enlarge y limits=0.06]
\addplot coordinates {(71.0,Analysis+reqs 0-1) (31.8,COBOL types 2a) (13.9,VSAM 2b) (16.4,Posting 3a) (20.5,Interest/report 3b) (15.0,Statements 3c) (18.3,Batch misc 3d) (22.9,CICS runtime 4a) (12.1,Sign-on/menus 4b) (16.8,Users 4c) (35.3,Transactions 4d) (63.2,Cards+accounts 4e) (16.8,Binary/TUI 4f) (14.3,Docs v1.1+tests) (74.1,S2 difftest) (9.2,S2 hardening) (44.0,S2 test execution)};
\end{axis}
\end{tikzpicture}
\end{center}

## Time and tokens (from the Claude Code transcripts)

| Metric | Session 1 | Session 2 | Total |
|---|--:|--:|--:|
| Active time (gaps > 10 min excluded) | 1.1 h | 0.5 h | 1.6 h |
| Assistant turns / tool calls | 194 / 212 | 180 / 187 | 374 / 399 |
| Output tokens | 367k | 137k | 504k |
| Prompt cache reads | 90.9M | 41.3M | 132.2M |
| Prompt cache writes | 0.76M | 0.32M | 1.08M |
| Commits on branch `rust-port` | 15 | 3 | 18 |

Measured with `carddemo-rs/tools/usage.py` and `tools/phase_metrics.py`. Cache reads dominate: every turn re-reads the conversation.

# Quality and fidelity

## Testing

- **Unit tests:** numerics (zoned/COMP-3/binary), edit pictures, NUMVAL, cp037, VSAM browse rules, BMS parser, CEEDAYS dates
- **Record tests:** generated layouts decode the original EBCDIC sample data
- **Batch:** the full nightly cycle (TRANBKP → POSTTRAN → INTCALC → COMBTRAN → TRANREPT → CREASTMT); CBEXPORT → CBIMPORT is byte-identical
- **Online:** scripted 3270 journeys through all 17 programs (sign-on, admin, lists, updates, payments, report submission)
- **Multi-terminal:** a concurrent update is detected; parallel payments get unique transaction ids
- **Manual suite:** executed as one scripted test per case: **41 / 41 pass**
- **Result:** all tests pass (`cargo test --workspace`), clippy clean

## Differential test against the original COBOL

| Job / program | Outputs compared | Result |
|---|---|---|
| POSTTRAN / CBTRN02C | TRANSACT, ACCTDATA, TCATBALF, DALYREJS, RC, SYSOUT | identical |
| INTCALC / CBACT04C | ACCTDATA, SYSTRAN, RC, SYSOUT | identical |
| TRANREPT / CBTRN03C | report (519 lines), RC, SYSOUT | identical |
| CREASTMT / CBSTM03A+B | statement text + HTML (7.9k lines), RC | identical |
| READACCT/CARD/XREF/CUST, DALYVRFY | OUTFILE (COMP-3), ARRYFILE, VB file, SYSOUT | identical |
| CBEXPORT / CBIMPORT | export file, 6 import outputs (incl. error path), SYSOUT | identical |

The original sources were compiled unchanged with GnuCOBOL 3.2, except for three patches: CBSTM03A's z/OS control-block walk is stubbed out, the KD-12 FD is fixed, and tabs are expanded. Both sides use the same input snapshots and a fixed clock. The documented platform differences (DISPLAY sign style, INITIALIZE sign) are normalised.
`python3 carddemo-rs/difftest/run.py`

## What the comparison and the test run found

| Area | Rust port before | Original behaviour (now matched) |
|---|---|---|
| Storage without VALUE | spaces | binary zeros: FILLER bytes of posted/interest transactions, new TCATBAL records, import outputs |
| INITIALIZE | blanked FILLER | leaves FILLER and REDEFINES alone |
| CBSTM03A HTML | `DELIMITED BY ' '` | `DELIMITED BY '  '`: full names and addresses |
| CBCUS01C, CBACT01C | one DISPLAY per record | a second DISPLAY in the main loop |
| CBIMPORT | no "Errors Written" line | statistics line present |
| COTRN00C search | key = spaces → NOTFND | key = X'00' → finds the init record |
| Account messages | 1 space, cut at 78 | `file.  Resp:`, cut at 75 (WS-RETURN-MSG) |

## Defects found in the original (kept on purpose)

| ID | Where | Effect |
|---|---|---|
| KD-01 | INTCALC | the last account's interest is never written back |
| KD-02/03 | TRANREPT | last amount counted twice; no final account total |
| KD-07 | card update | the CVV is reset to 000 |
| KD-08 | account update | the "blank phone" test checks the area code twice |
| KD-10 | user/tran list | search past the last key → AEIP abend |
| KD-11 | user/tran list | PF7 then PF8 on page 1 → empty page |
| KD-12 | CBEXPORT/CBIMPORT | RECORD KEY outside the file's record: does not compile as shipped |

Also: the COACTVW map blanks most of the `Tran:` header, and the sample data fails the
COACTUPC state/zip check for 48 of 50 customers.

## Lessons learned

- **Read the source, never a filtered view.** A space-collapsing reader and a grep filter both
  produced wrong requirements (caught and fixed: KD-06 was withdrawn).
- **COBOL semantics hide in ordering.** For example `SET NEXT-PAGE-NO` *before* the commarea MOVE.
- **The 3270 matters:** FSET fields, nulls versus spaces, overlapping map fields.
- **Generate from the originals:** copybooks → Rust records, BMS → screens, CSLKPCDY → lookups.
- **Run the original.** Unit and journey tests check what we *think* the code does. The GnuCOBOL comparison found
  6 kinds of gap at once, and executing the suite found 3 more. Most were about storage nobody writes: FILLERs and fields without VALUE.
- **Execute the test suite, don't just write it:** 8 of 41 case texts had wrong expectations.

# Next steps

## Next steps

1. Live demo rehearsal (`docs/DEMO.md`); `carddemo-rs/difftest/run.py` can be part of the demo
2. Optional: an online differential test (needs a CICS emulator; out of scope so far)
3. Optional modules (IMS / DB2 / MQ) if the scope is extended

## Demo

```text
carddemo reset          # 5 ms
carddemo online         # sign on as USER0001 / PASSWORD
carddemo batch          # nightly cycle in about 20 ms
```

Branch `rust-port`: `docs/requirements.pdf`, `docs/manual-test-suite.pdf`, `docs/DEMO.md`.
