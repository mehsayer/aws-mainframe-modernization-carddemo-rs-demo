# Online Requirements — Common Behaviour, Sign-on, Menus, User Administration

## Common online behaviour (BR-ONL)

| ID | Requirement |
|-----|------------------------------|
| BR-ONL-001 | Every screen is a 24×80 map. Header: row 1 `Tran :` + transaction id (col 8), title line 1 (col 21, yellow), `Date :` + `mm/dd/yy` (col 71). Row 2 `Prog :` + program name, title line 2, `Time :` + `hh:mm:ss`. Titles: `AWS Mainframe Modernization` and `CardDemo` (COTTL01Y). |
| BR-ONL-002 | Interaction is pseudo-conversational. A program sends its screen and ends with `RETURN TRANSID(<own tranid>) COMMAREA(...)`, and the next key press re-enters the same program. The first entry is detected with `CDEMO-PGM-CONTEXT = 0` (ENTER), and the program sets it to 1 (REENTER) before sending. |
| BR-ONL-003 | Navigating to another program uses XCTL. The caller sets `CDEMO-FROM-TRANID`, `CDEMO-FROM-PROGRAM`, `CDEMO-TO-PROGRAM` and `CDEMO-PGM-CONTEXT = 0`. |
| BR-ONL-004 | If a program is started without a commarea (EIBCALEN = 0), it transfers to sign-on (COSGN00C). The exception is COSGN00C itself, which shows the sign-on screen. |
| BR-ONL-005 | Any key the screen does not handle shows `Invalid key pressed. Please see below...` (CSMSG01Y) in the message line, and the screen is redisplayed. |
| BR-ONL-006 | Field-level errors: the message goes to the error line (row 23, red unless stated otherwise) and the cursor is placed on the field in error. |
| BR-ONL-007 | The commarea (COCOM01Y, 160 bytes) holds from/to tran and program, user id, user type (`A` admin / `U` user), program context, customer id and names, account id and status, card number, and last map/mapset. Programs append their own working fields after these 160 bytes. |
| BR-ONL-008 | Exiting the application (PF3 on sign-on) clears the screen and shows `Thank you for using CardDemo application...`. The task ends without a next transaction. |

## FR-SGN — Sign-on (CC00, COSGN00C, map COSGN00/COSGN0A)

| ID | Requirement |
|-----|------------------------------|
| FR-SGN-01 | First entry: show an empty sign-on screen with the cursor in User ID. The screen also shows the application id and system id. |
| FR-SGN-02 | ENTER with a blank user id → `Please enter User ID ...` (cursor User ID). With a blank password → `Please enter Password ...` (cursor Password). |
| FR-SGN-03 | The user id and password are converted to upper case before lookup and comparison. |
| FR-SGN-04 | Read USRSEC by user id. If it is not found → `User not found. Try again ...`. Any other error → `Unable to verify the User ...`. |
| FR-SGN-05 | Wrong password → `Wrong Password. Try again ...` (cursor Password). |
| FR-SGN-06 | Success: set commarea from-tran `CC00`, from-program `COSGN00C`, user id, user type, context 0. Then XCTL to **COADM01C** if the type is `A`, otherwise to **COMEN01C**. |
| FR-SGN-07 | PF3 → thank-you message, end of session (BR-ONL-008). Any other key → invalid-key message. |

## FR-MEN — Main menu (CM00, COMEN01C, map COMEN01/COMEN1A)

| ID | Requirement |
|-----|------------------------------|
| FR-MEN-01 | Show options 1–11 from COMEN02Y as `NN. <name>`: 1 Account View (COACTVWC), 2 Account Update (COACTUPC), 3 Credit Card List (COCRDLIC), 4 Credit Card View (COCRDSLC), 5 Credit Card Update (COCRDUPC), 6 Transaction List (COTRN00C), 7 Transaction View (COTRN01C), 8 Transaction Add (COTRN02C), 9 Transaction Reports (CORPT00C), 10 Bill Payment (COBIL00C), 11 Pending Authorization View (COPAUS0C). |
| FR-MEN-02 | The option input (2 characters) is right-justified and zero-filled. Non-numeric, 0, or greater than 11 → `Please enter a valid option number...`. |
| FR-MEN-03 | An option flagged admin-only (`A`) chosen by a regular user → `No access - Admin Only option... `. (No base option is flagged `A`.) |
| FR-MEN-04 | Option 11 (COPAUS0C) first checks that the program is installed. *In the base scope it is not installed*, so the message is `This option Pending is not installed...` (red). The text is literally the option name up to its first space. |
| FR-MEN-05 | A program name starting with `DUMMY` → `This option <name-to-first-space>is coming soon ...` (green). |
| FR-MEN-06 | Any other valid option: XCTL to that program with from-tran `CM00`, from-program `COMEN01C`, context 0. |
| FR-MEN-07 | PF3 → XCTL to sign-on (COSGN00C). |

## FR-ADM — Admin menu (CA00, COADM01C, map COADM01/COADM1A)

| ID | Requirement |
|-----|------------------------------|
| FR-ADM-01 | Show options 1–6 from COADM02Y: 1 User List (Security), 2 User Add (Security), 3 User Update (Security), 4 User Delete (Security), 5 Transaction Type List/Update (Db2), 6 Transaction Type Maintenance (Db2). |
| FR-ADM-02 | Option validation is the same as FR-MEN-02, with a maximum of 6. |
| FR-ADM-03 | A valid option XCTLs to its program (from-tran `CA00`, from-program `COADM01C`). If the program does not exist (PGMIDERR, *which is the case for options 5–6 in the base scope*), show `This option is not installed ...` in green. |
| FR-ADM-04 | PF3 → sign-on. |

## FR-USR — User administration (security file USRSEC, record CSUSR01Y: id 8, first name 20, last name 20, password 8, type 1, filler 23)

### FR-USR-LST — User list (CU00, COUSR00C, map COUSR00/COUSR0A)

| ID | Requirement |
|-----|------------------------------|
| FR-USR-LST-01 | Show up to 10 users per page in user-id order: select field, user id, first name, last name, type. The page number is shown. Optional "Search User ID" field: browsing starts at that key, or at the first record if it is blank. |
| FR-USR-LST-02 | First entry and ENTER: read the first page starting at the search key (or the start of the file) and set the page number to 1. |
| FR-USR-LST-03 | Selection: the first row with a non-blank select code is used. `U`/`u` → XCTL COUSR02C (update); `D`/`d` → XCTL COUSR03C (delete). The selected user id is passed in the commarea extension (CDEMO-CU00-USR-SELECTED). Any other code → `Invalid selection. Valid values are U and D`. |
| FR-USR-LST-04 | PF8 (page down): if there is a next page, continue after the last user id shown; otherwise `You are already at the bottom of the page...`. |
| FR-USR-LST-05 | PF7 (page up): if the page number is greater than 1, page back from the first user id shown; otherwise `You are already at the top of the page...`. |
| FR-USR-LST-06 | Hitting the end of the file while filling a page → `You have reached the bottom of the page...` (or `...top of the page...` going backward). The next-page flag is computed by reading one record ahead. |
| FR-USR-LST-07 | PF3 → admin menu (COADM01C). File errors → `Unable to lookup User...`. |

### FR-USR-ADD — Add user (CU01, COUSR01C, map COUSR01/COUSR1A)

| ID | Requirement |
|-----|------------------------------|
| FR-USR-ADD-01 | Fields: first name, last name, user id, password, user type. Validation runs in that order, and the first empty field gives `First Name can NOT be empty...`, `Last Name can NOT be empty...`, `User ID can NOT be empty...`, `Password can NOT be empty...` or `User Type can NOT be empty...`. |
| FR-USR-ADD-02 | Write the USRSEC record (no case conversion). Success → all fields cleared, and the message `User <id> has been added ...` in green. |
| FR-USR-ADD-03 | Duplicate key → `User ID already exist...`. Other error → `Unable to Add User...`. |
| FR-USR-ADD-04 | PF4 clears the screen. PF3 → admin menu. |

### FR-USR-UPD — Update user (CU02, COUSR02C, map COUSR02/COUSR2A)

| ID | Requirement |
|-----|------------------------------|
| FR-USR-UPD-01 | On first entry with a user preselected from the list, fetch that user automatically. ENTER fetches the user in the User ID field: blank → `User ID can NOT be empty...`; not found → `User ID NOT found...`; found → show the fields with `Press PF5 key to save your updates ...` (neutral colour). |
| FR-USR-UPD-02 | PF5 validates all fields (user id, first name, last name, password, type; same messages as FR-USR-ADD-01). It re-reads the record and compares each field. If nothing changed → `Please modify to update ...` (red). Otherwise REWRITE → `User <id> has been updated ...` (green). |
| FR-USR-UPD-03 | PF3 = save (as PF5), then return to the calling program (or the admin menu if none). PF12 = return to the admin menu without saving. PF4 = clear. |
| FR-USR-UPD-04 | Errors: `Unable to lookup User...`, `Unable to Update User...`. |

### FR-USR-DEL — Delete user (CU03, COUSR03C, map COUSR03/COUSR3A)

| ID | Requirement |
|-----|------------------------------|
| FR-USR-DEL-01 | On first entry with a user preselected from the list, fetch it. ENTER fetches by the User ID field: blank → `User ID can NOT be empty...`; not found → `User ID NOT found...`; found → show first name, last name and type with `Press PF5 key to delete this user ...` (neutral). The password is never shown. |
| FR-USR-DEL-02 | PF5: validate the user id, read the record for update, then DELETE. Success → fields cleared, and `User <id> has been deleted ...` (green). Not found → `User ID NOT found...`. Other error → `Unable to Update User...` (literal text of the original). |
| FR-USR-DEL-03 | PF3 → return to the calling program (or the admin menu). PF12 → admin menu. PF4 → clear. |
