# CardDemo Live Demo Guide

*Presenter: Illia Burov*

This guide is for presenters and for audiences with only basic technical knowledge.
Every step says what to type, what you will see, and what it means.

## What the demo shows, in one minute

*CardDemo* is a sample credit-card system written for an IBM mainframe in **COBOL**, a programming
language from the 1960s that still runs many banks and insurers. It has two parts:

- **Batch jobs** run at night without anyone watching. They post the day's card transactions,
  calculate interest, print a transaction report and produce customer statements.
- **Online screens** are what staff use during the day: signing on, looking up accounts and cards,
  paying bills.

An AI (Claude Code) translated the whole system into **Rust**, a modern programming language.
In the demo we run **the original COBOL programs and the new Rust programs on the same data**, then
let the computer check that they produced exactly the same results.

| Folder | What is in it |
|---|---|
| `cobol/` | The original system, unchanged (from AWS's public CardDemo sample) |
| `rust/` | The translation produced with Claude Code |
| `demo/` | The demo commands used below |
| `docs/` | Requirements, test suite, this guide, the presentation (PDF) |

## Words you will hear

| Word | Plain meaning |
|---|---|
| Terminal | A window where you type commands instead of clicking |
| Command | One line you type in the terminal, then press **Enter** |
| Program | A piece of software that does one job, e.g. "post today's transactions" |
| Batch job | A program that runs on its own, usually at night, and writes files |
| Report / statement | Output files a person reads: a list of transactions, a customer's monthly statement |
| Compile | Turn source code into a program the computer can run |

## Before the audience arrives (presenter, once)

You need three free tools installed: **GnuCOBOL** (runs COBOL outside a mainframe), **Rust** and
**Python 3**. On this machine they are already installed. On another Linux machine, for example:

```sh
sudo xbps-install -S gnucobol gmp-devel python3 rustup      # Void Linux
sudo apt install gnucobol python3 cargo                      # Debian / Ubuntu
```

Open a terminal, go to the project folder and build everything once (takes a minute or two):

```sh
cd ~/src/aws-mainframe-modernization-carddemo-rs-demo
python3 demo/demo.py all
```

If the last line says **"The Rust programs produced exactly the same results as the original COBOL
programs."**, you are ready. Make the terminal font large (Ctrl and + in most terminals).

## Demo 1: one after the other (5 minutes)

Use a single terminal. Run the old system first, then the new one, then compare.

### Step 1: run the original COBOL system

```sh
python3 demo/demo.py cobol
```

What you will see (shortened):

```text
==> COBOL: loading the sample data (the same files the mainframe version ships with)
==> COBOL: daily posting (program CBTRN02C)
    TRANSACTIONS PROCESSED :000000300
    TRANSACTIONS REJECTED  :000000038
    return code 4
==> COBOL: interest calculation (program CBACT04C)
    50 interest transactions written, return code 0
==> COBOL: transaction report (program CBTRN03C)
==> COBOL: customer statements (programs CBSTM03A + CBSTM03B)
COBOL run finished in 0.1 s. Results in demo/out/cobol/:
    posting-log.txt       54 lines
    report.txt           519 lines
    statements.txt      1262 lines
    statements.html     6632 lines
```

**What to say:** "These are the original 1990s-style mainframe programs, unchanged, running on a laptop.
They processed 300 card transactions, refused 38 that went over the credit limit, added interest
for 50 accounts, and printed a report and 50 customer statements."

### Step 2: run the new Rust system

```sh
python3 demo/demo.py rust
```

You will see the same steps and the same counts (300 processed, 38 rejected), with the results
in `demo/out/rust/`.

**What to say:** "Same input, same jobs, but now every program is the Rust translation.
Nothing from the COBOL side is used."

### Step 3: let the computer compare

```sh
python3 demo/demo.py compare
```

```text
==> Comparing the results of the COBOL run and the Rust run, file by file
    posting-log.txt       54 lines   IDENTICAL
    rejected.txt          38 lines   IDENTICAL
    report.txt           519 lines   IDENTICAL
    statements.txt      1262 lines   IDENTICAL
    statements.html     6632 lines   IDENTICAL
The Rust programs produced exactly the same results as the original COBOL programs.
```

**What to say:** "Over 8,500 lines of output, character for character the same, including the
amounts, rounding, and even three mistakes the original programmers made, which we kept on purpose
so the new system behaves exactly like the old one until the business decides to fix them."

Shortcut: `python3 demo/demo.py all` runs steps 1 to 3 in one go.

## Demo 2: side by side (5 minutes)

This version is more visual. Open **two terminal windows** and place them next to each other
(left half and right half of the screen). In both, go to the project folder first:

```sh
cd ~/src/aws-mainframe-modernization-carddemo-rs-demo
```

| Left window: the old system | Right window: the new system |
|---|---|
| `python3 demo/demo.py cobol` | `python3 demo/demo.py rust` |

Press Enter in the left window, then in the right one. The audience sees both systems doing
the same work, step by step, with the same numbers.

### Look at the results side by side

**The transaction report**, old on the left, new on the right, in one window
(press the space bar to scroll and `q` to quit):

```sh
sdiff -w 270 demo/out/cobol/report.txt demo/out/rust/report.txt | less -S
```

Lines that are the same have nothing between the two halves. A difference would be marked with `|`.

**The customer statements as web pages**, one browser tab each:

```sh
xdg-open demo/out/cobol/statements.html
xdg-open demo/out/rust/statements.html
```

Switch between the two tabs: the statements are the same.

**One customer's statement**, the text version, first 40 lines of each:

```sh
head -40 demo/out/cobol/statements.txt
head -40 demo/out/rust/statements.txt
```

### Look at the code side by side (optional, for a technical audience)

The Rust code keeps the names of the original COBOL paragraphs, so you can follow one in the other.
For example, the step that creates a missing balance record:

| Left window | Right window |
|---|---|
| `less +/2700-A-CREATE cobol/app/cbl/CBTRN02C.cbl` | `less +/2700-A-CREATE rust/crates/batch/src/cbtrn02c.rs` |

## Demo 3: the online screens (5 minutes)

```sh
python3 demo/demo.py online
```

This opens the **new** system's screens in the terminal, looking like the original green-screen
terminal. Keys: **Enter** sends the screen, **F3** goes back, **F7/F8** page up/down, **F5** saves,
**Tab** moves to the next field, **Esc** clears the screen, **Ctrl+Q** quits.

A short walk-through:

1. User `USER0001`, password `PASSWORD`, Enter. You see the **Main Menu**.
2. Type `1` and Enter: **Account View**. Type account `00000000001` and Enter: balance, limits,
   the customer's name and address.
3. F3 back to the menu. Type `3`: the **Card List**. F8 shows the next page. Type `S` next to a card and
   press Enter to see its details.
4. F3 twice, then `10`: **Bill Payment**. Account `00000000001`, Enter shows the balance, type `Y` and
   Enter to pay. The screen shows the new transaction number.
5. F3 to the menu, F3 again to sign off.

**Why there is no COBOL version of this part in the demo:** the original screens need a mainframe
transaction system (IBM CICS), which is not available on a laptop. For the screens we show instead that
the Rust version draws them from the original screen definition files: compare
`less cobol/app/bms/COSGN00.bms` with the sign-on screen. Their correctness is checked by the 41-case
test suite (see `docs/manual-test-suite.pdf`).

## Demo 4: the full proof (2 minutes, optional)

```sh
python3 rust/difftest/run.py --no-build
```

This is the check the engineers use. It compares 13 programs and 40 outputs: files, reports, return
codes and console messages. It ends with **"Overall: all outputs identical"**. The full table is in
`rust/difftest/out/report.md`.

## Talking points

- **Speed.** The translation of about 27,000 lines of COBOL took about 1.6 hours of the AI's working
  time, spread over two sessions, plus a person reviewing the plan and the results.
- **Proof, not promises.** The new system is checked against the old one by running both, not by reading
  code. The checks are repeatable: anyone can run the commands above.
- **The AI found problems in the original.** Twelve defects are documented in the requirements
  (`docs/requirements.pdf`, "Known defects"). Two programs in the original do not even compile.
- **People stay in control.** The AI wrote a plan and waited for approval. It writes down everything it
  decides, and it is told what to review and what never to change. Those instructions live in
  `CLAUDE.md`.

## If something goes wrong

| Problem | What to do |
|---|---|
| `command not found: python3` | Install Python 3, or type `python` instead of `python3` |
| `cobc: not found` while building | GnuCOBOL is not installed; see "Before the audience arrives" |
| `DIFFERENT` in the comparison | Run `python3 demo/demo.py all` again; if it stays, show `diff demo/out/cobol/report.txt demo/out/rust/report.txt` |
| The online screen looks garbled | Make the terminal window bigger (at least 80 columns × 24 lines) and start again |
| You want a clean start | `rm -rf demo/out` and run the commands again |
