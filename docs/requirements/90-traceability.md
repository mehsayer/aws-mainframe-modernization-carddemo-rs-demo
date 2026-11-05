# Traceability and Glossary

## Program → requirement traceability

| Source program / artefact | Requirement groups | Count |
|---|---|---:|
| COSGN00C | `FR-SGN-01` … `FR-SGN-07` | 7 |
| COMEN01C / COMEN02Y | `FR-MEN-01` … `FR-MEN-07` | 7 |
| COADM01C / COADM02Y | `FR-ADM-01` … `FR-ADM-04` | 4 |
| COUSR00C | `FR-USR-LST-01` … `FR-USR-LST-07` | 7 |
| COUSR01C | `FR-USR-ADD-01` … `FR-USR-ADD-04` | 4 |
| COUSR02C | `FR-USR-UPD-01` … `FR-USR-UPD-04` | 4 |
| COUSR03C | `FR-USR-DEL-01` … `FR-USR-DEL-03` | 3 |
| COACTVWC | `FR-ACV-01` … `FR-ACV-05` | 5 |
| COCRDSLC | `FR-CDV-01` … `FR-CDV-06` | 6 |
| COCRDLIC | `FR-CDL-01` … `FR-CDL-09` | 9 |
| COCRDUPC | `FR-CDU-01` … `FR-CDU-09` | 9 |
| COACTUPC (+CSUTLDPY, CSLKPCDY) | `FR-ACU-01` … `FR-ACU-11` | 20 |
| COTRN00C | `FR-TRL-01` … `FR-TRL-05` | 5 |
| COTRN01C | `FR-TRV-01` … `FR-TRV-04` | 4 |
| COTRN02C (+CSUTLDTC) | `FR-TRA-01` … `FR-TRA-10` | 10 |
| COBIL00C | `FR-BIL-01` … `FR-BIL-08` | 8 |
| CORPT00C | `FR-RPT-01` … `FR-RPT-07` | 7 |
| CBTRN02C / POSTTRAN | `FR-BAT-POST-01` … `FR-BAT-POST-11` | 11 |
| CBACT04C / INTCALC | `FR-BAT-INT-01` … `FR-BAT-INT-07` | 7 |
| CBTRN03C / TRANREPT | `FR-BAT-REPT-01` … `FR-BAT-REPT-08` | 8 |
| CBSTM03A, CBSTM03B / CREASTMT | `FR-BAT-STMT-01` … `FR-BAT-STMT-07` | 7 |
| CBACT01C-03C, CBCUS01C, COBDATFT / READxxxx | `FR-BAT-RDR-01` … `FR-BAT-RDR-03` | 3 |
| CBEXPORT, CBIMPORT | `FR-BAT-XPRT-01` … `FR-BAT-XPRT-04` | 4 |
| COBSWAIT+MVSWAIT, CSUTLDTC, IDCAMS/SORT/IEBGENER steps | `FR-BAT-UTIL-01` … `FR-BAT-UTIL-03` | 3 |

Totals: 162 functional requirements, 8 shared rules in tables, 12 non-functional requirements and 7 known-defect entries.

## Glossary

| Term | Meaning |
|---|---|
| AIX | VSAM alternate index (a secondary key path), e.g. CXACAIX = XREF by account id |
| BMS | CICS Basic Mapping Support. A *map* is a 24×80 screen layout; a *mapset* groups maps |
| COMMAREA | Communication area passed between CICS programs and between pseudo-conversational tasks |
| COMP / COMP-3 | Binary / packed-decimal numeric storage |
| KSDS | VSAM key-sequenced data set (indexed file) |
| Pseudo-conversational | Each screen interaction is a separate short task; state is carried in the COMMAREA |
| RESP / RESP2 | CICS response codes (e.g. 13 = NOTFND, 14 = DUPREC, 20 = ENDFILE) |
| XCTL | CICS transfer of control to another program, with no return |
| Zoned decimal | One digit per byte; the sign is overpunched in the last byte (e.g. `{` = +0, `}` = −0) |
