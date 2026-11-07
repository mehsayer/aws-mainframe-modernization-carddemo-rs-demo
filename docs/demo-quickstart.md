---
title: "CardDemo: the demo to download"
subtitle: "Quick start for Windows and Linux"
author: "Illia Burov"
geometry: margin=2.2cm
---

# What this is

*CardDemo* is a sample credit-card system that Amazon Web Services wrote for an IBM mainframe in COBOL:
night-time **batch jobs** (posting the day's card transactions, interest, customer statements, a
transaction report) and **online screens** for staff (sign-on, accounts, cards, bill payment). The whole
system was translated into the modern programming language Rust with Claude Code; the translation shows
the same screens and produces the same files as the original.

This download is that translation as **one program** with the sample data. It needs **no installation, no
mainframe and no COBOL**: unpack it and start it. It runs on **Windows** (10 or 11, 64-bit) and **Linux**
(64-bit, glibc 2.34 or newer: Ubuntu 22.04, Debian 12, Fedora 35, RHEL 9 or later).

Download it from the project's GitHub page, section **Releases**:
<https://github.com/mehsayer/aws-mainframe-modernization-carddemo-rs-demo/releases>

| File | For |
|---|---|
| `carddemo-<version>-windows-x86_64.zip` | Windows |
| `carddemo-<version>-linux-x86_64.tar.gz` | Linux |

# Windows: start it with a double click

1. **Unpack the zip file**: right-click it in Explorer, *Extract All...*, *Extract*. (Starting the program
   from inside the zip view does not work: Windows unpacks only the program, not the sample data next
   to it.)
2. Open the unpacked folder `carddemo-<version>-windows-x86_64` and **double-click `carddemo.exe`**.
3. The first time, Windows may say *"Windows protected your PC"*, because the program is not signed by a
   publisher. Click **More info**, then **Run anyway**.
4. A console window opens with the **demo menu**. Type the number of an option and press **Enter**.

The program keeps its data in the folder `carddemo-data` next to `carddemo.exe`. To start again from
the original sample data, use option 5 or delete that folder.

# Linux: start it in a terminal

```sh
tar xzf carddemo-<version>-linux-x86_64.tar.gz
cd carddemo-<version>-linux-x86_64
./carddemo
```

The demo menu appears in the terminal. Make the window at least 80 columns by 26 lines (the online
screens are 80 by 24 characters, like a mainframe terminal).

# The demo menu

| Option | What it does | What to look at |
|---|---|---|
| **1** Online system | The staff screens, as on a 3270 mainframe terminal | Sign on, look up an account, pay a bill (below) |
| **2** Nightly batch | Runs the night's jobs: backup, posting, interest, statements | Each step with its return code; `MAXCC=0000` means success |
| **3** Transaction report | The transaction report for a date range (default: 2022-01-01 to today; run option 2 first) | Then option 4, file `TRANREPT`: transactions per card and account, with totals |
| **4** Output files | A viewer for the files the jobs wrote | Customer statements (text, or HTML in the web browser), rejected transactions |
| **5** Reset | Back to the shipped sample data; removes earlier output files | |
| **6** Original COBOL screens | Linux only, and only if GnuCOBOL is installed (below) | The same screens, run by the original COBOL programs |
| **0** Quit | | |

## A short walk through the online screens (option 1)

Keys: **Enter** sends the screen, **F3** goes back, **F7/F8** page up and down, **F5** saves,
**Tab** moves to the next field, **Esc** clears the screen, **Ctrl+Q** returns to the demo menu.
A field of full length moves on to the next field by itself, as on a mainframe terminal.

1. User `USER0001`, password `PASSWORD`, **Enter**: the **Main Menu**.
   (`ADMIN001` / `PASSWORD` signs on as administrator, with the user-management menu.)
2. Type `1` and **Enter**: **Account View**. Account `00000000001`, **Enter**: balance, credit limits,
   the customer's name and address.
3. **F3** back to the menu. `3` and **Enter**: the **Card List**; **F8** shows the next page. Type `S` next
   to a card and press **Enter** for its details.
4. **F3** twice, then `10` and **Enter**: **Bill Payment**. Account `00000000001`, **Enter** shows the
   balance; type `Y` and **Enter** to pay it. The screen shows the new transaction number.
5. **F3** to the menu, **F3** again to sign off, **Ctrl+Q** back to the demo menu.

## The nightly batch and its output (options 2 and 4)

Option 2 prints one line per job step: the job (for example `POSTTRAN`), the step, the program
(`CBTRN02C`, the original COBOL program's name) and its return code. Then open option 4:

- `STATEMNT.PS`: the customer statements as text; `STATEMNT.HTML` the same as web pages (the viewer
  offers to open it in the browser).
- `DALYREJS`: the daily transactions that posting rejected, with the reason code and text at the end of
  each record (for example `0102OVERLIMIT TRANSACTION`).
- `TRANREPT` (after option 3): the transaction report. The nightly batch stamps the transactions it posts
  with the day it runs, so the report's default range ends today.
- The other files are working files of the jobs, shown as fixed-length records: dots stand for bytes that
  are not text (packed numbers).

Viewer keys: arrow keys, **PgUp/PgDn**, **Home/End** to scroll, **Left/Right** for wide records,
**q** or **Esc** to go back.

# Linux: the original COBOL screens side by side (optional)

The Linux download also contains the original COBOL screen programs, compiled with GnuCOBOL 3.2 (folder
`cobol-modules`). With the GnuCOBOL 3.2 runtime library installed (`libcob.so.4`, for example from the
`gnucobol` or `gnucobol3` package of your distribution, or built from source), menu option **6** runs
them on the same kind of stand-in for the mainframe's transaction system (CICS) that the Rust version
uses. Start the program twice, in two terminal windows side by side, and do the same steps with option 1 in
one and option 6 in the other: the screens are the same. If the runtime is missing or of another version,
the menu says why option 6 is not available. (Their output and messages go to
`carddemo-data/cobol-sysout.log`.)

The complete proof, which compiles and runs all original COBOL programs and compares every output file
and screen automatically, needs the full project and the GnuCOBOL compiler; see `docs/DEMO.md` in the
repository.

# If something goes wrong

| What you see | What to do |
|---|---|
| The window closes at once (Windows) | The zip was not unpacked, or `data\EBCDIC` is missing: unpack the whole zip and start `carddemo.exe` from the unpacked folder |
| "The sample data was not found" | The folder `data/EBCDIC` must be next to the program |
| Garbled or cut-off screens | Make the window larger (at least 80 x 26) |
| "Error: ... carddemo-data ..." | The folder is not writable: unpack the download into your own folder (Downloads, Desktop) |
| Linux: `GLIBC_2.34 not found` | The system is older than the minimum (glibc 2.34) |
| The data looks wrong after experiments | Option 5 resets it |

The program, its sources and the sample data are under the Apache License 2.0 (files `LICENSE` and
`NOTICE`); the sample application CardDemo is © Amazon.com, Inc. or its affiliates.
