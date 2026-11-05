---
title: "CardDemo — Software Requirements Specification"
subtitle: "Derived from the COBOL/CICS/VSAM source for the Rust re-implementation"
date: "2026-11-05 — version 1.1"
author: "Prepared with Claude Code"
toc: true
toc-depth: 2
numbersections: true
geometry: margin=2cm
fontsize: 10pt
mainfont: "Noto Serif"
sansfont: "Noto Sans"
monofont: "JetBrains Mono"
colorlinks: true
header-includes:
  - \usepackage{longtable}
  - \usepackage{booktabs}
  - \renewcommand{\arraystretch}{1.15}
  - \usepackage{newunicodechar}
  - \newunicodechar{→}{\ensuremath{\rightarrow}}
  - \newunicodechar{≤}{\ensuremath{\leq}}
  - \newunicodechar{−}{\ensuremath{-}}
  - \newunicodechar{×}{\ensuremath{\times}}
---

# Introduction

## Purpose

This document gives the functional and non-functional requirements of the **CardDemo**
credit-card management application. It was produced by reverse-engineering the original
mainframe source code (COBOL, CICS BMS maps, copybooks, JCL and assembler) in the
repository `aws-mainframe-modernization-carddemo`.

It serves three purposes:

1. It is the specification for the Rust re-implementation (`carddemo-rs`).
2. It is the basis of the manual test suite. Every test case traces to a requirement ID
   defined here.
3. It is a record of how the system behaves today, **including its defects**. The
   re-implementation keeps those defects on purpose (they are listed as *KD-nn*), so
   that the old and new systems can be compared output for output.

## Scope

The scope is the **base application** only:

- 17 online CICS programs
- 13 batch COBOL programs plus 2 assembler routines
- the VSAM data model
- the batch job streams

The optional modules are **out of scope**: credit-card authorisations
(IMS/DB2/MQ), transaction type management (DB2), and the MQ account/date inquiry.
Where a base program refers to an optional module, these requirements describe what
the base program does when that module is *not installed*:

- main menu option 11
- admin menu options 5–6

## Conventions

| Prefix | Meaning |
|---|---|
| `BR-…` | Business or behavioural rule shared by several functions |
| `FR-<AREA>-nn` | Functional requirement |
| `NFR-nn` | Non-functional requirement |
| `KD-nn` | Known defect of the original, kept for fidelity |

Text in `monospace` is literal: messages exactly as the user sees them, field values,
file and program names. Every requirement names its source program, so it can be
traced back to the code.

## System overview

| Area | Transactions / jobs | Programs |
|---|---|---|
| Security | CC00 sign-on; CU00–CU03 user list/add/update/delete | COSGN00C, COUSR00C–03C |
| Navigation | CM00 main menu; CA00 admin menu | COMEN01C, COADM01C |
| Accounts | CAVW view; CAUP update | COACTVWC, COACTUPC |
| Cards | CCLI list; CCDL view; CCUP update | COCRDLIC, COCRDSLC, COCRDUPC |
| Transactions | CT00 list; CT01 view; CT02 add; CR00 reports; CB00 bill pay | COTRN00C–02C, CORPT00C, COBIL00C |
| Batch | POSTTRAN, INTCALC, TRANREPT, CREASTMT, COMBTRAN, READxxxx, CBEXPORT/CBIMPORT, WAITSTEP, file (re)load jobs | CBTRN01C–03C, CBACT01C–04C, CBCUS01C, CBSTM03A/B, CBEXPORT, CBIMPORT, COBSWAIT, CSUTLDTC, COBDATFT, MVSWAIT |

The two user roles are **Admin** (`A`), who lands on the admin menu after sign-on, and
**User** (`U`), who lands on the main menu.

## Data stores

| Store (CICS file) | Organisation | Key | Record | Length |
|---|---|---|---|---:|
| USRSEC | KSDS | user id (8) | SEC-USER-DATA (CSUSR01Y) | 80 |
| ACCTDAT | KSDS | account id 9(11) | ACCOUNT-RECORD (CVACT01Y) | 300 |
| CARDDAT | KSDS + AIX CARDAIX on account id | card number (16) | CARD-RECORD (CVACT02Y) | 150 |
| CUSTDAT | KSDS | customer id 9(9) | CUSTOMER-RECORD (CVCUS01Y) | 500 |
| CCXREF | KSDS + AIX CXACAIX on account id | card number (16) | CARD-XREF-RECORD (CVACT03Y) | 50 |
| TRANSACT | KSDS (+ AIX on proc timestamp) | tran id (16) | TRAN-RECORD (CVTRA05Y) | 350 |
| TCATBALF | KSDS | acct+type+cat (17) | TRAN-CAT-BAL-RECORD (CVTRA01Y) | 50 |
| DISCGRP | KSDS | group+type+cat (16) | DIS-GROUP-RECORD (CVTRA02Y) | 50 |
| TRANTYPE | KSDS | type (2) | TRAN-TYPE-RECORD (CVTRA03Y) | 60 |
| TRANCATG | KSDS | type+cat (6) | TRAN-CAT-RECORD (CVTRA04Y) | 60 |
| DALYTRAN | sequential | — | DALYTRAN-RECORD (CVTRA06Y) | 350 |

The full field-level layout of every record is in the *Data Dictionary* chapter.
