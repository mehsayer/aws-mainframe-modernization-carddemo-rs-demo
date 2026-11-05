# CardDemo (Rust): Live Demo Script

About 10 minutes. Run everything from the repository root.

## 0. Preparation (once)
```sh
cd carddemo-rs && cargo build --release && cd ..
alias carddemo=$PWD/carddemo-rs/target/release/carddemo
export CARDDEMO_HOME=$PWD/carddemo-data      # SQLite store + data sets
carddemo reset                                # load the shipped EBCDIC sample data
```

## 1. Online: sign-on and menus (3 min)
`carddemo online` opens the 3270-style terminal. The keys are: Enter; F1–F12 = PF1–PF12; Shift+F1–F12 = PF13–PF24; Esc = CLEAR; Tab / Shift+Tab move between fields; Ctrl+E = ERASE EOF; Ctrl+Q = quit.

1. Type user `ADMIN001`, password `PASSWORD`, then Enter. The **Admin Menu** appears.
   Option `1`: user list. Type `U` next to `USER0001`, press Enter, change the first name, press F5, then F3.
2. Press F3 to return to sign-on. Sign on as `USER0001` / `PASSWORD`. The **Main Menu** appears.
3. Option `1` (Account View): enter `00000000001`.
4. Option `3` (Card List): F8 / F7 to page, `S` on a row to view the card, `U` on a row to update it (change the status to `N`, press Enter, then F5).
5. Option `6` (Transaction List): page with F8, `S` on a row to view.
6. Option `10` (Bill Payment): account `00000000001`, Enter shows the balance, then `Y` and Enter to pay.
   The new transaction appears at the end of the transaction list.

## 2. Batch: the nightly cycle (3 min)
```sh
carddemo jobs                       # the JCL jobs, by name
carddemo job POSTTRAN               # posting: 300 read, 38 rejected, RC=4
carddemo job INTCALC                # interest calculation
carddemo batch                      # TRANBKP POSTTRAN INTCALC TRANBKP COMBTRAN CREASTMT
less $CARDDEMO_HOME/datasets/AWS.M2.CARDDEMO.STATEMNT.PS     # text statements
xdg-open $CARDDEMO_HOME/datasets/AWS.M2.CARDDEMO.STATEMNT.HTML
```

## 3. Report submitted from the screen (2 min)
In `carddemo online`, sign on as a user, choose option `9`, put `X` in *Monthly* or set *Custom* dates, type `Y` to confirm, and press Enter.
The JCL deck written to the TD queue `JOBS` is run by the built-in internal reader. The status line shows where the report was written
(`datasets/AWS.M2.CARDDEMO.TRANREPT.G000nV00`).

## 4. Fidelity talking points (2 min)
- Screens are rendered from the **original BMS sources** (`app/bms`). Record layouts are generated from the **original copybooks**.
- Data is loaded byte-exact from the original **EBCDIC** files (zoned decimal and COMP-3), and keys are ordered in EBCDIC collating sequence.
- Defects in the original are kept on purpose (requirements chapter *Known defects*), for example:
  - INTCALC never writes back the last account (KD-01);
  - a card update resets the CVV (KD-07);
  - the COACTVW map blanks most of the `Tran:` header with an unnamed field.
- `cargo test --workspace` runs the scripted terminal journeys and the full batch cycle.

## Reset between runs
`carddemo reset` (about 5 ms).
