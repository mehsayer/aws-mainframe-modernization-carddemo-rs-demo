# Differential test: COBOL (GnuCOBOL) vs Rust

Run date 2022-07-01T12:00:00 (both sides), INTCALC PARM 2022071800, TRANREPT 2022-01-01..2022-07-06.

| Job / program | Output | Rust / COBOL | Result |
|---|---|---|---|
| READACCT / CBACT01C | return code | 0 / 0 | identical |
| READACCT / CBACT01C | OUTFILE (with COMP-3) | 50 / 50 records | identical |
| READACCT / CBACT01C | ARRYFILE (OCCURS, COMP-3) | 50 / 50 records | identical (note 1) |
| READACCT / CBACT01C | VBRCFILE (variable records) | identical bytes / identical bytes | identical |
| READACCT / CBACT01C | SYSOUT | 752 / 752 lines | identical |
| READCARD / CBACT02C | return code | 0 / 0 | identical |
| READCARD / CBACT02C | SYSOUT | 52 / 52 lines | identical |
| READXREF / CBACT03C | return code | 0 / 0 | identical |
| READXREF / CBACT03C | SYSOUT | 102 / 102 lines | identical |
| READCUST / CBCUS01C | return code | 0 / 0 | identical |
| READCUST / CBCUS01C | SYSOUT | 102 / 102 lines | identical |
| DALYVRFY / CBTRN01C | return code | 0 / 0 | identical |
| DALYVRFY / CBTRN01C | SYSOUT | 1807 / 1807 lines | identical |
| POSTTRAN / CBTRN02C | return code | 4 / 4 | identical |
| POSTTRAN / CBTRN02C | TRANSACT | 262 / 262 records | identical |
| POSTTRAN / CBTRN02C | ACCTDATA | 50 / 50 records | identical |
| POSTTRAN / CBTRN02C | TCATBALF | 100 / 100 records | identical |
| POSTTRAN / CBTRN02C | DALYREJS | 38 / 38 records | identical |
| POSTTRAN / CBTRN02C | SYSOUT | 54 / 54 lines | identical |
| INTCALC / CBACT04C | return code | 0 / 0 | identical |
| INTCALC / CBACT04C | ACCTDATA | 50 / 50 records | identical |
| INTCALC / CBACT04C | SYSTRAN (interest transactions) | 50 / 50 records | identical |
| INTCALC / CBACT04C | SYSOUT | 302 / 302 lines | identical |
| TRANREPT / CBTRN03C | return code | 0 / 0 | identical |
| TRANREPT / CBTRN03C | TRANREPT report | 519 / 519 lines | identical |
| TRANREPT / CBTRN03C | SYSOUT | 317 / 317 lines | identical |
| CREASTMT / CBSTM03A+B | return code | 0 / 0 | identical |
| CREASTMT / CBSTM03A+B | STATEMNT.PS (text) | 1262 / 1262 lines | identical |
| CREASTMT / CBSTM03A+B | STATEMNT.HTML | 6632 / 6632 lines | identical |
| CBEXPORT (patched FD, KD-12) | return code | 0 / 0 | identical |
| CBEXPORT (patched FD, KD-12) | EXPFILE | 512 / 512 records | identical |
| CBEXPORT (patched FD, KD-12) | SYSOUT | 20 / 20 lines | identical |
| CBIMPORT (patched FD, KD-12) | return code | 0 / 0 | identical |
| CBIMPORT (patched FD, KD-12) | CUSTOUT | 50 / 50 records | identical |
| CBIMPORT (patched FD, KD-12) | ACCTOUT | 50 / 50 records | identical |
| CBIMPORT (patched FD, KD-12) | XREFOUT | 50 / 50 records | identical |
| CBIMPORT (patched FD, KD-12) | TRNXOUT | 312 / 312 records | identical |
| CBIMPORT (patched FD, KD-12) | CARDOUT | 50 / 50 records | identical |
| CBIMPORT (patched FD, KD-12) | ERROUT | 1 / 1 records | identical |
| CBIMPORT (patched FD, KD-12) | SYSOUT | 14 / 14 lines | identical |

Note 1: READACCT / CBACT01C, ARRYFILE (OCCURS, COMP-3): GnuCOBOL INITIALIZE leaves signed DISPLAY zeros unsigned ('0'); IBM INITIALIZE acts as MOVE ZERO and sets the positive sign ('{' = X'C0'), as the Rust port does. Bytes 80 and 99 normalised.

**Overall: all outputs identical**

