# Online Requirements — Transactions, Bill Payment, Reports

Transaction master: TRANSACT KSDS keyed by TRAN-ID (16 characters), record CVTRA05Y (350 bytes).

## FR-TRL — Transaction list (CT00, COTRN00C, map COTRN00/COTRN0A)

| ID | Requirement |
|-----|------------------------------|
| FR-TRL-01 | Shows up to 10 transactions per page in TRAN-ID order. Each row has a select field, transaction id, date (`mm/dd/yy` from TRAN-ORIG-TS), description and amount (`+99999999.99`). The page number is shown. |
| FR-TRL-02 | Optional "Search Tran ID" field. If it is blank, browsing starts at the first record. If it is non-numeric → `Tran ID must be Numeric ...`. Otherwise browsing starts at that key (STARTBR GTEQ). |
| FR-TRL-03 | Selection `S`/`s` on a row → XCTL COTRN01C with the selected id in the commarea extension (CDEMO-CT00-TRN-SELECTED). Any other code → `Invalid selection. Valid value is S`. |
| FR-TRL-04 | Paging with PF7/PF8 works exactly like FR-USR-LST-04..06, with these messages: `You are already at the top of the page...`, `You are already at the bottom of the page...`, `You have reached the bottom of the page...`, `You have reached the top of the page...`, `You are at the top of the page...` (STARTBR not found). |
| FR-TRL-05 | PF3 → main menu (COMEN01C). File errors → `Unable to lookup transaction...`. |

## FR-TRV — Transaction view (CT01, COTRN01C, map COTRN01/COTRN1A)

| ID | Requirement |
|-----|------------------------------|
| FR-TRV-01 | On first entry with a transaction preselected (from the list), it is displayed immediately. |
| FR-TRV-02 | ENTER: blank id → `Tran ID can NOT be empty...`. Not found → `Transaction ID NOT found...`. Other error → `Unable to lookup Transaction...`. |
| FR-TRV-03 | Displays transaction id, card number, type code, category code, source, amount (`+99999999.99`), description, original and processed timestamps, merchant id, merchant name, city and zip. |
| FR-TRV-04 | PF3 → back to the caller (or the main menu). PF4 → clear the screen. PF5 → transaction list (COTRN00C). |

## FR-TRA — Transaction add (CT02, COTRN02C, map COTRN02/COTRN2A)

**Processing rule BR-TRA-0:** in this program every screen send also ends the task
(SEND followed by RETURN TRANSID). So the **first** validation error found is the one
shown, and nothing after it runs.

| ID | Requirement |
|-----|------------------------------|
| FR-TRA-01 | Key fields: account id **or** card number. If an account id is entered: it must be numeric (`Account ID must be Numeric...`); it is looked up in the XREF alternate index by account (`Account ID NOT found...`); and the card number field is filled from XREF. Otherwise, if a card number is entered: it must be numeric (`Card Number must be Numeric...`); it is looked up in XREF (`Card Number NOT found...`); and the account id is filled. If neither is entered → `Account or Card Number must be entered...`. |
| FR-TRA-02 | Mandatory data fields, checked in this order: Type CD, Category CD, Source, Description, Amount, Orig Date, Proc Date, Merchant ID, Merchant Name, Merchant City, Merchant Zip. Each gives `<Field> can NOT be empty...` (e.g. `Type CD can NOT be empty...`, `Merchant Zip can NOT be empty...`). |
| FR-TRA-03 | Type CD and Category CD must be numeric (`Type CD must be Numeric...`, `Category CD must be Numeric...`). |
| FR-TRA-04 | Amount format: sign `+`/`-`, 8 digits, `.`, 2 digits. Otherwise → `Amount should be in format -99999999.99`. The amount is then normalised and redisplayed as `+99999999.99`. |
| FR-TRA-05 | Orig and Proc dates must have the form `YYYY-MM-DD` (`Orig Date should be in format YYYY-MM-DD`, `Proc Date ...`). They must also be valid calendar dates according to CSUTLDTC (`Orig Date - Not a valid date...`, `Proc Date - Not a valid date...`). Message number 2513 is tolerated. |
| FR-TRA-06 | Merchant ID must be numeric (`Merchant ID must be Numeric...`). |
| FR-TRA-07 | Confirmation field: `Y`/`y` → add. `N`/`n`/blank → `Confirm to add this transaction...`. Anything else → `Invalid value. Valid values are (Y/N)...`. |
| FR-TRA-08 | Add: find the highest existing TRAN-ID (browse from HIGH-VALUES, READPREV; an empty file gives 0). The new id is that value + 1, as 16 digits. Build the record from the screen: type, category, source, description, amount, card number, merchant fields, orig TS = the orig date, proc TS = the proc date. Then WRITE it. |
| FR-TRA-09 | Success → all fields cleared, message `Transaction added successfully.  Your Tran ID is <id>.` in green. Duplicate key → `Tran ID already exist...`. Other error → `Unable to Add Transaction...`. |
| FR-TRA-10 | PF5 copies the data fields of the **last transaction in the file** (highest id) into the screen, after validating the key fields, and then runs the ENTER processing. PF4 clears. PF3 → back to the caller (or the main menu). |

## FR-BIL — Bill payment (CB00, COBIL00C, map COBIL00/COBIL0A)

| ID | Requirement |
|-----|------------------------------|
| FR-BIL-01 | Input: account id (11) and a confirmation (Y/N). Blank account id → `Acct ID can NOT be empty...`. The account id is not checked for digits before it is moved to the PIC 9(11) key, so any other input is compiler-dependent (GnuCOBOL reads `6` followed by spaces as account 6; the Rust port uses the characters as typed, so that account is not found). |
| FR-BIL-02 | The account is read for update. Not found → `Account ID NOT found...`. Other error → `Unable to lookup Account...`. The current balance is shown as `+9999999999.99`. |
| FR-BIL-03 | If the balance is ≤ 0 → `You have nothing to pay...`. |
| FR-BIL-04 | Confirmation blank → the balance is shown with `Confirm to make a bill payment...`. `N`/`n` → screen cleared. Any value other than Y/N → `Invalid value. Valid values are (Y/N)...`. |
| FR-BIL-05 | Confirmation `Y`/`y`: read the XREF by account (AIX) to get the card number. The new transaction id is the highest TRAN-ID + 1. Write a transaction with type `02`, category `0002`, source `POS TERM`, description `BILL PAYMENT - ONLINE`, amount = **full current balance**, card number from XREF, merchant id `999999999`, merchant name `BILL PAYMENT`, city `N/A`, zip `N/A`, orig and proc timestamps = current `YYYY-MM-DD HH:MM:SS.000000`. |
| FR-BIL-06 | After the write: `ACCT-CURR-BAL = ACCT-CURR-BAL − amount` (i.e. 0), then REWRITE the account. Success message (green): `Payment successful.  Your Transaction ID is <id>.`, and the fields are cleared. |
| FR-BIL-07 | Errors: `Tran ID already exist...`, `Unable to Add Bill pay Transaction...`, `Unable to Update Account...`, `Unable to lookup XREF AIX file...`, `Unable to lookup Transaction...`. |
| FR-BIL-08 | PF3 → back to the caller (or the main menu). PF4 → clear. |

## FR-RPT — Transaction reports (CR00, CORPT00C, map CORPT00/CORPT0A)

**Processing rule BR-RPT-0:** every screen send ends the task (SEND then RETURN), so the first error found is the one shown.

| ID | Requirement |
|-----|------------------------------|
| FR-RPT-01 | Choose one report type, checked in this order: Monthly, Yearly, Custom. If none is chosen → `Select a report type to print report...`. |
| FR-RPT-02 | Monthly: from the 1st of the current month to its last day (computed as the 1st of the next month minus 1 day). Yearly: from `YYYY-01-01` to `YYYY-12-31` of the current year. |
| FR-RPT-03 | Custom: start and end month, day and year are mandatory (`Start Date - Month can NOT be empty...`, `Start Date - Day …`, `Start Date - Year …`, `End Date - Month …`, `End Date - Day …`, `End Date - Year …`). The values are normalised with NUMVAL-C into zero-padded numbers. Month > 12 or non-numeric → `Start Date - Not a valid Month...` (or End). Day > 31 → `… Not a valid Day...`. Year non-numeric → `… Not a valid Year...`. A calendar date check via CSUTLDTC gives `Start Date - Not a valid date...` / `End Date - Not a valid date...`. |
| FR-RPT-04 | Confirmation: blank → `Please confirm to print the <name> report...`. `N`/`n` → screen cleared. Any other value except Y → `"<value>" is not a valid value to confirm...`. |
| FR-RPT-05 | On confirmation `Y`: the JCL deck for TRANREPT (with SORT symbol parameters for the start and end dates and the DATEPARM record `<start> <end>`) is written line by line to the TD queue `JOBS` (the internal reader). *Rust: the job is queued to the built-in job runner, which runs the TRANREPT job (FR-BAT-REPT) with those dates. The JCL text is kept in a spool log for traceability.* TDQ failure → `Unable to Write TDQ (JOBS)...`. |
| FR-RPT-06 | Success → fields cleared, message (green) `<Monthly|Yearly|Custom> report submitted for printing ...`. |
| FR-RPT-07 | PF3 → main menu. |
