# Online Requirements — Accounts and Cards

These five programs (COACTVWC, COACTUPC, COCRDLIC, COCRDSLC, COCRDUPC) use a newer coding
style than the rest of the application.

They share these rules:

- **BR-AC-01.** Function keys are normalised: PF13–PF24 count as PF1–PF12 (CSSTRPFY).
  Any key the screen does not accept is treated as ENTER. These screens never show the
  "invalid key" message.
- **BR-AC-02.** Commarea: the 160-byte COCOM01Y area, followed by a program-specific
  area. The whole commarea is passed back on RETURN (2000 bytes for COACTVWC).
- **BR-AC-03.** Entry from the main menu (`CDEMO-FROM-PROGRAM = COMEN01C` and the
  context is not REENTER) **re-initialises the whole commarea**. That clears the user
  id and user type, and the program then sets the user type to `U` before leaving
  with PF3.
- **BR-AC-04.** PF3 returns to `CDEMO-FROM-PROGRAM`/`CDEMO-FROM-TRANID` (or the main
  menu if they are blank). It sets from-tran and from-program to this program, sets
  context ENTER, sets the user type to `U`, and sets last mapset/map.
- **BR-AC-05.** File error messages use the format
  `File Error: <op 8> on <file 9> returned RESP <resp 10>,RESP2 <resp2 10>`.
  RESP values are printed as 9-digit unsigned numbers.
- **BR-AC-06.** An unexpected state sends plain text `UNEXPECTED DATA SCENARIO` and ends
  the task. An abend sends the abend data and abends with code 9999.
- **BR-AC-07.** A mandatory field left blank on re-entry is shown as `*` in red. A field
  in error is shown in red with the cursor on it.

## FR-ACV — Account view (CAVW, COACTVWC, map COACTVW/CACTVWA)

| ID | Requirement |
|-----|------------------------------|
| FR-ACV-01 | First entry: show the screen with the info message `Enter or update id of account to display`. |
| FR-ACV-02 | Input: account number (11). Blank or `*` → `Account number not provided` and the field is shown as a red `*`. Non-numeric or all zeros → `Account Filter must be a non-zero 11 digit number`. |
| FR-ACV-03 | Lookup chain: XREF by account (CXACAIX) gives the customer id and card number (also stored in the commarea). Then the account master, then the customer master. Not-found messages (RESP/RESP2 as 9 digits left-justified in 10-byte fields; the result is cut at 75 characters): `Account:<id> not found in Cross ref file.  Resp:<resp>  Reas:<resp2>`, `Account:<id> not found in Acct Master file.Resp:<resp> Reas:<resp2>`, `CustId:<id> not found in customer master.Resp: <resp> REAS:<resp2>`. |
| FR-ACV-04 | Displayed fields: account status, current balance, credit limit, cash credit limit, current cycle credit, current cycle debit, open, expiry and reissue dates, group id. Customer fields: id, SSN as `999-99-9999`, FICO score, date of birth, first, middle and last name, address lines 1–2, city (address line 3), state, zip, country, phone 1, phone 2, government id, EFT account id, primary card holder flag. |
| FR-ACV-06 | *Map quirk:* map CACTVWA contains an unnamed field `LENGTH=9,POS=(1,1)` after the header fields. On a 3270 it overwrites row 1, columns 1–10 with nulls, so only the last letter of the transaction id (`W`) remains visible. |
| FR-ACV-05 | PF3 → back to the caller (BR-AC-04). |

## FR-CDV — Credit card detail view (CCDL, COCRDSLC, map COCRDSL/CCRDSLA)

| ID | Requirement |
|-----|------------------------------|
| FR-CDV-01 | Entered from the card list (context ENTER, from-program COCRDLIC): the account and card from the commarea are used to read the card immediately. Both search fields are then protected. |
| FR-CDV-02 | Entered any other way: the prompt `Please enter Account and Card Number` is shown in the info line. |
| FR-CDV-03 | Account filter: blank, `*` or all zeros → `Account number not provided`. Non-numeric → `ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER`. |
| FR-CDV-04 | Card filter: blank, `*` or all zeros → `Card number not provided`. Non-numeric → `CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER`. Only the first error message is kept. Both fields are always validated, so both can be flagged red. |
| FR-CDV-05 | If both filters are valid, the card is read by card number only; the account number is not cross-checked. Not found → `Did not find cards for this search condition` (both fields red). Found → info message ` Displaying requested details`, embossed name, expiry month and year (from `YYYY-MM-DD`), and card status (Y/N). |
| FR-CDV-06 | PF3 → back to the caller (BR-AC-04). Other keys are treated as ENTER. |

## FR-CDL — Credit card list (CCLI, COCRDLIC, map COCRDLI/CCRDLIA)

| ID | Requirement |
|-----|------------------------------|
| FR-CDL-01 | Shows the card file (CARDDAT, in card-number order) **7 rows per page**. Each row has a select field, account number, card number and active status. Optional filters: account id (11) and card number (16). A card is listed only if it matches every filter that was supplied. |
| FR-CDL-02 | Filter validation: blank or zero means no filter. A non-numeric account → `ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER`. A non-numeric card → `CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER` (only if no earlier message). On any filter error the field turns red, the row select fields are protected, and the list is not re-read. |
| FR-CDL-03 | Row action codes: `S` = view detail (XCTL COCRDSLC), `U` = update (XCTL COCRDUPC). The selected row's account and card number go into the commarea, with from-program COCRDLIC and context ENTER. More than one `S`/`U` → `PLEASE SELECT ONLY ONE RECORD TO VIEW OR UPDATE`, with the selected rows flagged red. Any other non-blank code → `INVALID ACTION CODE` (that row red). |
| FR-CDL-04 | PF8: if there is a next page, show the 7 matching cards after the last card on the current page. With no further page → `NO MORE PAGES TO DISPLAY` once the last page has been shown. Reaching end of file while filling a page → `NO MORE RECORDS TO SHOW`. |
| FR-CDL-05 | PF7: on the first page → `NO PREVIOUS PAGES TO DISPLAY`. Otherwise read backwards from the first card of the current page to fill the previous page. |
| FR-CDL-06 | No matching card on the first page → `NO RECORDS FOUND FOR THIS SEARCH CONDITION.` |
| FR-CDL-07 | When there are rows or more pages, the info line shows `TYPE S FOR DETAIL, U TO UPDATE ANY RECORD` (neutral). |
| FR-CDL-08 | PF3 → main menu (from-tran `CCLI`, from-program `COCRDLIC`, user type `U`). Keys other than ENTER/PF3/PF7/PF8 are treated as ENTER. On entry from another program the list starts again from the first card. |
| FR-CDL-09 | File errors use the BR-AC-05 format (here starting with the literal `File Error:` with no trailing space). |

## FR-CDU — Credit card update (CCUP, COCRDUPC, map COCRDUP/CCRDUPA)

This screen runs as a state machine. The state (`CCUP-CHANGE-ACTION`) is kept in the
commarea extension, together with the old and new card details:

| State | Meaning |
|---|---|
| not fetched (blank) | Waiting for the search keys. |
| `S` | Details shown. |
| `E` | Changes entered but not valid. |
| `N` | Changes valid, waiting for PF5. |
| `C` | Changes committed. |
| `L` | Lock error. |
| `F` | Update failed. |

| ID | Requirement |
|-----|------------------------------|
| FR-CDU-01 | Entered from the card list (or PF12 when called from the list): read the card for the account and card from the commarea and show its details (state S). Entered from the menu or with no details fetched: prompt `Please enter Account and Card Number`. |
| FR-CDU-02 | Search key validation is the same as FR-CDV-03/04: `Account number not provided`, `ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER`, `Card number not provided`, `CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER`. Card not found → `Did not find cards for this search condition`. Found → info `Details of selected card shown above`. The embossed name is converted to upper case for display and comparison. |
| FR-CDU-03 | Editable fields in state S/E: embossed name, active status, expiry month, expiry year. The expiry day is not editable (shown dark). The key fields are protected. |
| FR-CDU-04 | If the new data equals the old data (case-insensitive) → `No change detected with respect to values fetched.` |
| FR-CDU-05 | Validation (all four fields are checked; the first message is kept). Name: blank → `Card name not provided`; letters and spaces only, else `Card name can only contain alphabets and spaces`. Status: must be `Y` or `N`, else `Card Active Status must be Y or N`. Month: must be 1–12, else `Card expiry month must be between 1 and 12`. Year: must be 1950–2099, else `Invalid card expiry year`. Any failure → state E, info `Update card details presented above.`, and the failing fields shown in red (or `*` if blank). |
| FR-CDU-06 | All valid → state N, info `Changes validated.Press F5 to save`. All fields are protected and the PF5 hint is highlighted. |
| FR-CDU-07 | PF5 in state N: re-read the card FOR UPDATE. Failure → `Could not lock record for update` (state L). If the record differs from the old snapshot (CVV, name, expiry, status) → `Record changed by some one else. Please review`, and the new values are shown (state S). Otherwise REWRITE the card with: card number, account id, CVV (see KD-07), the new name, expiry `YYYY-MM-DD` from the new year and month plus the old day, and the new status. Rewrite failure → `Update of record failed` (state F). Success → state C, info `Changes committed to database`. |
| FR-CDU-08 | After C/L/F (`Changes unsuccessful. Please try again` for L/F): if the screen was called from the card list, return to it automatically on the next key. Otherwise the next interaction starts again with an empty search. |
| FR-CDU-09 | PF3 → back to the caller (BR-AC-04), with SYNCPOINT. If the caller was the card list, the commarea account and card are cleared. PF12 (once details are fetched) → re-fetch and discard the changes. Keys not valid in the current state are treated as ENTER. |
| KD-07 | *Known defect, kept:* the CVV written on REWRITE comes from a field that is never filled (`CCUP-NEW-CVV-CD`), so an update **resets the card's CVV**. Rust writes `000`. |

## FR-ACU — Account update (CAUP, COACTUPC, map COACTUP/CACTUPA)

This screen uses the same state machine as FR-CDU (`ACUP-CHANGE-ACTION`: not fetched /
S / E / N / C / L / F). One screen updates both the **account** record and its
**customer** record.

| ID | Requirement |
|-----|------------------------------|
| FR-ACU-01 | First entry: the info line shows `Enter or update id of account to update`. Only the account id field is enterable. |
| FR-ACU-02 | Account id: blank or `*` → `Account number not provided`. Non-numeric or zero → `Account Number if supplied must be a 11 digit Non-Zero Number`. |
| FR-ACU-03 | Fetch: read XREF by account (CXACAIX) to get the customer id, then the account master, then the customer master. The not-found messages are the same as FR-ACV-03. Success → state S, info `Details of selected account shown above`. The old values are kept in the commarea. |
| FR-ACU-04 | Editable fields in state S/E. Account: active status, open date (Y/M/D), credit limit, expiry date (Y/M/D), cash credit limit, reissue date (Y/M/D), current balance, current cycle credit, current cycle debit, group id. Customer: SSN (3/2/4), date of birth (Y/M/D), FICO score, first, middle and last name, address line 1, address line 2, state, zip, city, country, phone 1 (area/prefix/line), phone 2, government id, EFT account id, primary card holder flag. A field containing `*` or spaces is treated as empty. |
| FR-ACU-05 | Monetary inputs are accepted in any NUMVAL-C form (sign, commas, decimal point) and redisplayed with `+ZZZ,ZZZ,ZZZ.99`. |
| FR-ACU-06 | If no field changed (case-insensitive for text) → `No change detected with respect to values fetched.` |
| FR-ACU-07 | Validation runs in screen order and **every** field is checked. Only the **first** message is shown; every failing field turns red, or shows `*` if blank. `<Name>` below is the field label: Account Status, Open Date, Credit Limit, Expiry Date, Cash Credit Limit, Reissue Date, Current Balance, Current Cycle Credit Limit, Current Cycle Debit Limit, SSN, Date of Birth, FICO Score, First Name, Middle Name, Last Name, Address Line 1, State, Zip, City, Country, Phone Number 1, Phone Number 2, EFT Account Id, Primary Card Holder. |
| FR-ACU-07a | Yes/No fields (status, primary holder): blank → `<Name> must be supplied.`; not Y/N → `<Name> must be Y or N.` |
| FR-ACU-07b | Dates (CSUTLDPY rules). Year blank → `<Name> : Year must be supplied.`; non-numeric → `<Name> must be 4 digit number.`; century not 19/20 → `<Name> : Century is not valid.`. Month blank → `<Name> : Month must be supplied.`; not 1–12 → `<Name>: Month must be a number between 1 and 12.`. Day blank → `<Name> : Day must be supplied.`; not 1–31 → `<Name>:day must be a number between 1 and 31.`. Day 31 in a 30-day month → `<Name>:Cannot have 31 days in this month.`. Feb 30 → `<Name>:Cannot have 30 days in this month.`. Feb 29 in a non-leap year → `<Name>:Not a leap year.Cannot have 29 days in this month.`. Finally a CSUTLDTC check: `<Name> validation error Sev code: <sev> Message code: <msg>`. Date of birth must also be in the past: `Date of Birth:cannot be in the future `. |
| FR-ACU-07c | Signed amounts: blank → `<Name> must be supplied.`; not a valid NUMVAL-C → `<Name> is not valid`. |
| FR-ACU-07d | SSN part 1 (3 digits): required, numeric, non-zero (`SSN: First 3 chars must be supplied.`, `… must be all numeric.`, `… must not be zero.`), and not 000, 666 or 900–999 (`SSN: First 3 chars: should not be 000, 666, or between 900 and 999`). Part 2 (2 digits) and part 3 (4 digits): required, numeric, non-zero (`SSN 4th & 5th chars …`, `SSN Last 4 chars …`). |
| FR-ACU-07e | FICO: required numeric non-zero (3 digits), and between 300 and 850 → `FICO Score: should be between 300 and 850`. |
| FR-ACU-07f | Alphabetic fields: first name, last name, state, city and country are required (`<Name> must be supplied.`) and may contain only letters and spaces (`<Name> can have alphabets only.`). Middle name is optional but alphabetic. Address line 1 is mandatory (any characters). Address line 2 is not validated. |
| FR-ACU-07g | State must be a valid US state code (CSLKPCDY list) → `State: is not a valid state code`. Zip: required, numeric, non-zero (5 digits). If both state and zip are valid, the state plus the first 2 zip digits must be a valid combination (CSLKPCDY) → `Invalid zip code for state`. |
| FR-ACU-07h | Phones (area 3 / prefix 3 / line 4). An all-blank phone is accepted (see KD-08). Area code: required (`<Name>: Area code must be supplied.`), numeric (`…: Area code must be A 3 digit number.`), non-zero (`…: Area code cannot be zero`), and a valid North-American general-purpose area code from the CSLKPCDY list (`…: Not valid North America general purpose area code`). Prefix: required, numeric, non-zero (`…: Prefix code must be supplied.` / `must be A 3 digit number.` / `cannot be zero`). Line number: required, numeric, non-zero (`…: Line number code must be supplied.` / `must be A 4 digit number.` / `cannot be zero`). |
| FR-ACU-07i | EFT account id: required, numeric, non-zero (10 digits). |
| FR-ACU-08 | Any validation error → state E, info `Update account details presented above.` All valid → state N, info `Changes validated.Press F5 to save`, all fields protected. |
| FR-ACU-09 | PF5 in state N. Read the account FOR UPDATE (`Could not lock account record for update` → L), then the customer FOR UPDATE (`Could not lock customer record for update` → L). Compare both records with the old snapshot (case-insensitive for text fields); any difference → `Record changed by some one else. Please review` (state S). Otherwise build the updated records: dates as `YYYY-MM-DD`, phones as `(aaa)bbb-cccc`, SSN as 9 digits. REWRITE the account, then the customer. If the account rewrite fails → `Update of record failed` (F). If the customer rewrite fails → SYNCPOINT ROLLBACK (which also undoes the account) and F. Success → state C, info `Changes committed to database`. |
| FR-ACU-10 | After C/L/F the next interaction starts with an empty search. The info line shows `Changes unsuccessful. Please try again` for L/F. |
| FR-ACU-11 | PF3 → back to the caller with SYNCPOINT (BR-AC-04). PF12 (after a fetch) → re-read and discard edits. PF5 is only valid in state N; all other keys are treated as ENTER. |
| FR-ACU-12 | *Data note:* the shipped sample customers have random FICO scores, area codes and zip codes. 48 of 50 fail the state/zip check and about half fail FICO or area-code checks. An update of these accounts is therefore only accepted after those fields are corrected. |
| KD-08 | *Known defect, kept:* the "phone entirely blank" test checks the area code twice (once instead of the line number), so a phone with blank area and prefix but a non-blank line number is accepted as blank. |
