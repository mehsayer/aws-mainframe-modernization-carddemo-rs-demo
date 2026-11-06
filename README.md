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

Rust translation, demo and documentation: **Illia Burov**, with Claude Code.

## License and attribution

- The original CardDemo in `cobol/` is © Amazon.com, Inc. or its affiliates and is distributed unchanged under
  the Apache License 2.0 (`LICENSE`, `NOTICE`); its copyright headers are kept as they are.
- The Rust code in `rust/` is a derived work: a translation of the original programs, copybooks and screen
  definitions, made by Illia Burov with Claude Code. It is distributed under the same Apache License 2.0.
- This project is not affiliated with, sponsored by or endorsed by Amazon or Amazon Web Services.
  "AWS" and "CardDemo" are used only to identify the original sample.
