"""Manual test cases for CardDemo (Rust). Each case: id, title, requirements, precondition,
steps as (action, expected). Rendered by build.py into manual-test-suite.md/.pdf.

Conventions: "Reset" = `carddemo reset`. Online tests start with `carddemo online`.
U1 = USER0001/PASSWORD (regular user), A1 = ADMIN001/PASSWORD (admin).
Account numbers and card numbers must be typed in full (11 / 16 digits)."""

R = "Data reset (`carddemo reset`)"
U = R + "; signed on as USER0001 / PASSWORD (main menu shown)"
A = R + "; signed on as ADMIN001 / PASSWORD (admin menu shown)"

CASES = [
# ---------------------------------------------------------------- sign-on
("TC-SGN-01", "Sign-on screen layout", "FR-SGN-01, BR-ONL-001", R, [
  ("Start `carddemo online`.", "Sign-on screen: `Tran : CC00`, `Prog : COSGN00C`, titles, today's date `mm/dd/yy` and time, AppID `CARDDEMO`, SysID; the cursor is in User ID; the password field shows `________`."),
]),
("TC-SGN-02", "Mandatory user id / password", "FR-SGN-02", R, [
  ("Press Enter with all fields empty.", "`Please enter User ID ...`, cursor in User ID."),
  ("Type `USER0001` in User ID, erase the password field (Ctrl+E), press Enter.", "`Please enter Password ...`, cursor in Password."),
  ("Type User ID `USER0001`, leave the password field untouched (underscores), press Enter.", "`Wrong Password. Try again ...` (the underscores are transmitted: FSET field)."),
]),
("TC-SGN-03", "Unknown user and wrong password", "FR-SGN-04, FR-SGN-05", R, [
  ("User ID `NOBODY`, password `X`, Enter.", "`User not found. Try again ...`"),
  ("User ID `USER0001`, password `WRONG`, Enter.", "`Wrong Password. Try again ...`, cursor in Password."),
]),
("TC-SGN-04", "Case-insensitive sign-on and role routing", "FR-SGN-03, FR-SGN-06", R, [
  ("User ID `user0001`, password `password`, Enter.", "Main Menu (`CM00`, `COMEN01C`)."),
  ("F3, then sign on as `admin001` / `password`.", "Admin Menu (`CA00`, `COADM01C`)."),
]),
("TC-SGN-05", "Exit and invalid key", "FR-SGN-07, BR-ONL-005, BR-ONL-008", R, [
  ("On the sign-on screen press F5.", "`Invalid key pressed. Please see below...`"),
  ("Press F3.", "Screen cleared: `Thank you for using CardDemo application...`; the session ends."),
  ("Press Enter.", "A new sign-on screen is shown."),
]),
# ---------------------------------------------------------------- menus
("TC-MEN-01", "Main menu options", "FR-MEN-01", U, [
  ("Inspect the menu.", "Options `01. Account View` ... `11. Pending Authorization View` are listed."),
]),
("TC-MEN-02", "Option validation", "FR-MEN-02, BR-ONL-005", U, [
  ("Type `99`, Enter.", "`Please enter a valid option number...`"),
  ("Type `0`, Enter.", "`Please enter a valid option number...`"),
  ("Type `ab`, Enter.", "`Please enter a valid option number...`"),
  ("Press F7.", "`Invalid key pressed. Please see below...`"),
]),
("TC-MEN-03", "Optional module not installed", "FR-MEN-04", U, [
  ("Type `11`, Enter.", "Red message `This option Pending is not installed...`"),
]),
("TC-MEN-04", "Navigation and return", "FR-MEN-06, FR-MEN-07", U, [
  ("Type `1` (single digit), Enter.", "Account View screen (`CAVW`). The option was right-justified to `01`."),
  ("Press F3.", "Main menu."),
  ("Press F3.", "Sign-on screen."),
]),
("TC-ADM-01", "Admin menu", "FR-ADM-01..04", A, [
  ("Inspect the menu.", "Options 1-6 (User List/Add/Update/Delete (Security), Transaction Type ... (Db2))."),
  ("Type `5`, Enter.", "Green message `This option is not installed ...` (Db2 module out of scope)."),
  ("Type `7`, Enter.", "`Please enter a valid option number...`"),
  ("Press F3.", "Sign-on screen."),
]),
# ---------------------------------------------------------------- users
("TC-USR-01", "User list and paging", "FR-USR-LST-01, -02, -04, -05, -06", A, [
  ("Option `1`.", "User list page `00000001`: ADMIN001 ... ADMIN005, USER0001 ... USER0005."),
  ("Press F7.", "`You are already at the top of the page...`"),
  ("Press F8.", "Rows are blank and `You have reached the bottom of the page...` (KD-11)."),
  ("Press F8 again.", "`You are already at the bottom of the page...`"),
  ("Press Enter.", "The first page is listed again."),
]),
("TC-USR-02", "Search from a user id", "FR-USR-LST-01", A + "; user list shown", [
  ("Type `USER0003` in Search User ID, Enter.", "The list starts at USER0003."),
]),
("TC-USR-03", "Search beyond the last key abends (original defect)", "KD-10", A + "; user list shown", [
  ("Type `ZZZZZZZZ` in Search User ID, Enter.", "The screen shows `... Transaction CU00 failed with abend AEIP ...`. Enter returns to sign-on."),
]),
("TC-USR-04", "Selection codes", "FR-USR-LST-03", A + "; user list shown", [
  ("Type `X` next to USER0001, Enter.", "`You have reached the bottom of the page...`: with exactly 10 users the read-ahead hits end of file and its message replaces `Invalid selection. Valid values are U and D` (original behaviour)."),
  ("Type `u` next to USER0001, Enter.", "Update User screen with USER0001 pre-filled and `Press PF5 key to save your updates ...`"),
]),
("TC-USR-05", "Add user: validation and duplicates", "FR-USR-ADD-01..04", A, [
  ("Option `2`; press Enter.", "`First Name can NOT be empty...`"),
  ("Fill First `Ada`, Last `Lovelace`, User ID `USER0099`, Password `secret`, Type `U`; Enter.", "Green `User USER0099 has been added ...`, fields cleared."),
  ("Repeat the same input.", "`User ID already exist...`"),
  ("Press F4.", "Fields cleared."),
]),
("TC-USR-06", "Update user", "FR-USR-UPD-01..04", A, [
  ("Option `3`, User ID `USER0002`, Enter.", "Fields shown, `Press PF5 key to save your updates ...`"),
  ("Press F5 without changes.", "Red `Please modify to update ...`"),
  ("Change First Name, press F5.", "Green `User USER0002 has been updated ...`"),
  ("User ID `NOBODY99`, Enter.", "`User ID NOT found...`"),
]),
("TC-USR-07", "Delete user", "FR-USR-DEL-01..03", A + "; USER0099 added (TC-USR-05)", [
  ("Option `4`, User ID `USER0099`, Enter.", "Names and type are shown (no password), `Press PF5 key to delete this user ...`"),
  ("Press F5.", "Green `User USER0099 has been deleted ...`"),
  ("Enter again with `USER0099`.", "`User ID NOT found...`"),
]),
# ---------------------------------------------------------------- accounts
("TC-ACV-01", "Account view: input checks", "FR-ACV-01, -02, BR-AC-07", U, [
  ("Option `1`.", "Info `Enter or update id of account to display`. Row 1 shows only `W` of the transaction id (map quirk FR-ACV-06)."),
  ("Enter with an empty account.", "`Account number not provided`, red `*` in the account field."),
  ("Account `abc`, Enter.", "`Account Filter must be a non-zero 11 digit number`"),
]),
("TC-ACV-02", "Account view: display", "FR-ACV-03, -04", U + "; account view", [
  ("Account `00000000001`, Enter.", "Status `Y`, balance and limits in `+ZZZ,ZZZ,ZZZ.99` form (current balance `+        194.00`), dates, customer name, SSN as `999-99-9999`, address, phones."),
  ("Account `99999999999`, Enter.", "`Account:99999999999 not found in Cross ref file.  Resp:000000013  Reas:0000` (two spaces before `Resp:` and `Reas:`; the message is cut at 75 characters)."),
  ("Press F3.", "Main menu."),
]),
("TC-ACU-01", "Account update: fetch and no-change", "FR-ACU-01..06", U, [
  ("Option `2`.", "Info `Enter or update id of account to update`."),
  ("Account `0`, Enter.", "`Account Number if supplied must be a 11 digit Non-Zero Number`"),
  ("Account `00000000001`, Enter.", "Details shown, info `Update account details presented above.`"),
  ("Press Enter without changes.", "`No change detected with respect to values fetched.`"),
]),
("TC-ACU-02", "Account update: validation order", "FR-ACU-07a..07i, FR-ACU-12", U + "; account 00000000001 fetched", [
  ("Set Primary Card Holder `X`, Enter.", "`FICO Score: should be between 300 and 850` (sample data FICO is invalid and is checked first)."),
  ("Set FICO `750`, State `NY`, Zip `10001`, Phone 1 `212`/`555`/`0100`, Phone 2 `212`/`555`/`0101`, Primary `X`, Enter.", "`Primary Card Holder must be Y or N.`, the field is red."),
  ("Set Open Date month `13`, Enter.", "`Open Date: Month must be a number between 1 and 12.`"),
  ("Set Date of Birth to a future year, Enter.", "`Date of Birth:cannot be in the future `"),
  ("Set SSN first part `666`, Enter.", "`SSN: First 3 chars: should not be 000, 666, or between 900 and 999`"),
  ("Set Phone 1 area code `123`, Enter.", "`Phone Number 1: Not valid North America general purpose area code`"),
  ("Set State `NY`, Zip `90210`, Enter.", "`Invalid zip code for state`"),
]),
("TC-ACU-03", "Account update: commit", "FR-ACU-08, -09, -10", U + "; account 00000000001 fetched", [
  ("Fix FICO/State/Zip/Phones as in TC-ACU-02, Primary `Y`, Credit Limit `5000`, Enter.", "Info `Changes validated.Press F5 to save`; all fields protected."),
  ("Press F5.", "Info `Changes committed to database`."),
  ("Press Enter; then view account 00000000001 (option 1).", "The empty search screen is shown; account view shows credit limit `+      5,000.00`."),
]),
# ---------------------------------------------------------------- cards
("TC-CDL-01", "Card list and paging", "FR-CDL-01, -04, -05, -07", U, [
  ("Option `3`.", "7 cards in card-number order, page `1`, info `TYPE S FOR DETAIL, U TO UPDATE ANY RECORD`."),
  ("Press F7.", "`NO PREVIOUS PAGES TO DISPLAY`"),
  ("Press F8, then F7.", "Page 2 shown, then page 1 again with the same first card."),
]),
("TC-CDL-02", "Card list filters and selection errors", "FR-CDL-02, -03, -06", U + "; card list", [
  ("Account filter `abc`, Enter.", "`ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER`, field red."),
  ("Account filter `00000000002`, Enter.", "Only cards of account 2 are listed."),
  ("Clear the filter; type `S` on row 1 and `U` on row 2, Enter.", "`PLEASE SELECT ONLY ONE RECORD TO VIEW OR UPDATE`, both rows red."),
  ("Type `X` on a row, Enter.", "`INVALID ACTION CODE`"),
]),
("TC-CDV-01", "Card detail from the list", "FR-CDV-01, -05, -06", U + "; card list", [
  ("`S` on row 1, Enter.", "Card detail: info ` Displaying requested details`, embossed name, expiry month/year, status; search fields protected."),
  ("Press F3.", "Card list."),
]),
("TC-CDV-02", "Card detail by keys", "FR-CDV-02..04", U, [
  ("Option `4`.", "Info `Please enter Account and Card Number`."),
  ("Enter with both fields empty.", "`Account number not provided`, both fields red `*`."),
  ("Account `00000000001`, card `1234`, Enter.", "`CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER`"),
]),
("TC-CDU-01", "Card update cycle", "FR-CDU-01..07, KD-07", U + "; card list", [
  ("`U` on row 1, Enter.", "Info `Details of selected card shown above`."),
  ("Enter without changes.", "`No change detected with respect to values fetched.`"),
  ("Status `X`, Enter.", "`Card Active Status must be Y or N`"),
  ("Name `JOHN 2`, Enter.", "`Card name can only contain alphabets and spaces`"),
  ("Expiry month `13`, Enter.", "`Card expiry month must be between 1 and 12`"),
  ("Valid values (status `N`), Enter.", "Info `Changes validated.Press F5 to save`."),
  ("Press F5.", "Info `Changes committed to database`. The record's CVV is now `000` (KD-07)."),
  ("Press Enter.", "Back on the card list (called from the list)."),
]),
# ---------------------------------------------------------------- transactions
("TC-TRL-01", "Transaction list", "FR-TRL-01..05", U + "; daily posting run (TC-BAT-01)", [
  ("Option `6`.", "Page `00000001`. Row 1 is the initialisation record from DALYTRAN.PS.INIT (key X'00', shown blank). Ten rows per page."),
  ("Search Tran ID `abc`, Enter.", "`Tran ID must be Numeric ...`"),
  ("F8 / F7.", "Next / previous page."),
  ("`S` on row 2, Enter.", "Transaction View with that transaction."),
]),
("TC-TRV-01", "Transaction view", "FR-TRV-01..04", U, [
  ("Option `7`, Enter with an empty id.", "`Tran ID can NOT be empty...`"),
  ("Id `9999999999999999`, Enter.", "`Transaction ID NOT found...`"),
  ("A valid id from the list, Enter.", "All fields shown, amount `+99999999.99`."),
  ("F4 / F5.", "Screen cleared / transaction list."),
]),
("TC-TRA-01", "Transaction add: validations", "FR-TRA-01..07, BR-TRA-0", U, [
  ("Option `8`, Enter.", "`Account or Card Number must be entered...`"),
  ("Account `00000000001`, Enter.", "Card number filled from XREF; `Type CD can NOT be empty...`"),
  ("Fill all fields but amount `12.50`, Enter.", "`Amount should be in format -99999999.99`"),
  ("Amount `+00000012.50`, Orig Date `2022-02-30`, Enter.", "`Orig Date - Not a valid date...`"),
  ("Valid dates, Confirm empty, Enter.", "`Confirm to add this transaction...`"),
]),
("TC-TRA-02", "Transaction add: copy last and add", "FR-TRA-08..10", U, [
  ("Option `8`, account `00000000001`, press F5.", "Fields copied from the last transaction; `Confirm to add this transaction...`"),
  ("Confirm `Y`, Enter.", "Green `Transaction added successfully.  Your Tran ID is <n>.`, where n = highest id + 1 as 16 digits."),
]),
("TC-BIL-01", "Bill payment", "FR-BIL-01..08", U, [
  ("Option `10`, Enter.", "`Acct ID can NOT be empty...`"),
  ("Account `00000000001`, Enter.", "Balance `+0000000194.00`, `Confirm to make a bill payment...`"),
  ("Confirm `Y`, Enter.", "Green `Payment successful.  Your Transaction ID is <n>.`"),
  ("Account `00000000001`, Enter.", "`You have nothing to pay...`"),
]),
("TC-RPT-01", "Report submission", "FR-RPT-01..07, BR-RPT-0", U, [
  ("Option `9`, Enter.", "`Select a report type to print report...`"),
  ("`X` in Monthly, Enter.", "`Please confirm to print the Monthly report...`"),
  ("`X` in Monthly, Confirm `Y`, Enter.", "Green `Monthly report submitted for printing ...`; the status line names the report data set (internal reader ran TRANREPT)."),
  ("Custom with start month `13`, Enter.", "`Start Date - Not a valid Month...`"),
  ("Custom 2022-01-01 .. 2022-12-31, Confirm `X`.", "`\"X\" is not a valid value to confirm...`"),
]),
# ---------------------------------------------------------------- batch
("TC-BAT-01", "Daily posting", "FR-BAT-POST-01..11", R, [
  ("`carddemo job TRANBKP` then `carddemo job POSTTRAN`.", "`TRANSACTIONS PROCESSED :000000300`, `TRANSACTIONS REJECTED  :000000038`, POSTTRAN RC=0004."),
  ("Inspect `datasets/AWS.M2.CARDDEMO.DALYREJS.G0001V00`.", "38 records of 430 bytes; the trailer holds reason 100-103 and its description."),
  ("Online: transaction list.", "The posted transactions are listed; account balances changed accordingly."),
]),
("TC-BAT-02", "Interest calculation", "FR-BAT-INT-01..07, KD-01", "TC-BAT-01 done", [
  ("`carddemo job INTCALC --parm 2022071800`.", "RC=0000; SYSTRAN generation written with ids `2022071800000001`..., type 01, category 0005, description `Int. for a/c <acct>`."),
  ("Check the last account in TCATBAL order (online account view).", "Its cycle credit/debit are NOT reset (KD-01); other accounts are reset to 0."),
]),
("TC-BAT-03", "Combine and statements", "FR-BAT-STMT-01..07", "TC-BAT-02 done", [
  ("`carddemo job TRANBKP`, `carddemo job COMBTRAN`, `carddemo job CREASTMT`.", "All RC=0000."),
  ("Open `STATEMNT.PS` and `STATEMNT.HTML`.", "One statement per card/account with name, address, balance, FICO, transaction lines and `Total EXP:`; the HTML shows name/address, basic details and the transactions as a table (it has no total line)."),
]),
("TC-BAT-04", "Transaction report", "FR-BAT-REPT-01..08, KD-02, KD-03", "TC-BAT-01 done", [
  ("`carddemo job TRANREPT --start 2022-01-01 --end 2099-12-31`.", "Report with header, page totals every 20 lines, account totals and `Grand Total`."),
  ("Check the totals.", "The last transaction's amount is counted twice in the final page total (KD-02); there is no final account total (KD-03)."),
]),
("TC-BAT-05", "File print utilities", "FR-BAT-RDR-01..03, FR-BAT-VER-01..02", R, [
  ("`carddemo job READACCT` / `READCARD` / `READXREF` / `READCUST` / `DALYVRFY`.", "Record dumps as described; READXREF and READCUST print each record twice; DALYVRFY shows card, account and customer for each of the 300 daily transactions, and once more for the last one after end of file (FR-BAT-VER-02); all cards verify with the sample data."),
]),
("TC-BAT-06", "Export / import round trip", "FR-BAT-XPRT-01..05, KD-09", R, [
  ("`carddemo job CBEXPORT`.", "`CBEXPORT: Total Records Exported:` = customers + accounts + xrefs + transactions + cards."),
  ("`carddemo job CBIMPORT`.", "`Errors Written: 000000000`, `Unknown Record Types: 000000000`; the *.IMPORT data sets equal the master files in every named field (FILLER bytes are X'00')."),
]),
("TC-BAT-07", "Reload jobs and reset", "FR-BAT-UTIL-03, NFR-08", R, [
  ("`carddemo job ACCTFILE` (and the other reload jobs).", "The cluster is reloaded from the sample data."),
  ("`time carddemo reset`.", "Completes in well under a second."),
]),
("TC-BAT-08", "Differential test against the original COBOL", "NFR-01, NFR-13, NFR-14, KD-12", "GnuCOBOL 3.x installed; `cargo build --release`", [
  ("In `carddemo-rs`: `python3 difftest/run.py`.", "Builds the original batch programs with GnuCOBOL, runs them and the Rust port on the same data, and prints the report (also in `difftest/out/report.md`)."),
  ("Read the result table.", "Every row is `identical` (one row marked with an accepted GnuCOBOL difference, explained under the table); `Overall: all outputs identical`; exit code 0."),
]),
]
