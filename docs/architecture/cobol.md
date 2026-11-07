---
title: "CardDemo (COBOL) — Architecture and Interfaces"
subtitle: "Derived from the COBOL/CICS/VSAM/JCL source code of the original application"
date: "2026-11-07 — version 1.0"
author: "Illia Burov (prepared with Claude Code)"
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
  - \newunicodechar{←}{\ensuremath{\leftarrow}}
  - \newunicodechar{↔}{\ensuremath{\leftrightarrow}}
  - \newunicodechar{×}{\ensuremath{\times}}
  - |
    ```{=latex}
    \usepackage{fvextra}
    \usepackage{needspace}
    \RecustomVerbatimEnvironment{Highlighting}{Verbatim}{breaklines,commandchars=\\\{\},fontsize=\footnotesize}
    \makeatletter\def\verbatim@font{\ttfamily\footnotesize}\makeatother
    ```
---

# Introduction

## Purpose

This document describes the architecture and the interfaces of the original **CardDemo** mainframe
application: its subsystems, the CICS transactions and programs, the screens, the communication area that
the online programs pass to each other, the batch job streams, the interfaces of the batch programs, and
the data sets and record layouts they share.

It is a companion to the *Software Requirements Specification* (`requirements.pdf`), which describes *what*
the system does; this document describes *how it is built*. The Rust translation has its own document
(`architecture-rust.pdf`).

## How this document was made: derived from the code

**This document was derived from the source code; it is not based on separate design documents.** No
architecture or interface documentation was supplied with the original application. Every statement here
was obtained by reading the source in `cobol/app/` — COBOL programs (`cbl/`), copybooks (`cpy/`), BMS map
definitions (`bms/`), JCL (`jcl/`), the CICS resource definitions (`csd/CARDDEMO.CSD`) and the assembler
routines (`asm/`) — partly with small extraction scripts (CICS commands, `SELECT ... ASSIGN` clauses, `DEFINE
CLUSTER` statements, `EXEC PGM=` steps), and checked against the code. The source was taken from the
repository at commit `49ee1a5` (7 November 2026); the `cobol/` folder there is AWS CardDemo as forked from
`aws-samples/aws-mainframe-modernization-carddemo` at commit `59cc6c2`, unchanged.

Consequences for the reader:

- **Where this document and the code disagree, the code is right.** The document records the code as it is,
  including oddities and defects (the requirements list the known defects as *KD-nn*); it does not say how
  the system was *meant* to work.
- Names (programs, transactions, files, fields, paragraphs) are quoted exactly as they appear in the code.
- Statements about run-time behaviour (for example navigation between screens) are read from the program
  logic; they were confirmed by running the original programs with GnuCOBOL on a CICS stand-in (see the Rust
  architecture document), not on a mainframe.

## Scope

The **base application**: 17 online CICS programs, 13 batch COBOL programs, the wait utility `COBSWAIT`,
2 assembler routines, 17 BMS mapsets, 30 copybooks, 38 JCL members and the CSD. The optional modules in
`cobol/app/app-*` (credit-card authorisations with IMS/DB2/MQ, transaction-type maintenance with DB2, MQ
account and date inquiry) are outside the scope, as in the requirements; where base code refers to them, this
document says so.

# System overview

CardDemo is a credit-card management system with two subsystems sharing one set of VSAM files:

| Subsystem | Runs under | Made of | Users |
|---|---|---|---|
| **Online** | CICS, 3270 terminals | 17 pseudo-conversational COBOL programs with BMS maps | Customer-service users (type `U`) and administrators (type `A`) |
| **Batch** | z/OS JES, JCL | Job streams of COBOL programs, IDCAMS, DFSORT, IEBGENER, IEFBR14 | Operations (scheduled, for example nightly) |
| **Data** | VSAM, QSAM | KSDS clusters with alternate indexes; sequential files and GDGs | Both |

```{=latex}
\needspace{13\baselineskip}
```

```text
 3270 terminal
      │ BMS screens
      ▼
 CICS region (CSD CARDDEMO): 18 transactions, COxxxxxC programs ── WRITEQ TD ──► queue JOBS
      │ EXEC CICS READ, WRITE, REWRITE, DELETE, STARTBR, ...                         │ internal
      ▼                                                                               │ reader
 VSAM KSDS clusters and alternate indexes ◄────────┐                                 ▼
                                                   │ SELECT ... ASSIGN TO <DD>   TRANREPT job
 JCL job streams ──► CBxxxxxC programs, IDCAMS, SORT ──► sequential data sets, GDGs,
 (scheduler, operator)                                     reports, statements
```

The two subsystems meet in two places only: the shared VSAM files (the online programs read and update the
same master files that batch posts to), and the transient-data queue `JOBS`, through which the online report
screen (`CORPT00C`) submits the transaction-report job to the internal reader.

# Online subsystem

## Programming model

Every online program is **pseudo-conversational**: a task receives a screen, processes it, sends the next
screen and ends with `EXEC CICS RETURN TRANSID(<own transaction>) COMMAREA(...)`. The next key the user
presses starts a new task of the same transaction, which finds its state in the communication area.

- **First entry vs. re-entry.** `EIBCALEN = 0` means the transaction was started without a communication
  area (directly from the terminal); programs then sign the user on again or show their first screen. Within
  the application, `CDEMO-PGM-CONTEXT` distinguishes the first entry into a program after an `XCTL`
  (`CDEMO-PGM-ENTER`, 0) from a re-entry with a screen to receive (`CDEMO-PGM-REENTER`, 1).
- **Navigation** is by `EXEC CICS XCTL PROGRAM(...) COMMAREA(...)`: the calling program sets
  `CDEMO-FROM-PROGRAM`/`CDEMO-FROM-TRANID` and the target in `CDEMO-TO-PROGRAM`/`CDEMO-TO-TRANID`.
  **PF3** returns to the program in `CDEMO-FROM-PROGRAM` (or to the menu, or to sign-on).
- **Keys** are read from `EIBAID` and compared with the `DFHAID` constants (Enter, Clear, PF1–PF24).
  The card and account programs map PF13–PF24 to PF1–PF12 (`YYYY-STORE-PFKEY`, copybook `CSSTRPFY`).
- **Error handling** is either `RESP(...)` on each file command with an `EVALUATE` of the response, or
  `EXEC CICS HANDLE ABEND LABEL(...)` with an abend routine (`COACTUPC`, `COACTVWC`, `COCRDSLC`, `COCRDUPC`).
- **Updates** run in the task's unit of work; `COACTUPC` and `COCRDUPC` take an explicit `SYNCPOINT`, and
  read-for-update plus a compare of old and new values guards against changes by other users.

## Transactions and programs

From the CSD (`DEFINE TRANSACTION`) and the programs' `LIT-THISTRANID`/`WS-TRANID` constants:

| Transaction | Program | Mapset / map | Function | Files (CICS commands) |
|---|---|---|---|---|
| `CC00` | `COSGN00C` | `COSGN00` / `COSGN0A` | Sign-on | `USRSEC` (READ) |
| `CM00` | `COMEN01C` | `COMEN01` / `COMEN1A` | Main menu (user) | — (`INQUIRE PROGRAM` for option 11) |
| `CA00` | `COADM01C` | `COADM01` / `COADM1A` | Admin menu | — |
| `CAVW` | `COACTVWC` | `COACTVW` / `CACTVWA` | Account view | `CXACAIX`, `ACCTDAT`, `CUSTDAT` (READ) |
| `CAUP` | `COACTUPC` | `COACTUP` / `CACTUPA` | Account update | `CXACAIX`, `ACCTDAT`, `CUSTDAT` (READ, READ UPDATE, REWRITE) |
| `CCLI` | `COCRDLIC` | `COCRDLI` / `CCRDLIA` | Credit card list | `CARDDAT` (STARTBR, READNEXT, READPREV, ENDBR) |
| `CCDL` | `COCRDSLC` | `COCRDSL` / `CCRDSLA` | Credit card view | `CARDDAT` (READ) |
| `CCUP` | `COCRDUPC` | `COCRDUP` / `CCRDUPA` | Credit card update | `CARDDAT` (READ, READ UPDATE, REWRITE) |
| `CT00` | `COTRN00C` | `COTRN00` / `COTRN0A` | Transaction list | `TRANSACT` (browse) |
| `CT01` | `COTRN01C` | `COTRN01` / `COTRN1A` | Transaction view | `TRANSACT` (READ) |
| `CT02` | `COTRN02C` | `COTRN02` / `COTRN2A` | Transaction add | `CXACAIX`, `CCXREF`, `ACCTDAT` (READ), `TRANSACT` (browse, WRITE) |
| `CR00` | `CORPT00C` | `CORPT00` / `CORPT0A` | Transaction reports | TD queue `JOBS` (WRITEQ TD) |
| `CB00` | `COBIL00C` | `COBIL00` / `COBIL0A` | Bill payment | `CXACAIX` (READ), `ACCTDAT` (READ UPDATE, REWRITE), `TRANSACT` (browse, WRITE) |
| `CU00` | `COUSR00C` | `COUSR00` / `COUSR0A` | User list | `USRSEC` (browse) |
| `CU01` | `COUSR01C` | `COUSR01` / `COUSR1A` | User add | `USRSEC` (WRITE) |
| `CU02` | `COUSR02C` | `COUSR02` / `COUSR2A` | User update | `USRSEC` (READ UPDATE, REWRITE) |
| `CU03` | `COUSR03C` | `COUSR03` / `COUSR3A` | User delete | `USRSEC` (READ UPDATE, DELETE) |
| `CDV1` | `COCRDSEC` | — | "Developer transaction" in the CSD | The program is **not** in the source: the transaction cannot run |

CICS commands used across the online programs: `SEND MAP`, `SEND TEXT`, `RECEIVE MAP`, `RETURN`, `XCTL`,
`READ`, `WRITE`, `REWRITE`, `DELETE`, `STARTBR`, `READNEXT`, `READPREV`, `ENDBR`, `SYNCPOINT`, `WRITEQ TD`,
`HANDLE ABEND`, `HANDLE CONDITION`, `ABEND`, `INQUIRE PROGRAM`, `ASKTIME`, `FORMATTIME`, `ASSIGN`.

## Navigation

```text
 CC00 COSGN00C  sign-on ──(user type U)──► CM00 COMEN01C main menu ──► options 1-10 (and 11)
                         └─(user type A)──► CA00 COADM01C admin menu ──► options 1-4 (and 5-6)
 F3 on a menu ──► back to sign-on;  F3 on sign-on ──► "Thank you" text, the session ends
```

The menus are table-driven. Each option is an entry (number, name, program, user type) in copybook
`COMEN02Y` (main menu) or `COADM02Y` (admin menu); the menu `XCTL`s to the program of the chosen entry.

| Main menu (`COMEN02Y`) | Program | | Admin menu (`COADM02Y`) | Program |
|---|---|---|---|---|
| 1 Account View | `COACTVWC` | | 1 User List (Security) | `COUSR00C` |
| 2 Account Update | `COACTUPC` | | 2 User Add (Security) | `COUSR01C` |
| 3 Credit Card List | `COCRDLIC` | | 3 User Update (Security) | `COUSR02C` |
| 4 Credit Card View | `COCRDSLC` | | 4 User Delete (Security) | `COUSR03C` |
| 5 Credit Card Update | `COCRDUPC` | | 5 Transaction Type List/Update (Db2) | `COTRTLIC` (optional module) |
| 6 Transaction List | `COTRN00C` | | 6 Transaction Type Maintenance (Db2) | `COTRTUPC` (optional module) |
| 7 Transaction View | `COTRN01C` | | | |
| 8 Transaction Add | `COTRN02C` | | | |
| 9 Transaction Reports | `CORPT00C` | | | |
| 10 Bill Payment | `COBIL00C` | | | |
| 11 Pending Authorization View | `COPAUS0C` (optional module) | | | |

`COMEN01C` checks with `EXEC CICS INQUIRE PROGRAM(...) NOHANDLE` that the chosen program is installed; for the
optional modules it is not, and the menu shows a message instead of transferring.

Inside the functions, the programs transfer among themselves:

- the card list `COCRDLIC` → card view `COCRDSLC` (`S`) or card update `COCRDUPC` (`U`) for a selected card;
- the transaction list `COTRN00C` → transaction view `COTRN01C` for a selected transaction;
- the user list `COUSR00C` → user update `COUSR02C` (`U`) or user delete `COUSR03C` (`D`);
- every function → back to its caller (PF3), the menu, or sign-on when there is no communication area.

## The communication area (`COCOM01Y`)

All online programs share the 160-byte `CARDDEMO-COMMAREA`. It is the only interface between them:
everything a program needs to know about the session travels in it.

| Offset | Length | Field | Content |
|---:|---:|---|---|
| 0 | 4 | `CDEMO-FROM-TRANID` | Transaction of the calling program |
| 4 | 8 | `CDEMO-FROM-PROGRAM` | Calling program (target of PF3) |
| 12 | 4 | `CDEMO-TO-TRANID` | Transaction of the target |
| 16 | 8 | `CDEMO-TO-PROGRAM` | Target program |
| 24 | 8 | `CDEMO-USER-ID` | Signed-on user |
| 32 | 1 | `CDEMO-USER-TYPE` | `A` administrator, `U` user |
| 33 | 1 | `CDEMO-PGM-CONTEXT` | 0 first entry, 1 re-entry |
| 34 | 9 | `CDEMO-CUST-ID` | Selected customer |
| 43 | 25 × 3 | `CDEMO-CUST-FNAME`, `-MNAME`, `-LNAME` | Customer name |
| 118 | 11 | `CDEMO-ACCT-ID` | Selected account |
| 129 | 1 | `CDEMO-ACCT-STATUS` | Account status |
| 130 | 16 | `CDEMO-CARD-NUM` | Selected card |
| 146 | 7 | `CDEMO-LAST-MAP` | Last map sent |
| 153 | 7 | `CDEMO-LAST-MAPSET` | Last mapset sent |

Several programs append a **program-specific area** after these 160 bytes (list paging keys, selected rows,
the old values of an update, a confirmation flag) and pass the longer area on `RETURN`. A program reads it
back on re-entry. The account and card programs (`COACTVWC`, `COACTUPC`, `COCRDLIC`, `COCRDSLC`,
`COCRDUPC`) copy their area from beyond `EIBCALEN` even when the caller passed only the 160 bytes (KD-16 in
the requirements).

## Screens (BMS)

Each program has one mapset with one map (`DFHMSD`/`DFHMDI`/`DFHMDF` in `bms/`), 24 × 80 characters. The
programs use the generated symbolic maps (copybooks named after the mapset, for example `COSGN00` with
`COSGN0AI`/`COSGN0AO`): for each field the length (`L`), attribute (`A`), colour (`C`) and data (`I`/`O`).
Common screen content comes from copybooks: the title lines (`COTTL01Y`), the date and time
(`CSDAT01Y`), messages (`CSMSG01Y`, `CSMSG02Y`), and attribute settings (`CSSETATY`).

## CICS resources (`CARDDEMO.CSD`)

| Resource | Name | Definition |
|---|---|---|
| Files | `ACCTDAT`, `CARDDAT`, `CUSTDAT`, `CCXREF`, `TRANSACT`, `USRSEC` | The KSDS clusters (see *Data*), all with READ, ADD, UPDATE, DELETE and BROWSE |
| Files (paths) | `CARDAIX`, `CXACAIX` | The alternate-index paths over the card file and the card cross reference |
| TD queue | `JOBS` | Extrapartition queue to the JES internal reader |
| Programs, mapsets | 18 programs, 17 mapsets | One per transaction program and screen (`COCRDSEC` defined but not supplied) |

The files are opened and closed by the batch jobs `OPENFIL` and `CLOSEFIL` (SDSF commands to CICS) around the
batch steps that load or replace the clusters.

# Batch subsystem

## Job streams

The JCL members in `jcl/` fall into four groups. Steps and programs are taken from the `EXEC` statements.

| Group | Job | Steps (programs) | Purpose |
|---|---|---|---|
| Loading the data | `ACCTFILE`, `CARDFILE`, `CUSTFILE`, `XREFFILE`, `TRANFILE`, `DISCGRP`, `TCATBALF`, `TRANCATG`, `TRANTYPE`, `DUSRSECJ` | IDCAMS `DELETE`/`DEFINE CLUSTER`/`REPRO` (CARDFILE, XREFFILE, TRANFILE also `DEFINE AIX`, `BLDINDEX`, `DEFINE PATH`); DUSRSECJ: IEBGENER of in-stream user records, then REPRO | Create a cluster and load it from the sample data set `AWS.M2.CARDDEMO.*.PS` |
| Nightly processing | `TRANBKP` | REPROC proc (REPRO to the `TRANSACT.BKUP` GDG), IDCAMS delete/define | Back up the transaction master and define it empty |
| | `POSTTRAN` | `CBTRN02C` | Post the daily transactions; rejects to the `DALYREJS` GDG |
| | `INTCALC` | `CBACT04C` (PARM run date) | Interest and fees per account; new transactions to the `SYSTRAN` GDG |
| | `COMBTRAN` | SORT, IDCAMS REPRO | Merge the backup with the system transactions into `TRANSACT.COMBINED`, reload the master |
| | `CREASTMT` | IDCAMS, SORT, IDCAMS, IEFBR14, `CBSTM03A` (calls `CBSTM03B`) | Customer statements as text (`STATEMNT.PS`) and HTML (`STATEMNT.HTML`) |
| | `TRANREPT` | REPROC, SORT (INCLUDE on processing date), `CBTRN03C` | The transaction report for a date range (`DATEPARM`) |
| Utilities and listings | `READACCT`, `READCARD`, `READCUST`, `READXREF` | `CBACT01C`, `CBACT02C`, `CBCUS01C`, `CBACT03C` | List a master file; `CBACT01C` also writes three demonstration formats |
| | `CBEXPORT`, `CBIMPORT` | `CBEXPORT`, `CBIMPORT` | Export all masters into one multi-record file and import it back into sequential files |
| | `WAITSTEP` | `COBSWAIT` (calls `MVSWAIT`) | Wait for the time given in the PARM |
| | `TRANIDX`, `DEFGDGB`, `DEFGDGD`, `REPTFILE`, `DALYREJS`, `DEFCUST`, `PRTCATBL`, `ESDSRRDS` | IDCAMS, IEBGENER, SORT | Define the transaction AIX, the GDG bases and further data sets; print the category balances; ESDS/RRDS samples |
| Operations | `OPENFIL`, `CLOSEFIL` | SDSF | Open and close the CICS files |
| | `CBADMCDJ` | `DFHCSDUP` | Define the CICS resources (the CSD) |
| | `FTPJCL`, `INTRDRJ1`, `INTRDRJ2`, `TXT2PDF1` | FTP, IDCAMS, IEBGENER, IKJEFT1B (REXX) | Transfer, internal-reader and text-to-PDF examples |

`CBTRN01C` (verify the daily transactions against the cross reference and the accounts, without updates) has
no JCL in the source.

## The nightly batch

```{=latex}
\needspace{9\baselineskip}
```

```text
 TRANBKP ─► POSTTRAN ─► INTCALC ─► TRANBKP ─► COMBTRAN ─► CREASTMT
 backup,    daily       interest   backup     merge,      statements
 empty      trans.      → SYSTRAN  again      reload      .PS and .HTML
 KSDS       → TRANSACT; GDG                   TRANSACT
            rejects →
            DALYREJS GDG

 TRANREPT: on demand; also submitted online by CORPT00C through the TD queue JOBS
```

## Batch program interfaces

From each program's `FILE-CONTROL` (`SELECT ... ASSIGN TO <DD name>`) and `CALL` statements. "Random" and
"sequential" are the `ACCESS MODE`s; the record layout comes from the copybook named.

| Program | DD name | Organisation, access | Key | Record (copybook) |
|---|---|---|---|---|
| `CBTRN01C` | `DALYTRAN` | sequential | — | `CVTRA06Y` daily transaction |
| | `CUSTFILE`, `XREFFILE`, `CARDFILE`, `ACCTFILE`, `TRANFILE` | indexed, random | customer, card, card, account, transaction id | `CVCUS01Y`, `CVACT03Y`, `CVACT02Y`, `CVACT01Y`, `CVTRA05Y` |
| `CBTRN02C` | `DALYTRAN` in, `DALYREJS` out | sequential | — | `CVTRA06Y` (+ 80-byte reject trailer) |
| | `TRANFILE`, `XREFFILE`, `ACCTFILE`, `TCATBALF` | indexed, random | transaction, card, account, category-balance key | `CVTRA05Y`, `CVACT03Y`, `CVACT01Y`, `CVTRA01Y` |
| `CBTRN03C` | `TRANFILE` in, `TRANREPT` out, `DATEPARM` in | sequential | — | `CVTRA05Y`, report lines `CVTRA07Y`, dates |
| | `CARDXREF`, `TRANTYPE`, `TRANCATG` | indexed, random | card, type, type + category | `CVACT03Y`, `CVTRA03Y`, `CVTRA04Y` |
| `CBACT04C` | `TCATBALF` | indexed, sequential | account + type + category | `CVTRA01Y` |
| | `XREFFILE` (alternate key account), `ACCTFILE`, `DISCGRP` | indexed, random | card / account, account, group + type + category | `CVACT03Y`, `CVACT01Y`, `CVTRA02Y` |
| | `TRANSACT` out | sequential | — | `CVTRA05Y`; run date from the PARM (linkage) |
| `CBSTM03A` | `STMTFILE`, `HTMLFILE` out | sequential | — | statement lines; file access via `CALL 'CBSTM03B'` |
| `CBSTM03B` | `TRNXFILE`, `XREFFILE` | indexed, sequential | card + transaction, card | `COSTM01`, `CVACT03Y` (passed by the caller) |
| | `CUSTFILE`, `ACCTFILE` | indexed, random | customer, account | `CUSTREC`, `CVACT01Y` |
| `CBACT01C` | `ACCTFILE` in; `OUTFILE`, `ARRYFILE`, `VBRCFILE` out | indexed sequential; sequential | account | `CVACT01Y`; three output formats; calls `COBDATFT` |
| `CBACT02C`, `CBACT03C`, `CBCUS01C` | `CARDFILE`, `XREFFILE`, `CUSTFILE` | indexed, sequential | card, card, customer | `CVACT02Y`, `CVACT03Y`, `CVCUS01Y` |
| `CBEXPORT` | five masters in; `EXPFILE` out | indexed, sequential | — / export sequence number | masters; `CVEXPORT` (500-byte multi-record) |
| `CBIMPORT` | `EXPFILE` in; `CUSTOUT`, `ACCTOUT`, `XREFOUT`, `TRNXOUT`, `CARDOUT`, `ERROUT` out | indexed sequential; sequential | export sequence number | `CVEXPORT`; the master layouts |

All batch programs end abnormally through `CALL 'CEE3ABD'` (Language Environment) with a user abend code
after an unexpected file status, displaying the status first (`9910-DISPLAY-IO-STATUS`, `9999-ABEND-PROGRAM`).

## Called routines

| Routine | Language | Called by | Interface |
|---|---|---|---|
| `CSUTLDTC` | COBOL | `COTRN02C`, `CORPT00C` | Validate a date: linkage `LS-DATE` X(10), `LS-DATE-FORMAT` X(10), `LS-RESULT` X(80); calls the LE service `CEEDAYS` |
| `CBSTM03B` | COBOL | `CBSTM03A` | File access for the statements: one linkage area with DD name X(8), operation (`O` open, `C` close, `R` read, `K` read by key, `W` write, `Z` rewrite), return code X(2), key X(25) and key length, and a 1000-byte data area |
| `COBDATFT` | Assembler | `CBACT01C` | Convert a date between formats (input type, output type, date) |
| `MVSWAIT` | Assembler | `COBSWAIT` | Wait for the interval given, with an interval-control timer |
| `CEE3ABD`, `CEEDAYS` | LE services | batch programs; `CSUTLDTC` | Abend with code; days since the Lilian epoch |

# Data

## VSAM clusters

From the `DEFINE CLUSTER` statements in the loading jobs. Keys are given as length and offset. The full
cluster name is `AWS.M2.CARDDEMO.<name>.VSAM.KSDS`; the table gives `<name>`.

| Cluster | CICS file | Key (length, offset) | Record | Copybook | Content |
|---|---|---|---|---|---|
| `ACCTDATA` | `ACCTDAT` | 11, 0 account id | 300 | `CVACT01Y` | Accounts: status, balances, limits, dates, group |
| `CARDDATA` | `CARDDAT` | 16, 0 card number | 150 | `CVACT02Y` | Cards: account, CVV, embossed name, expiry, status |
| `CUSTDATA` | `CUSTDAT` | 9, 0 customer id | 500 | `CVCUS01Y` | Customers: names, address, phones, SSN, FICO score |
| `CARDXREF` | `CCXREF` | 16, 0 card number | 50 | `CVACT03Y` | Card → customer and account |
| `TRANSACT` | `TRANSACT` | 16, 0 transaction id | 350 | `CVTRA05Y` | Posted transactions |
| `USRSEC` | `USRSEC` | 8, 0 user id | 80 | `CSUSR01Y` | Users: names, password, type |
| `TCATBALF` | — | 17, 0 account + type + category | 50 | `CVTRA01Y` | Balance per account and transaction category |
| `DISCGRP` | — | 16, 0 group + type + category | 50 | `CVTRA02Y` | Interest rate per disclosure group and category |
| `TRANTYPE` | — | 2, 0 type | 60 | `CVTRA03Y` | Transaction types |
| `TRANCATG` | — | 6, 0 type + category | 60 | `CVTRA04Y` | Transaction categories |
| `TRXFL` | — | 32, 0 card + transaction id | 350 | `COSTM01` | Work file of the statement job |

## Alternate indexes

The full name of an alternate index is `AWS.M2.CARDDEMO.<name>.VSAM.AIX`; the table gives `<name>`.

| AIX | Base cluster | Key (length, offset) | Path (CICS file) | Used by |
|---|---|---|---|---|
| `CARDDATA` | cards | 11, 16 account id (non-unique) | `CARDAIX` | defined (CSD, and as `LIT-CARDFILENAME-ACCT-PATH` in the card and account programs), but no `EXEC CICS` command uses it |
| `CARDXREF` | card cross reference | 11, 25 account id (non-unique) | `CXACAIX` | `COACTVWC`, `COACTUPC`, `COTRN02C`, `COBIL00C`; `INTCALC` (DD `XREFFIL1`) |
| `TRANSACT` | transactions | 26, 304 processing timestamp (non-unique) | — | defined by `TRANFILE`/`TRANIDX` |

## Sequential data sets and generation data groups

| Data set (`AWS.M2.CARDDEMO.`) | Written by | Read by | Record |
|---|---|---|---|
| `*.PS` (`ACCTDATA.PS`, `CARDDATA.PS`, ...) | — (shipped sample data, EBCDIC) | loading jobs (REPRO) | the master layouts |
| `DALYTRAN.PS` | — (input of the day) | `POSTTRAN`, `CBTRN01C` | 350, `CVTRA06Y` |
| `DALYREJS` (GDG) | `POSTTRAN` | — | 430: daily transaction + reject reason (4 digits) and text (76) |
| `TRANSACT.BKUP` (GDG) | `TRANBKP`, `TRANREPT` | `COMBTRAN`, `TRANREPT` | 350 |
| `SYSTRAN` (GDG) | `INTCALC` | `COMBTRAN` | 350 interest/fee transactions |
| `TRANSACT.COMBINED` (GDG) | `COMBTRAN` | reloaded into `TRANSACT` | 350 |
| `TRXFL.SEQ` | `CREASTMT` (SORT) | `CREASTMT` (REPRO into `TRXFL.VSAM.KSDS`) | 350 |
| `STATEMNT.PS`, `STATEMNT.HTML` | `CREASTMT` | — | 80-byte statement lines, 100-byte HTML lines |
| `TRANREPT` (GDG) | `TRANREPT` | — | 133-byte report lines (`CVTRA07Y`) |
| `EXPORT.DATA` | `CBEXPORT` | `CBIMPORT` | 500, `CVEXPORT`; defined as a KSDS by the `CBEXPORT` job (both programs' `RECORD KEY` names a WORKING-STORAGE field: KD-12) |
| `ACCTDATA.PSCOMP`, `.ARRYPS`, `.VBPS` | `READACCT` | — | `CBACT01C`'s demonstration formats |

## Record layouts

| Copybook | Record (01 level) | Length | Key |
|---|---|---:|---|
| `CVACT01Y` | `ACCOUNT-RECORD` | 300 | `ACCT-ID` 9(11) |
| `CVACT02Y` | `CARD-RECORD` | 150 | `CARD-NUM` X(16) |
| `CVACT03Y` | `CARD-XREF-RECORD` | 50 | `XREF-CARD-NUM` X(16); `XREF-ACCT-ID` alternate |
| `CVCUS01Y` | `CUSTOMER-RECORD` | 500 | `CUST-ID` 9(09) |
| `CVTRA01Y` | `TRAN-CAT-BAL-RECORD` | 50 | account 9(11) + type X(2) + category 9(4) |
| `CVTRA02Y` | `DIS-GROUP-RECORD` | 50 | group X(10) + type X(2) + category 9(4) |
| `CVTRA03Y` | `TRAN-TYPE-RECORD` | 60 | `TRAN-TYPE` X(2) |
| `CVTRA04Y` | `TRAN-CAT-RECORD` | 60 | type X(2) + category 9(4) |
| `CVTRA05Y` | `TRAN-RECORD` | 350 | `TRAN-ID` X(16) |
| `CVTRA06Y` | `DALYTRAN-RECORD` | 350 | `DALYTRAN-ID` X(16) |
| `CVTRA07Y` | report header, detail and total lines | 112–133 | — |
| `COSTM01` | `TRNX-RECORD` | 350 | card X(16) + transaction X(16) |
| `CVEXPORT` | `EXPORT-RECORD` | 500 | record type and sequence number; one layout per master record |
| `CSUSR01Y` | `SEC-USER-DATA` | 80 | `SEC-USR-ID` X(8) |
| `COCOM01Y` | `CARDDEMO-COMMAREA` | 160 | — (see the online subsystem) |

Field-level layouts (every elementary item with its picture and offset) are in the requirements, chapter
*Data dictionary*, which was derived from the same copybooks.

## Relationships

```{=latex}
\needspace{12\baselineskip}
```

```text
 CUSTOMER (CUST-ID) 1 ── n CARD-XREF (card → customer, account) n ── 1 ACCOUNT (ACCT-ID)
                              │ card number                           │ account id
                              ▼                                       ▼
                     CARD (CARD-NUM, CARD-ACCT-ID)        TRAN-CAT-BAL (account, type, cat.)
                              │ card number                           │ ACCT-GROUP-ID
                              ▼                                       ▼
                  TRANSACTION (TRAN-CARD-NUM)             DIS-GROUP (group, type, category)
                     │ type           │ type + category
                     ▼                ▼
                 TRAN-TYPE         TRAN-CAT
 USER (SEC-USR-ID): sign-on only, no link to the card data
```

# External interfaces

| Interface | Direction | Form |
|---|---|---|
| 3270 terminal | in/out | BMS maps, 24 × 80; AID keys Enter, Clear, PF1–PF24 |
| Daily transaction file | in | `DALYTRAN.PS`, 350-byte records |
| Report request | in | `DATEPARM` (start and end date) of `TRANREPT`; online, `CORPT00C` writes the JCL of a `TRANREPT` job to TD queue `JOBS` |
| Interest run date | in | PARM of `INTCALC` (`CBACT04C` linkage) |
| Statements, report, rejects | out | `STATEMNT.PS`/`.HTML`, `TRANREPT` GDG, `DALYREJS` GDG |
| Export file | out/in | `EXPORT.DATA`, 500-byte multi-record format (`CBEXPORT`, `CBIMPORT`) |
| Operator messages | out | `DISPLAY` to SYSOUT; abends via `CEE3ABD` with user codes |

# Source inventory

| Kind | Folder | Count |
|---|---|---:|
| COBOL programs, online | `cbl/CO*.cbl` (without `COBSWAIT`) | 17 |
| COBOL programs, batch and routines | `cbl/CB*`, `CSUTLDTC`, `COBSWAIT` | 15 |
| Assembler | `asm/` | 2 |
| Copybooks | `cpy/` | 30 |
| BMS mapsets | `bms/` | 17 |
| JCL members | `jcl/` | 38 |
| CICS resource definitions | `csd/CARDDEMO.CSD` | 18 transactions, 8 files, 17 mapsets, 1 TD queue |
