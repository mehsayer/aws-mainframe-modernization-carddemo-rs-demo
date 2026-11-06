# Data Dictionary

Field layouts generated from the copybooks by `rust/tools/copybook.py`. Offsets are 1-based. COMP/COMP-3 lengths are storage bytes.

### SEC-USER-DATA — `CSUSR01Y.cpy` (80 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 8 | SEC-USR-ID | X(08) |  |
| 9 | 20 | SEC-USR-FNAME | X(20) |  |
| 29 | 20 | SEC-USR-LNAME | X(20) |  |
| 49 | 8 | SEC-USR-PWD | X(08) |  |
| 57 | 1 | SEC-USR-TYPE | X(01) |  |
| 58 | 23 | SEC-USR-FILLER | X(23) |  |

### ACCOUNT-RECORD — `CVACT01Y.cpy` (300 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 11 | ACCT-ID | 9(11) |  |
| 12 | 1 | ACCT-ACTIVE-STATUS | X(01) |  |
| 13 | 12 | ACCT-CURR-BAL | S9(10)V99 |  |
| 25 | 12 | ACCT-CREDIT-LIMIT | S9(10)V99 |  |
| 37 | 12 | ACCT-CASH-CREDIT-LIMIT | S9(10)V99 |  |
| 49 | 10 | ACCT-OPEN-DATE | X(10) |  |
| 59 | 10 | ACCT-EXPIRAION-DATE | X(10) |  |
| 69 | 10 | ACCT-REISSUE-DATE | X(10) |  |
| 79 | 12 | ACCT-CURR-CYC-CREDIT | S9(10)V99 |  |
| 91 | 12 | ACCT-CURR-CYC-DEBIT | S9(10)V99 |  |
| 103 | 10 | ACCT-ADDR-ZIP | X(10) |  |
| 113 | 10 | ACCT-GROUP-ID | X(10) |  |
| 123 | 178 | FILLER | X(178) |  |

### CARD-RECORD — `CVACT02Y.cpy` (150 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 16 | CARD-NUM | X(16) |  |
| 17 | 11 | CARD-ACCT-ID | 9(11) |  |
| 28 | 3 | CARD-CVV-CD | 9(03) |  |
| 31 | 50 | CARD-EMBOSSED-NAME | X(50) |  |
| 81 | 10 | CARD-EXPIRAION-DATE | X(10) |  |
| 91 | 1 | CARD-ACTIVE-STATUS | X(01) |  |
| 92 | 59 | FILLER | X(59) |  |

### CARD-XREF-RECORD — `CVACT03Y.cpy` (50 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 16 | XREF-CARD-NUM | X(16) |  |
| 17 | 9 | XREF-CUST-ID | 9(09) |  |
| 26 | 11 | XREF-ACCT-ID | 9(11) |  |
| 37 | 14 | FILLER | X(14) |  |

### CUSTOMER-RECORD — `CVCUS01Y.cpy` (500 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 9 | CUST-ID | 9(09) |  |
| 10 | 25 | CUST-FIRST-NAME | X(25) |  |
| 35 | 25 | CUST-MIDDLE-NAME | X(25) |  |
| 60 | 25 | CUST-LAST-NAME | X(25) |  |
| 85 | 50 | CUST-ADDR-LINE-1 | X(50) |  |
| 135 | 50 | CUST-ADDR-LINE-2 | X(50) |  |
| 185 | 50 | CUST-ADDR-LINE-3 | X(50) |  |
| 235 | 2 | CUST-ADDR-STATE-CD | X(02) |  |
| 237 | 3 | CUST-ADDR-COUNTRY-CD | X(03) |  |
| 240 | 10 | CUST-ADDR-ZIP | X(10) |  |
| 250 | 15 | CUST-PHONE-NUM-1 | X(15) |  |
| 265 | 15 | CUST-PHONE-NUM-2 | X(15) |  |
| 280 | 9 | CUST-SSN | 9(09) |  |
| 289 | 20 | CUST-GOVT-ISSUED-ID | X(20) |  |
| 309 | 10 | CUST-DOB-YYYY-MM-DD | X(10) |  |
| 319 | 10 | CUST-EFT-ACCOUNT-ID | X(10) |  |
| 329 | 1 | CUST-PRI-CARD-HOLDER-IND | X(01) |  |
| 330 | 3 | CUST-FICO-CREDIT-SCORE | 9(03) |  |
| 333 | 168 | FILLER | X(168) |  |

### TRAN-CAT-BAL-RECORD — `CVTRA01Y.cpy` (50 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 17 | TRAN-CAT-KEY | group |  |
| 1 | 11 | &nbsp;&nbsp;TRANCAT-ACCT-ID | 9(11) |  |
| 12 | 2 | &nbsp;&nbsp;TRANCAT-TYPE-CD | X(02) |  |
| 14 | 4 | &nbsp;&nbsp;TRANCAT-CD | 9(04) |  |
| 18 | 11 | TRAN-CAT-BAL | S9(09)V99 |  |
| 29 | 22 | FILLER | X(22) |  |

### DIS-GROUP-RECORD — `CVTRA02Y.cpy` (50 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 16 | DIS-GROUP-KEY | group |  |
| 1 | 10 | &nbsp;&nbsp;DIS-ACCT-GROUP-ID | X(10) |  |
| 11 | 2 | &nbsp;&nbsp;DIS-TRAN-TYPE-CD | X(02) |  |
| 13 | 4 | &nbsp;&nbsp;DIS-TRAN-CAT-CD | 9(04) |  |
| 17 | 6 | DIS-INT-RATE | S9(04)V99 |  |
| 23 | 28 | FILLER | X(28) |  |

### TRAN-TYPE-RECORD — `CVTRA03Y.cpy` (60 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 2 | TRAN-TYPE | X(02) |  |
| 3 | 50 | TRAN-TYPE-DESC | X(50) |  |
| 53 | 8 | FILLER | X(08) |  |

### TRAN-CAT-RECORD — `CVTRA04Y.cpy` (60 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 6 | TRAN-CAT-KEY | group |  |
| 1 | 2 | &nbsp;&nbsp;TRAN-TYPE-CD | X(02) |  |
| 3 | 4 | &nbsp;&nbsp;TRAN-CAT-CD | 9(04) |  |
| 7 | 50 | TRAN-CAT-TYPE-DESC | X(50) |  |
| 57 | 4 | FILLER | X(04) |  |

### TRAN-RECORD — `CVTRA05Y.cpy` (350 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 16 | TRAN-ID | X(16) |  |
| 17 | 2 | TRAN-TYPE-CD | X(02) |  |
| 19 | 4 | TRAN-CAT-CD | 9(04) |  |
| 23 | 10 | TRAN-SOURCE | X(10) |  |
| 33 | 100 | TRAN-DESC | X(100) |  |
| 133 | 11 | TRAN-AMT | S9(09)V99 |  |
| 144 | 9 | TRAN-MERCHANT-ID | 9(09) |  |
| 153 | 50 | TRAN-MERCHANT-NAME | X(50) |  |
| 203 | 50 | TRAN-MERCHANT-CITY | X(50) |  |
| 253 | 10 | TRAN-MERCHANT-ZIP | X(10) |  |
| 263 | 16 | TRAN-CARD-NUM | X(16) |  |
| 279 | 26 | TRAN-ORIG-TS | X(26) |  |
| 305 | 26 | TRAN-PROC-TS | X(26) |  |
| 331 | 20 | FILLER | X(20) |  |

### DALYTRAN-RECORD — `CVTRA06Y.cpy` (350 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 16 | DALYTRAN-ID | X(16) |  |
| 17 | 2 | DALYTRAN-TYPE-CD | X(02) |  |
| 19 | 4 | DALYTRAN-CAT-CD | 9(04) |  |
| 23 | 10 | DALYTRAN-SOURCE | X(10) |  |
| 33 | 100 | DALYTRAN-DESC | X(100) |  |
| 133 | 11 | DALYTRAN-AMT | S9(09)V99 |  |
| 144 | 9 | DALYTRAN-MERCHANT-ID | 9(09) |  |
| 153 | 50 | DALYTRAN-MERCHANT-NAME | X(50) |  |
| 203 | 50 | DALYTRAN-MERCHANT-CITY | X(50) |  |
| 253 | 10 | DALYTRAN-MERCHANT-ZIP | X(10) |  |
| 263 | 16 | DALYTRAN-CARD-NUM | X(16) |  |
| 279 | 26 | DALYTRAN-ORIG-TS | X(26) |  |
| 305 | 26 | DALYTRAN-PROC-TS | X(26) |  |
| 331 | 20 | FILLER | X(20) |  |

### TRNX-RECORD — `COSTM01.CPY` (350 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 32 | TRNX-KEY | group |  |
| 1 | 16 | &nbsp;&nbsp;TRNX-CARD-NUM | X(16) |  |
| 17 | 16 | &nbsp;&nbsp;TRNX-ID | X(16) |  |
| 33 | 318 | TRNX-REST | group |  |
| 33 | 2 | &nbsp;&nbsp;TRNX-TYPE-CD | X(02) |  |
| 35 | 4 | &nbsp;&nbsp;TRNX-CAT-CD | 9(04) |  |
| 39 | 10 | &nbsp;&nbsp;TRNX-SOURCE | X(10) |  |
| 49 | 100 | &nbsp;&nbsp;TRNX-DESC | X(100) |  |
| 149 | 11 | &nbsp;&nbsp;TRNX-AMT | S9(09)V99 |  |
| 160 | 9 | &nbsp;&nbsp;TRNX-MERCHANT-ID | 9(09) |  |
| 169 | 50 | &nbsp;&nbsp;TRNX-MERCHANT-NAME | X(50) |  |
| 219 | 50 | &nbsp;&nbsp;TRNX-MERCHANT-CITY | X(50) |  |
| 269 | 10 | &nbsp;&nbsp;TRNX-MERCHANT-ZIP | X(10) |  |
| 279 | 26 | &nbsp;&nbsp;TRNX-ORIG-TS | X(26) |  |
| 305 | 26 | &nbsp;&nbsp;TRNX-PROC-TS | X(26) |  |
| 331 | 20 | &nbsp;&nbsp;FILLER | X(20) |  |

### EXPORT-RECORD — `CVEXPORT.cpy` (500 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 1 | EXPORT-REC-TYPE | X(1) |  |
| 2 | 26 | EXPORT-TIMESTAMP | X(26) |  |
| 2 | 26 | EXPORT-TIMESTAMP-R | group | REDEFINES EXPORT-TIMESTAMP |
| 2 | 10 | &nbsp;&nbsp;EXPORT-DATE | X(10) |  |
| 12 | 1 | &nbsp;&nbsp;EXPORT-DATE-TIME-SEP | X(1) |  |
| 13 | 15 | &nbsp;&nbsp;EXPORT-TIME | X(15) |  |
| 28 | 4 | EXPORT-SEQUENCE-NUM | 9(9) COMP |  |
| 32 | 4 | EXPORT-BRANCH-ID | X(4) |  |
| 36 | 5 | EXPORT-REGION-CODE | X(5) |  |
| 41 | 460 | EXPORT-RECORD-DATA | X(460) |  |
| 41 | 460 | EXPORT-CUSTOMER-DATA | group | REDEFINES EXPORT-RECORD-DATA |
| 41 | 4 | &nbsp;&nbsp;EXP-CUST-ID | 9(09) COMP |  |
| 45 | 25 | &nbsp;&nbsp;EXP-CUST-FIRST-NAME | X(25) |  |
| 70 | 25 | &nbsp;&nbsp;EXP-CUST-MIDDLE-NAME | X(25) |  |
| 95 | 25 | &nbsp;&nbsp;EXP-CUST-LAST-NAME | X(25) |  |
| 120 | 50 | &nbsp;&nbsp;EXP-CUST-ADDR-LINES | group | OCCURS 3 |
| 120 | 50 | &nbsp;&nbsp;&nbsp;&nbsp;EXP-CUST-ADDR-LINE | X(50) |  |
| 270 | 2 | &nbsp;&nbsp;EXP-CUST-ADDR-STATE-CD | X(02) |  |
| 272 | 3 | &nbsp;&nbsp;EXP-CUST-ADDR-COUNTRY-CD | X(03) |  |
| 275 | 10 | &nbsp;&nbsp;EXP-CUST-ADDR-ZIP | X(10) |  |
| 285 | 15 | &nbsp;&nbsp;EXP-CUST-PHONE-NUMS | group | OCCURS 2 |
| 285 | 15 | &nbsp;&nbsp;&nbsp;&nbsp;EXP-CUST-PHONE-NUM | X(15) |  |
| 315 | 9 | &nbsp;&nbsp;EXP-CUST-SSN | 9(09) |  |
| 324 | 20 | &nbsp;&nbsp;EXP-CUST-GOVT-ISSUED-ID | X(20) |  |
| 344 | 10 | &nbsp;&nbsp;EXP-CUST-DOB-YYYY-MM-DD | X(10) |  |
| 354 | 10 | &nbsp;&nbsp;EXP-CUST-EFT-ACCOUNT-ID | X(10) |  |
| 364 | 1 | &nbsp;&nbsp;EXP-CUST-PRI-CARD-HOLDER-IND | X(01) |  |
| 365 | 2 | &nbsp;&nbsp;EXP-CUST-FICO-CREDIT-SCORE | 9(03) COMP-3 |  |
| 367 | 134 | &nbsp;&nbsp;FILLER | X(134) |  |
| 41 | 460 | EXPORT-ACCOUNT-DATA | group | REDEFINES EXPORT-RECORD-DATA |
| 41 | 11 | &nbsp;&nbsp;EXP-ACCT-ID | 9(11) |  |
| 52 | 1 | &nbsp;&nbsp;EXP-ACCT-ACTIVE-STATUS | X(01) |  |
| 53 | 7 | &nbsp;&nbsp;EXP-ACCT-CURR-BAL | S9(10)V99 COMP-3 |  |
| 60 | 12 | &nbsp;&nbsp;EXP-ACCT-CREDIT-LIMIT | S9(10)V99 |  |
| 72 | 7 | &nbsp;&nbsp;EXP-ACCT-CASH-CREDIT-LIMIT | S9(10)V99 COMP-3 |  |
| 79 | 10 | &nbsp;&nbsp;EXP-ACCT-OPEN-DATE | X(10) |  |
| 89 | 10 | &nbsp;&nbsp;EXP-ACCT-EXPIRAION-DATE | X(10) |  |
| 99 | 10 | &nbsp;&nbsp;EXP-ACCT-REISSUE-DATE | X(10) |  |
| 109 | 12 | &nbsp;&nbsp;EXP-ACCT-CURR-CYC-CREDIT | S9(10)V99 |  |
| 121 | 8 | &nbsp;&nbsp;EXP-ACCT-CURR-CYC-DEBIT | S9(10)V99 COMP |  |
| 129 | 10 | &nbsp;&nbsp;EXP-ACCT-ADDR-ZIP | X(10) |  |
| 139 | 10 | &nbsp;&nbsp;EXP-ACCT-GROUP-ID | X(10) |  |
| 149 | 352 | &nbsp;&nbsp;FILLER | X(352) |  |
| 41 | 460 | EXPORT-TRANSACTION-DATA | group | REDEFINES EXPORT-RECORD-DATA |
| 41 | 16 | &nbsp;&nbsp;EXP-TRAN-ID | X(16) |  |
| 57 | 2 | &nbsp;&nbsp;EXP-TRAN-TYPE-CD | X(02) |  |
| 59 | 4 | &nbsp;&nbsp;EXP-TRAN-CAT-CD | 9(04) |  |
| 63 | 10 | &nbsp;&nbsp;EXP-TRAN-SOURCE | X(10) |  |
| 73 | 100 | &nbsp;&nbsp;EXP-TRAN-DESC | X(100) |  |
| 173 | 6 | &nbsp;&nbsp;EXP-TRAN-AMT | S9(09)V99 COMP-3 |  |
| 179 | 4 | &nbsp;&nbsp;EXP-TRAN-MERCHANT-ID | 9(09) COMP |  |
| 183 | 50 | &nbsp;&nbsp;EXP-TRAN-MERCHANT-NAME | X(50) |  |
| 233 | 50 | &nbsp;&nbsp;EXP-TRAN-MERCHANT-CITY | X(50) |  |
| 283 | 10 | &nbsp;&nbsp;EXP-TRAN-MERCHANT-ZIP | X(10) |  |
| 293 | 16 | &nbsp;&nbsp;EXP-TRAN-CARD-NUM | X(16) |  |
| 309 | 26 | &nbsp;&nbsp;EXP-TRAN-ORIG-TS | X(26) |  |
| 335 | 26 | &nbsp;&nbsp;EXP-TRAN-PROC-TS | X(26) |  |
| 361 | 140 | &nbsp;&nbsp;FILLER | X(140) |  |
| 41 | 460 | EXPORT-CARD-XREF-DATA | group | REDEFINES EXPORT-RECORD-DATA |
| 41 | 16 | &nbsp;&nbsp;EXP-XREF-CARD-NUM | X(16) |  |
| 57 | 9 | &nbsp;&nbsp;EXP-XREF-CUST-ID | 9(09) |  |
| 66 | 8 | &nbsp;&nbsp;EXP-XREF-ACCT-ID | 9(11) COMP |  |
| 74 | 427 | &nbsp;&nbsp;FILLER | X(427) |  |
| 41 | 460 | EXPORT-CARD-DATA | group | REDEFINES EXPORT-RECORD-DATA |
| 41 | 16 | &nbsp;&nbsp;EXP-CARD-NUM | X(16) |  |
| 57 | 8 | &nbsp;&nbsp;EXP-CARD-ACCT-ID | 9(11) COMP |  |
| 65 | 2 | &nbsp;&nbsp;EXP-CARD-CVV-CD | 9(03) COMP |  |
| 67 | 50 | &nbsp;&nbsp;EXP-CARD-EMBOSSED-NAME | X(50) |  |
| 117 | 10 | &nbsp;&nbsp;EXP-CARD-EXPIRAION-DATE | X(10) |  |
| 127 | 1 | &nbsp;&nbsp;EXP-CARD-ACTIVE-STATUS | X(01) |  |
| 128 | 373 | &nbsp;&nbsp;FILLER | X(373) |  |

### CARDDEMO-COMMAREA — `COCOM01Y.cpy` (160 bytes)

| Offset | Len | Field | Picture | Notes |
|---:|---:|---|---|---|
| 1 | 34 | CDEMO-GENERAL-INFO | group |  |
| 1 | 4 | &nbsp;&nbsp;CDEMO-FROM-TRANID | X(04) |  |
| 5 | 8 | &nbsp;&nbsp;CDEMO-FROM-PROGRAM | X(08) |  |
| 13 | 4 | &nbsp;&nbsp;CDEMO-TO-TRANID | X(04) |  |
| 17 | 8 | &nbsp;&nbsp;CDEMO-TO-PROGRAM | X(08) |  |
| 25 | 8 | &nbsp;&nbsp;CDEMO-USER-ID | X(08) |  |
| 33 | 1 | &nbsp;&nbsp;CDEMO-USER-TYPE | X(01) |  |
| 34 | 1 | &nbsp;&nbsp;CDEMO-PGM-CONTEXT | 9(01) |  |
| 35 | 84 | CDEMO-CUSTOMER-INFO | group |  |
| 35 | 9 | &nbsp;&nbsp;CDEMO-CUST-ID | 9(09) |  |
| 44 | 25 | &nbsp;&nbsp;CDEMO-CUST-FNAME | X(25) |  |
| 69 | 25 | &nbsp;&nbsp;CDEMO-CUST-MNAME | X(25) |  |
| 94 | 25 | &nbsp;&nbsp;CDEMO-CUST-LNAME | X(25) |  |
| 119 | 12 | CDEMO-ACCOUNT-INFO | group |  |
| 119 | 11 | &nbsp;&nbsp;CDEMO-ACCT-ID | 9(11) |  |
| 130 | 1 | &nbsp;&nbsp;CDEMO-ACCT-STATUS | X(01) |  |
| 131 | 16 | CDEMO-CARD-INFO | group |  |
| 131 | 16 | &nbsp;&nbsp;CDEMO-CARD-NUM | 9(16) |  |
| 147 | 14 | CDEMO-MORE-INFO | group |  |
| 147 | 7 | &nbsp;&nbsp;CDEMO-LAST-MAP | X(7) |  |
| 154 | 7 | &nbsp;&nbsp;CDEMO-LAST-MAPSET | X(7) |  |
