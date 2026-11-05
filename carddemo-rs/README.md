# carddemo-rs

The AWS CardDemo mainframe application (COBOL / CICS / VSAM / JCL), translated to Rust.

| Crate | Replaces |
|---|---|
| `cobol` | COBOL data semantics: PIC numerics (zoned, COMP-3, COMP), edited pictures, MOVE/STRING/INSPECT, NUMVAL, cp037. `copybooks.rs` is generated from `app/cpy` (`tools/regen-records.sh`). |
| `vsam` | VSAM KSDS + alternate indexes on SQLite, with CICS browse semantics and EBCDIC key order. |
| `cics` | Pseudo-CICS: BMS maps parsed from `app/bms`, pseudo-conversational tasks, XCTL/RETURN, file control, TD queue. |
| `online` | The 17 CICS programs (COSGN00C ... COACTUPC), one function per COBOL paragraph. |
| `batch` | The batch programs (CBTRN02C, CBACT04C, CBTRN03C, CBSTM03A/B, CBEXPORT/CBIMPORT, ...) and the JCL jobs (`jobs.rs`). |
| `carddemo` | The binary: terminal UI (`online`), `job`, `batch`, `reset`, `script`. |

```sh
cargo build --release
target/release/carddemo reset && target/release/carddemo online
cargo test --workspace
```

See `../docs/DEMO.md` for the demo script, `../docs/requirements.pdf` for the specification, and `../docs/progress-log.md` for the project log.
