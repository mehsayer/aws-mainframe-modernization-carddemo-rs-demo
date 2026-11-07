# carddemo-rs

Rust translation of the AWS CardDemo mainframe application (`../cobol/`), by Illia Burov with Claude Code.

> **Modification notice (Apache License 2.0, section 4b):** this directory is a derived work of AWS CardDemo,
> © Amazon.com, Inc. or its affiliates. The COBOL programs, copybooks, BMS maps and JCL were translated into
> Rust; `crates/cobol/src/copybooks.rs` and `crates/online/src/lookups.rs` are generated from the original
> files, and the BMS map definitions are read from `../cobol/app/bms` at build time. Licensed under the Apache License 2.0 (`../LICENSE`, `../NOTICE`).

The AWS CardDemo mainframe application (COBOL / CICS / VSAM / JCL), translated to Rust.

| Crate | Replaces |
|---|---|
| `cobol` | COBOL data semantics: PIC numerics (zoned, COMP-3, COMP), edited pictures, MOVE/STRING/INSPECT, NUMVAL, cp037. `copybooks.rs` is generated from `cobol/app/cpy` (`tools/regen-records.sh`). |
| `vsam` | VSAM KSDS + alternate indexes on SQLite, with CICS browse semantics and EBCDIC key order. |
| `cics` | Pseudo-CICS: BMS maps parsed from `cobol/app/bms`, pseudo-conversational tasks, XCTL/RETURN, file control, TD queue. |
| `online` | The 17 CICS programs (COSGN00C ... COACTUPC), one function per COBOL paragraph. |
| `batch` | The batch programs (CBTRN02C, CBACT04C, CBTRN03C, CBSTM03A/B, CBEXPORT/CBIMPORT, ...) and the JCL jobs (`jobs.rs`). |
| `carddemo` | The binary: terminal UI (`online`), `job`, `batch`, `reset`, `script`. |

```sh
cargo build --release
target/release/carddemo reset && target/release/carddemo online
cargo test --workspace
```

See `../docs/DEMO.md` for the demo script, `../docs/requirements.pdf` for the specification, and `../docs/progress-log.md` for the project log.

## Differential test against the original COBOL

`difftest/run.py` compiles the original batch programs (`cobol/app/cbl`) with GnuCOBOL 3.x and runs them on
the same input data as the Rust port, then compares every output data set, report, return code and
SYSOUT line:

```sh
cargo build --release
python3 difftest/run.py          # report: difftest/out/report.md (copy in docs/difftest/report.md)
```

Covered: CBACT01C (with COBDATFT translated from the assembler), CBACT02C, CBACT03C, CBCUS01C,
CBTRN01C, CBTRN02C (POSTTRAN), CBACT04C (INTCALC), CBTRN03C (TRANREPT), CBSTM03A/B (CREASTMT), and
CBEXPORT/CBIMPORT (FD patched for KD-12). The harness generates small COBOL load/unload programs for the
indexed files, a C stub for CEE3ABD, and a PARM wrapper for CBACT04C; CBSTM03A's z/OS control-block walk
(PSA/TCB/TIOT) is commented out. Platform differences that are normalised are listed in the script.

## The original online programs on the CICS bridge

`cobridge/cicsprep.py` translates the original CICS COBOL programs (`EXEC CICS` becomes a call through a
procedure pointer in the EIB) and compiles them with GnuCOBOL; `crates/cobridge` runs them on the same CICS
runtime, screens and data store as the Rust programs (libcob, loaded at run time; building needs no
GnuCOBOL). The project's own `DFHAID`,
`DFHBMSCA`, `DFHATTR` and `DFHEIBLK` copybooks (`cobridge/copy/`) and a CEEDAYS (`cobridge/ceedays.c`) stand in
for the IBM-supplied parts.

```sh
python3 cobridge/cicsprep.py                    # 17 programs + CSUTLDTC + CEEDAYS -> cobridge/build/modules
target/release/carddemo online --cobol           # the original screens in the terminal UI
python3 difftest/online.py                      # all online tests on both; every screen compared
```

