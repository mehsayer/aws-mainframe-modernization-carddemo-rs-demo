# CardDemo: COBOL mainframe application translated to Rust with Claude Code

| Folder | Contents |
|---|---|
| [`cobol/`](cobol/) | The original [AWS CardDemo](cobol/README.md) mainframe application (COBOL, CICS, VSAM, JCL), unchanged |
| [`rust/`](rust/) | The Rust translation ([README](rust/README.md)): batch jobs, online screens in a terminal, differential test |
| [`demo/`](demo/) | `python3 demo/demo.py all`: run the original COBOL batch and the Rust batch, then compare the results |
| [`docs/`](docs/) | [Demo guide](docs/DEMO.md), requirements, manual test suite, presentation (PDF) |

Quick start (needs Rust, Python 3 and GnuCOBOL):

```sh
python3 demo/demo.py all       # COBOL run, Rust run, comparison
python3 demo/demo.py online    # the online screens (sign on as USER0001 / PASSWORD)
```

Working with Claude Code in this repository: see [`CLAUDE.md`](CLAUDE.md).

The original CardDemo is © Amazon.com, Inc. or its affiliates, Apache License 2.0 (see `LICENSE`, `NOTICE`).
