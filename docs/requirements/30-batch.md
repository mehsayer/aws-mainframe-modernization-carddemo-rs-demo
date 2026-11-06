# Batch Processing Requirements

All batch programs are sequential jobs. Each one opens its files at start. If a file
operation returns an unexpected status, the program displays a diagnostic and abends
with user code 999 (`CEE3ABD`). That is a non-zero termination, and later steps in the
job are skipped. The status diagnostic has the form `FILE STATUS IS: NNNNxxxx`, where
`xxxx` is the 4-digit status (`00ss` for numeric statuses; for VSAM `9x` statuses it is
`9` followed by the binary value of the second byte as 3 digits).

Common rule **BR-BAT-001**: each program displays `START OF EXECUTION OF PROGRAM <name>`
and `END OF EXECUTION OF PROGRAM <name>`. (CBEXPORT and CBIMPORT use their own banners
instead.)

Common rule **BR-BAT-002**: when a program builds a "DB2-format timestamp", the format is
`YYYY-MM-DD-HH.MM.SS.cc0000`. The date parts are separated by `-`, the time parts by `.`,
`cc` is hundredths of a second, and the value ends in the literal `0000`. Total length 26.

---

## FR-BAT-POST — Daily transaction posting (job POSTTRAN, program CBTRN02C)

*Source: cobol/app/cbl/CBTRN02C.cbl, cobol/app/jcl/POSTTRAN.jcl*

**Inputs:** DALYTRAN (sequential, 350-byte CVTRA06Y records), XREFFILE (KSDS keyed by
card number), ACCTFILE (KSDS keyed by account id, opened I-O), TCATBALF (KSDS keyed by
account id + type code + category code, opened I-O).
**Outputs:** TRANSACT (KSDS keyed by transaction id, opened OUTPUT, so the master file
is reloaded with only the posted records), DALYREJS (sequential, 430 bytes: the 350-byte
input record plus an 80-byte validation trailer).

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-POST-01 | Read every DALYTRAN record in sequence and count it in *transactions processed*. |
| FR-BAT-POST-02 | Validation step 1: look up the card number in XREF. If it is not found, set reason **100** `INVALID CARD NUMBER FOUND` and skip the remaining checks. |
| FR-BAT-POST-03 | Validation step 2: read the account by the XREF account id. If it is not found, set reason **101** `ACCOUNT RECORD NOT FOUND`. |
| FR-BAT-POST-04 | Over-limit check: compute `temp = ACCT-CURR-CYC-CREDIT − ACCT-CURR-CYC-DEBIT + DALYTRAN-AMT`. If `ACCT-CREDIT-LIMIT < temp`, set reason **102** `OVERLIMIT TRANSACTION`. |
| FR-BAT-POST-05 | Expiry check: if `ACCT-EXPIRAION-DATE < DALYTRAN-ORIG-TS(1:10)` (string comparison), set reason **103** `TRANSACTION RECEIVED AFTER ACCT EXPIRATION`. Checks 04 and 05 are both evaluated, and the later one overwrites the reason code. |
| FR-BAT-POST-06 | Rejected records go to DALYREJS: the input record followed by the trailer (reason code `9(04)` + description `X(76)`), and are counted in *transactions rejected*. |
| FR-BAT-POST-07 | Posting builds a TRAN record from the daily record field by field. `TRAN-PROC-TS` is the current DB2-format timestamp (BR-BAT-002). |
| FR-BAT-POST-08 | Category balance: read TCATBAL by (xref account id, type, category). If it is not found, display `TCATBAL record not found for key : <key>.. Creating.` and write a new record with balance = amount. Otherwise add the amount and rewrite. |
| FR-BAT-POST-09 | Account update: `ACCT-CURR-BAL += amt`. If `amt >= 0`, `ACCT-CURR-CYC-CREDIT += amt`; otherwise `ACCT-CURR-CYC-DEBIT += amt`. Then rewrite the account. |
| FR-BAT-POST-10 | Write the TRAN record to TRANSACT. A duplicate key or other error abends. |
| FR-BAT-POST-11 | At end, display `TRANSACTIONS PROCESSED :nnnnnnnnn` and `TRANSACTIONS REJECTED :nnnnnnnnn` (counters are `9(09)`). If any record was rejected, set return code **4**. |

## FR-BAT-INT — Interest calculation (job INTCALC, program CBACT04C)

*Source: cobol/app/cbl/CBACT04C.cbl, cobol/app/jcl/INTCALC.jcl. PARM = run date `YYYY-MM-DD`
(e.g. `2022071800`, of which the first 10 characters are used).*

**Inputs:** TCATBALF (KSDS, read sequentially in key order), XREFFILE (KSDS, read through
the alternate key, account id), DISCGRP (KSDS keyed by group id + type + category),
ACCTFILE (I-O).
**Output:** TRANSACT (sequential, a new SYSTRAN GDG generation) with one interest
transaction per category balance that has a non-zero rate.

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-INT-01 | Process TCATBAL records in key order. Display each record. |
| FR-BAT-INT-02 | When the account id changes, rewrite the previous account (FR-BAT-INT-06). On the first record, only set the first-time flag. Then reset total interest to 0, read the account (`ACCOUNT NOT FOUND: <id>` if missing) and read the XREF by account id. |
| FR-BAT-INT-03 | Interest rate: read DISCGRP with (ACCT-GROUP-ID, type, category). If it is not found (status 23), display `DISCLOSURE GROUP RECORD MISSING` / `TRY WITH DEFAULT GROUP CODE` and re-read with group id `DEFAULT`. If that also fails, abend. |
| FR-BAT-INT-04 | If the rate is not 0: `monthly = (TRAN-CAT-BAL × DIS-INT-RATE) / 1200`, truncated to 2 decimals (`S9(09)V99`, no ROUNDED). Add it to total interest. |
| FR-BAT-INT-05 | For each computed interest, write a transaction: ID = PARM date (10 chars) + 6-digit running suffix starting at 000001; type `01`; category `0005`; source `System`; description `Int. for a/c ` + ACCT-ID (11 digits); amount = monthly interest; merchant id 0; merchant name/city/zip blank; card = XREF card number; orig and proc timestamps = current DB2-format timestamp. |
| FR-BAT-INT-06 | Account update: `ACCT-CURR-BAL += total interest`, then `ACCT-CURR-CYC-CREDIT = 0` and `ACCT-CURR-CYC-DEBIT = 0`, then rewrite. |
| FR-BAT-INT-07 | Fees (`1400-COMPUTE-FEES`) are a stub that does nothing. |
| **KD-01** | *Known defect, kept for fidelity:* the end-of-file `ELSE PERFORM 1050-UPDATE-ACCOUNT` branch can never be reached, so the **last account** in TCATBAL never gets its interest and cycle reset written back. Its interest transactions are still written. |

## FR-BAT-REPT — Transaction detail report (job TRANREPT, program CBTRN03C)

*Source: cobol/app/cbl/CBTRN03C.cbl, cobol/app/jcl/TRANREPT.jcl. The JCL first sorts TRANSACT by card
number (and filters by date) into TRANSACT.DALY, then runs CBTRN03C. The job can also be
submitted from the online Report screen (CORPT00C).*

**Inputs:** TRANFILE (sequential, sorted), CARDXREF, TRANTYPE, TRANCATG (KSDS), DATEPARM
(an 80-byte record: start date `X(10)`, filler, end date `X(10)`).
**Output:** TRANREPT (133-byte lines).

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-REPT-01 | Read DATEPARM once and display `Reporting from <start> to <end>`. |
| FR-BAT-REPT-02 | Read each transaction. If `TRAN-PROC-TS(1:10)` is outside [start, end], the record is skipped with NEXT SENTENCE. *Fidelity note: the exact GnuCOBOL/Enterprise COBOL semantics are verified by differential test (DT-REPT).* |
| FR-BAT-REPT-03 | When the card number changes (and it is not the first record), write the account totals line. Then look up XREF for the new card. A missing XREF, transaction type, or category abends after `INVALID CARD NUMBER :`, `INVALID TRANSACTION TYPE :`, or `INVALID TRAN CATG KEY :`. |
| FR-BAT-REPT-04 | On the first detail line, write the headers: name header (`DALYREPT` padded to 38, `Daily Transaction Report` padded to 41, `Date Range: <start> to <end>`), a blank line, column header 1, and a line of 133 `-`. |
| FR-BAT-REPT-05 | Page size is 20 lines. When `line-counter mod 20 = 0` before a detail line, write the page totals and repeat the headers. |
| FR-BAT-REPT-06 | Detail line layout (CVTRA07Y): tran id(16) sp acct(11) sp type(2) `-` type-desc(15) sp cat(4) `-` cat-desc(29) sp source(10) 4sp amount `-ZZZ,ZZZ,ZZZ.ZZ` 2sp. |
| FR-BAT-REPT-07 | Totals: page total `Page Total` + dots + `+ZZZ,ZZZ,ZZZ.ZZ`. The page total adds to the grand total and resets. The account total resets after printing. Each totals line is followed by a dashed line. |
| FR-BAT-REPT-08 | At EOF: add the last record's amount to the page and account totals **again** (KD-02), then write page totals and grand totals. The last account's totals line is not written (KD-03). |

## FR-BAT-STMT — Customer statements (job CREASTMT, programs CBSTM03A + CBSTM03B)

*Source: cobol/app/cbl/CBSTM03A.CBL, CBSTM03B.CBL, cobol/app/jcl/CREASTMT.JCL.*

The job sorts TRANSACT by card number (pos 263, length 16) then transaction id, and
loads the result into a KSDS (TRXFL) keyed by card + tran id. CBSTM03A produces two
outputs: STMTFILE, a plain-text statement in 80-byte lines, and HTMLFILE, an HTML
statement in 100-byte lines. CBSTM03B is a file-access subroutine. CBSTM03A calls it
with a DD name, an operation (`O` open, `C` close, `R` read next, `K` read by key), a
key and key length, and receives a 2-character return code and the record data.

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-STMT-01 | (z/OS-only diagnostics) display the job and step name and the list of DD names from the TIOT. *Rust: the job/step name and the configured DD mapping.* |
| FR-BAT-STMT-02 | Load **all** transactions into memory, grouped by card: at most 51 cards × 10 transactions per card. Overflowing these limits is undefined in the original (KD-04). |
| FR-BAT-STMT-03 | For each XREF record in card order: read the customer by XREF cust id and the account by XREF acct id (either missing → abend), then produce one statement. |
| FR-BAT-STMT-04 | Text statement lines: `*` ×31 + `START OF STATEMENT` + `*` ×31; name (first, middle, last, each trimmed at the first space and joined with single spaces); address line 1; address line 2; line 3 + state + country + zip (each trimmed and space-joined); dashes; `Basic Details` centred; dashes; `Account ID :` padded to 20 + acct id; `Current Balance :` + `9(9).99-`; `FICO Score :` + score; dashes; `TRANSACTION SUMMARY`; dashes; column headings; dashes. |
| FR-BAT-STMT-05 | Each transaction of the card: tran id(16) sp description(49) `$` amount `Z(9).99-`. Its amount adds to the total. |
| FR-BAT-STMT-06 | Trailer: dashes, `Total EXP:` + 56 spaces + `$` + total `Z(9).99-`, then `*` ×32 + `END OF STATEMENT` + `*` ×32. |
| FR-BAT-STMT-07 | The HTML statement has the same content as an HTML table. Fixed lines come from the 88-level literals (bank name `Bank of XYZ`, address `410 Terry Ave N`, `Seattle WA 99999`). Each statement is a complete `<!DOCTYPE html> … </html>` document appended to the same file. |

## FR-BAT-RDR — File dump utilities (READACCT/CBACT01C, READCARD/CBACT02C, READXREF/CBACT03C, READCUST/CBCUS01C)

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-RDR-01 | CBACT02C, CBACT03C and CBCUS01C read the card, XREF and customer KSDS sequentially and DISPLAY each record. CBACT03C and CBCUS01C display each record twice (once in the read paragraph, once in the main loop). |
| FR-BAT-RDR-02 | CBACT01C reads accounts sequentially and, for each one, displays the labelled fields (`ACCT-ID :` … `ACCT-GROUP-ID :`, then a dashed line), then the VBRC records, and finally the whole account record (main-loop DISPLAY). It writes three outputs: |
| | (a) OUTFILE: id, status, balance, limits, dates; reissue date converted from `YYYY-MM-DD` to `YYYYMMDD` by COBDATFT (the 10-byte field ends in two X'00' bytes: COBDATFT stores 8 bytes into a WORKING-STORAGE area without VALUE); cycle credit; cycle debit as COMP-3, set to `2525.00` when the account's debit is zero and otherwise left unchanged from the previous record (KD-05); group id. |
| | (b) ARRYFILE: id + 5 × (balance, COMP-3 debit). Slot 1 = (bal, 1005.00), slot 2 = (bal, 1525.00), slot 3 = (−1025.00, −2500.00), slots 4–5 = zero. |
| | (c) VBRCFILE (variable-length records): a 12-byte record (id + status) and a 39-byte record (id + balance + limit + reissue year). |
| FR-BAT-RDR-03 | COBDATFT (assembler): type 1 input `YYYYMMDD` → output `YYYY-MM-DD` (output type must be 1); type 2 input `YYYY-MM-DD` → output `YYYYMMDD` (output type must be 2). Anything else sets the error message `INVALID INPUT`. |

## FR-BAT-VER — Daily transaction verification (CBTRN01C; no JCL ships with the original)

| ID | Requirement |
|---|---|
| FR-BAT-VER-01 | Read DALYTRAN sequentially and DISPLAY each record. For each record: look up XREF by card number. If found, display `SUCCESSFUL READ OF XREF`, `CARD NUMBER: `, `ACCOUNT ID : `, `CUSTOMER ID: `, then read the account (`SUCCESSFUL READ OF ACCOUNT FILE`, or `INVALID ACCOUNT NUMBER FOUND` + `ACCOUNT <id> NOT FOUND`). If not found, display `INVALID CARD NUMBER FOR XREF` and `CARD NUMBER <n> COULD NOT BE VERIFIED. SKIPPING TRANSACTION ID-<id>`. No files are updated. |
| FR-BAT-VER-02 | After end of file the lookup runs once more for the last record read (loop structure of the original). |

## FR-BAT-XPRT — Export / import (CBEXPORT, CBIMPORT)

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-XPRT-01 | CBEXPORT reads customers, accounts, XREF, transactions and cards in that order. For each record it writes one 500-byte EXPORT-RECORD (CVEXPORT) with type `C`/`A`/`X`/`T`/`D`, timestamp `YYYY-MM-DD HH:MM:SS.00` followed by 4 bytes X'00' (STRING into a 26-byte field without VALUE), binary sequence number, branch `0001`, region `NORTH`, and the record body in the mixed COMP / COMP-3 / zoned layout of CVEXPORT. |
| FR-BAT-XPRT-02 | It displays per-type counts and the total at the end. |
| FR-BAT-XPRT-03 | CBIMPORT reads the export file and splits it by record type back into customer, account, XREF, transaction and card files. An unknown type is written to ERROUT as `timestamp|type|seq|Unknown record type encountered`. |
| FR-BAT-XPRT-05 | *Deviation:* the EXPORT.DATA KSDS (`KEYS(4 28)` over a binary sequence number) is a fixed-length sequential file in write order. CBIMPORT reads it in the same order. |
| FR-BAT-XPRT-04 | It displays the import statistics (read, per-type counts, errors written, unknown types). Output records are INITIALIZEd field by field, so their FILLER bytes are X'00': an export/import round trip reproduces every named field, not the FILLER bytes. Validation (`3000-VALIDATE-IMPORT`) only displays a completion message. |

## FR-BAT-UTIL — Utilities

| ID | Requirement |
|-----|------------------------------|
| FR-BAT-UTIL-01 | COBSWAIT reads an 8-digit value from SYSIN and waits that many **centiseconds** (MVSWAIT STIMER BINTVL units). |
| FR-BAT-UTIL-02 | CSUTLDTC validates a date string against a format mask (as CEEDAYS does) and returns an 80-byte message: severity `9(4)`, `Mesg Code:`, message number, result text (`Date is valid`, `Insufficient`, `Datevalue error`, `Invalid Era`, `Unsupp. Range`, `Invalid month`, `Bad Pic String`, `Nonnumeric data`, `YearInEra is 0`, `Date is invalid`), `TstDate:` + date, `Mask used:` + mask. RETURN-CODE = severity. Supported masks used by the application: `YYYY-MM-DD` and `YYYYMMDD`. |
| FR-BAT-UTIL-03 | IDCAMS REPRO / DEFINE / DELETE, IEBGENER, SORT and IEFBR14 steps are replaced by the Rust job runner's built-in steps: load file into store, unload store to file, sort by key fields, delete/define dataset, GDG new generation. |

## Known defects kept for fidelity

| ID | Program | Description |
|----|-----|---------------------------|
| KD-01 | CBACT04C | The last account's interest update is never written back. |
| KD-02 | CBTRN03C | At EOF the last transaction amount is added to the totals a second time. |
| KD-03 | CBTRN03C | The last account's "Account Total" line is never printed. |
| KD-04 | CBSTM03A | Fixed in-memory limits of 51 cards × 10 transactions, with no bounds check. |
| KD-09 | CBIMPORT JCL | The JCL has no CARDOUT DD although the program opens CARD-OUTPUT. The Rust job supplies it. |
| KD-05 | CBACT01C | The OUTFILE cycle-debit value carries over from the previous record when the account's debit is non-zero. |
| KD-12 | CBEXPORT, CBIMPORT | The export file's `RECORD KEY IS EXPORT-SEQUENCE-NUM` names a WORKING-STORAGE field (CVEXPORT), not a field of the file's own record. COBOL requires the key inside the record description, so neither program compiles as shipped (verified with GnuCOBOL 3.2). The Rust port implements the evident intent; the differential test compiles both with the key moved into the FD record at the same offset. |

Source note: `CUSTREC.cpy` and `CSLKPCDY.cpy` contain TAB characters. With 8-column tabs the CUSTREC fields run past column 72 and CBSTM03A does not compile; the alignment of the other lines shows that 4-column tabs were intended.
