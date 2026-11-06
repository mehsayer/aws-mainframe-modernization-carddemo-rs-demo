# Plan: running the original COBOL online programs (CICS bridge for GnuCOBOL)

Status: **proposed, awaiting approval** (2026-11-06).

## Goal

Run the 17 original CICS COBOL programs of CardDemo (`cobol/app/cbl/CO*.cbl`) unchanged, in the same
terminal UI and on the same data as the Rust translation, so that:

1. the online part can be demonstrated **old and new side by side**, like the batch part;
2. an **online differential test** can replay the same keystrokes against both and compare every screen
   and every data change.

## Why a bridge, and not a mainframe emulator

IBM CICS is not available outside a licensed mainframe environment, so some stand-in for CICS is needed.
An emulated mainframe (Hercules) does not help: the only operating system it may run for free is MVS 3.8j
(1981). On MVS 3.8j there is no CICS (only the hobbyist look-alike KICKS) and the only COBOL compiler is a
1970s one (ANSI COBOL 74). CardDemo is modern Enterprise COBOL (EVALUATE, END-IF, inline PERFORM,
intrinsic functions, ...) and would have to be rewritten to compile there, which defeats the purpose.
Current z/OS with CICS on a PC needs IBM's licensed ZD&T product.

The project already contains a CICS layer: the Rust runtime (`rust/crates/cics`) that runs the translated
programs (screens from the original BMS files, VSAM files, XCTL/RETURN, TD queue). The bridge lets the
**original COBOL programs use that same layer**.

## How it works

```text
 cobol/app/cbl/COSGN00C.cbl ──► cicsprep (CICS translator) ──► COSGN00C.cob ──► cobc -m ──► COSGN00C.so
                                  EXEC CICS ... END-EXEC                           (GnuCOBOL module)
                                  becomes CALL 'CDCICS' USING ...
 carddemo online --cobol
   Region (rust/crates/cics) ── program COSGN00C? ──► cobridge: call COSGN00C.so via libcob
        ▲                                               │ (DFHEIBLK, DFHCOMMAREA)
        │                                               ▼
        └──────────── CDCICS(command, arguments) ◄── COBOL program
                      SEND MAP / RECEIVE MAP / READ / STARTBR / XCTL / RETURN / ...
                      executed by the same Task methods the Rust programs use
```

### Components

1. **`cicsprep`, the CICS translator** (Python, `rust/cobridge/cicsprep.py`). It does what IBM's CICS
   translator does, for the subset CardDemo uses (21 commands, listed below):
   - replaces each `EXEC CICS ... END-EXEC` with `CALL 'CDCICS'` and a parameter block (command code,
     option flags, lengths) plus the data areas by reference;
   - sets `RESP`/`RESP2` from the EIB after the call; replaces `DFHRESP(name)` with its numeric value;
   - adds `GOBACK` after `XCTL`, `RETURN` and `ABEND`, because control never comes back from them;
   - adds `DFHEIBLK` to the LINKAGE SECTION and `USING DFHEIBLK DFHCOMMAREA` to the PROCEDURE DIVISION.

   The source files themselves are not changed; the translator writes its output to a build directory.
2. **Own versions of the IBM copybooks** `DFHAID`, `DFHBMSCA`, `DFHATTR` and `DFHEIBLK`, written from the
   values IBM documents (the IBM files are not in the repository and are not copied).
3. **Symbolic maps.** `SEND MAP ... FROM(xxxO)` and `RECEIVE MAP ... INTO(xxxI)` pass the BMS symbolic-map
   buffers of `cobol/app/cpy-bms`. The bridge derives each field's offset (length, attribute and colour
   bytes, data) from the BMS source with the existing parser, checks them against the `cpy-bms` copybooks
   at build time, and converts the buffer to and from the screen model.
4. **`cobridge`, a Rust crate** linking `libcob`:
   - loads the COBOL modules and calls them with the EIB and the COMMAREA;
   - exports the `CDCICS` entry point and maps each command onto the existing `cics::Task`
     (file control, browse, screens, XCTL/RETURN, SYNCPOINT, WRITEQ TD, ASKTIME/FORMATTIME, ASSIGN);
   - lets the Region run each program either as Rust or as COBOL;
   - turns a COBOL runtime error into a transaction abend instead of ending the process.
5. **CEEDAYS** (Language Environment date service, called by the original `CSUTLDTC`): implemented in Rust;
   `CSUTLDTC` itself is compiled from the original source.
6. **Front ends**: `carddemo online --cobol`, `python3 demo/demo.py online-cobol`, and the demo guide's
   side-by-side instructions (two windows, two separate data stores).
7. **Online differential test** (`rust/difftest/online.py` or a Rust test): the 41 scripted manual cases run
   against both regions; after every key the screens and the data store are compared. The manual-suite
   assertions are also run against the COBOL region.

### CICS commands to support (from the 17 programs)

SEND MAP / SEND TEXT / SEND (FROM), RECEIVE MAP, RETURN (TRANSID, COMMAREA, LENGTH), XCTL,
READ (UPDATE), WRITE, REWRITE, DELETE, STARTBR (GTEQ), READNEXT, READPREV, ENDBR, SYNCPOINT (ROLLBACK),
WRITEQ TD, ABEND, HANDLE ABEND / HANDLE CONDITION, INQUIRE PROGRAM, ASKTIME, FORMATTIME, ASSIGN.

## Phases and checkpoints

| # | Step | Done when |
|---|---|---|
| 0 | Spike (done) | All 17 programs compile under GnuCOBOL with CICS statements stubbed out: **17/17** |
| 1 | Translator + copybooks | All 17 translate and compile with real `CALL 'CDCICS'` statements |
| 2 | Bridge core: screens, RETURN/XCTL, READ | Sign-on and menus work in `carddemo online --cobol` |
| 3 | All commands; CEEDAYS | Every screen of the manual suite reachable in the COBOL version |
| 4 | Online differential test | Same keystrokes → screens and data compared; differences fixed or documented |
| 5 | Demo, guide, presentation | `demo.py online-cobol`, side-by-side instructions, results in the deck |

Estimated effort: one to two working sessions like the previous ones. Each phase ends with a commit.

## Risks and limits

- **CICS is emulated in both runs.** The comparison checks that the translated business logic behaves like
  the original programs; it does not check our CICS layer against IBM's. The presentation says so.
- **GnuCOBOL vs. Enterprise COBOL.** The batch comparison found two known differences (DISPLAY of signed
  numbers, INITIALIZE of signed zeros). Others may appear on screens; they will be listed and normalised
  only where the cause is proven to be the compiler.
- **libcob is not thread-safe**: the COBOL region runs on one thread; its tests run serially.
- **HANDLE ABEND** is only partly meaningful outside CICS; it is used for unexpected program checks.
- **Data encoding**: as in the batch comparison, data is ASCII with EBCDIC sign conventions
  (`-fsign=EBCDIC`); the attribute values in our DFHBMSCA follow the same convention.
