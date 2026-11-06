//! CBSTM03A (+ file-access subroutine CBSTM03B) — account statements in text and
//! HTML (job CREASTMT). Requirements FR-BAT-STMT-01..07, KD-04.
//!
//! DD names: TRNXFILE (TRXFL KSDS: card + tran id), XREFFILE, CUSTFILE, ACCTFILE,
//! STMTFILE (80-byte lines), HTMLFILE (100-byte lines).
//!
//! The ALTER / GO TO driven open sequence of the original is expressed as straight
//! code with the same order of effects. The in-memory transaction table keeps the
//! original flat storage layout (51 cards x 10 transactions, no bounds checking), so
//! overflowing it overlays neighbouring entries exactly as on the mainframe (KD-04).

use crate::ctx::{Abend, StepCtx, StepResult};
use cobol::copybooks::{AccountRecord, CardXrefRecord, CustomerRecord, TrnxRecord};
use cobol::edit::edit;
use cobol::{text, Dec};

const CARD_ENTRY: usize = 16 + 10 * (16 + 318); // WS-CARD-TBL entry
const TABLE: usize = 51 * CARD_ENTRY; // WS-TRNX-TABLE
const CNTR: usize = 51 * 2; // WS-TRN-TBL-CNTR (S9(4) COMP)

/// WS-TRNX-TABLE followed by WS-TRN-TBL-CNTR, as contiguous storage.
struct TrnxTable {
    mem: Vec<u8>,
}

impl TrnxTable {
    fn new() -> Self {
        // INITIALIZE WS-TRNX-TABLE WS-TRN-TBL-CNTR
        let mut mem = vec![b' '; TABLE + CNTR];
        mem[TABLE..].fill(0);
        TrnxTable { mem }
    }
    fn slot(&mut self, off: usize, len: usize) -> Result<&mut [u8], Abend> {
        self.mem.get_mut(off..off + len).ok_or(Abend(4038)) // storage beyond the table
    }
    fn card_off(cr: usize) -> usize {
        (cr - 1) * CARD_ENTRY
    }
    fn tran_off(cr: usize, tr: usize) -> usize {
        Self::card_off(cr) + 16 + (tr - 1) * 334
    }
    fn trct_off(cr: usize) -> usize {
        TABLE + (cr - 1) * 2
    }
    fn set_card(&mut self, cr: usize, v: &[u8]) -> Result<(), Abend> {
        self.slot(Self::card_off(cr), 16)?.copy_from_slice(&v[..16]);
        Ok(())
    }
    fn card(&self, cr: usize) -> &[u8] {
        let o = Self::card_off(cr);
        &self.mem[o..o + 16]
    }
    fn set_tran(&mut self, cr: usize, tr: usize, id: &[u8], rest: &[u8]) -> Result<(), Abend> {
        let s = self.slot(Self::tran_off(cr, tr), 334)?;
        s[..16].copy_from_slice(&id[..16]);
        s[16..].copy_from_slice(&rest[..318]);
        Ok(())
    }
    fn tran(&self, cr: usize, tr: usize) -> &[u8] {
        let o = Self::tran_off(cr, tr);
        &self.mem[o..o + 334]
    }
    fn set_trct(&mut self, cr: usize, n: i16) -> Result<(), Abend> {
        self.slot(Self::trct_off(cr), 2)?.copy_from_slice(&n.to_be_bytes());
        Ok(())
    }
    fn trct(&self, cr: usize) -> i16 {
        let o = Self::trct_off(cr);
        i16::from_be_bytes([self.mem[o], self.mem[o + 1]])
    }
}

/// STRING a DELIMITED BY ' ' / SIZE pieces into a fresh area of `len` spaces.
fn st(len: usize, pieces: &[&[u8]]) -> Vec<u8> {
    let mut b = vec![b' '; len];
    text::string_into(&mut b, pieces);
    b
}
fn w(b: &[u8]) -> &[u8] {
    text::until(b, b" ")
}

struct Out {
    stmt: crate::seqfile::SeqOut,
    html: crate::seqfile::SeqOut,
}
impl Out {
    fn s(&mut self, line: &[u8]) -> Result<(), Abend> {
        let mut r = [b' '; 80];
        text::mv(&mut r, line);
        self.stmt.write(&r).map_err(|_| Abend(999))
    }
    fn h(&mut self, line: &[u8]) -> Result<(), Abend> {
        let mut r = [b' '; 100];
        text::mv(&mut r, line);
        self.html.write(&r).map_err(|_| Abend(999))
    }
}

const L08: &str = "<table  align=\"center\" frame=\"box\" style=\"width:70%; font:12px Segoe UI,sans-serif;\">";
const L10: &str = "<td colspan=\"3\" style=\"padding:0px 5px;background-color:#1d1d96b3;\">";
const L15: &str = "<td colspan=\"3\" style=\"padding:0px 5px;background-color:#FFAF33;\">";
const L22_35: &str = "<td colspan=\"3\" style=\"padding:0px 5px;background-color:#f2f2f2;\">";
const L30_42: &str = "<td colspan=\"3\" style=\"padding:0px 5px;background-color:#33FFD1; text-align:center;\">";
const L47: &str = "<td style=\"width:25%; padding:0px 5px; background-color:#33FF5E; text-align:left;\">";
const L50: &str = "<td style=\"width:55%; padding:0px 5px; background-color:#33FF5E; text-align:left;\">";
const L53: &str = "<td style=\"width:20%; padding:0px 5px; background-color:#33FF5E; text-align:right;\">";
const L58: &str = "<td style=\"width:25%; padding:0px 5px; background-color:#f2f2f2; text-align:left;\">";
const L61: &str = "<td style=\"width:55%; padding:0px 5px; background-color:#f2f2f2; text-align:left;\">";
const L64: &str = "<td style=\"width:20%; padding:0px 5px; background-color:#f2f2f2; text-align:right;\">";

fn err(ctx: &mut StepCtx, what: &str, rc: &str) -> Abend {
    ctx.display(what);
    ctx.display(format!("RETURN CODE: {rc}"));
    ctx.display("ABENDING PROGRAM");
    Abend(999)
}

pub fn run(ctx: &mut StepCtx) -> StepResult {
    // TIOT walk: job / step name and the DD names of this step
    ctx.display("Running JCL : CREASTMT Step STEP040 ");
    ctx.display("DD Names from TIOT: ");
    let mut dds: Vec<String> = ctx.dds.keys().cloned().collect();
    dds.sort();
    for d in dds {
        ctx.display(format!(": {d:<8} -- valid UCB"));
    }
    let mut out = Out { stmt: ctx.output("STMTFILE")?, html: ctx.output("HTMLFILE")? };
    let mut table = TrnxTable::new();

    // 8100-TRNXFILE-OPEN + 8500-READTRNX-READ: load all transactions by card
    let mut trnx_in = ctx.input("TRNXFILE", 350).map_err(|_| err(ctx, "ERROR OPENING TRNXFILE", "35"))?;
    let first = trnx_in.read().ok_or_else(|| err(ctx, "ERROR READING TRNXFILE", "10"))?;
    let mut trnx = TrnxRecord::from_bytes(&first);
    let mut save_card = trnx.trnx_card_num().to_vec();
    let mut cr_cnt: usize = 1;
    let mut tr_cnt: usize = 0;
    loop {
        if save_card == trnx.trnx_card_num() {
            tr_cnt += 1;
        } else {
            table.set_trct(cr_cnt, tr_cnt as i16)?;
            cr_cnt += 1;
            tr_cnt = 1;
        }
        table.set_card(cr_cnt, trnx.trnx_card_num())?;
        table.set_tran(cr_cnt, tr_cnt, trnx.trnx_id(), trnx.trnx_rest())?;
        save_card = trnx.trnx_card_num().to_vec();
        match trnx_in.read() {
            Some(r) => trnx = TrnxRecord::from_bytes(&r),
            None => break,
        }
    }
    table.set_trct(cr_cnt, tr_cnt as i16)?;

    // 8200/8300/8400 opens, then 1000-MAINLINE
    let xref_ds = ctx.vsam_name("XREFFILE").map_err(|_| err(ctx, "ERROR OPENING XREFFILE", "35"))?;
    let cust_ds = ctx.vsam_name("CUSTFILE").map_err(|_| err(ctx, "ERROR OPENING CUSTFILE", "35"))?;
    let acct_ds = ctx.vsam_name("ACCTFILE").map_err(|_| err(ctx, "ERROR OPENING ACCTFILE", "35"))?;
    let xrefs = ctx.vsam.unload(&xref_ds).map_err(|_| err(ctx, "ERROR READING XREFFILE", "30"))?;

    for xr in xrefs {
        let xref = CardXrefRecord::from_bytes(&xr);
        // 2000-CUSTFILE-GET / 3000-ACCTFILE-GET
        let cust = match ctx.vsam.read(&cust_ds, xref.xref_cust_id()) {
            Ok(r) => CustomerRecord::from_bytes(&r),
            Err(e) => return Err(err(ctx, "ERROR READING CUSTFILE", e.file_status())),
        };
        let acct = match ctx.vsam.read(&acct_ds, xref.xref_acct_id()) {
            Ok(r) => AccountRecord::from_bytes(&r),
            Err(e) => return Err(err(ctx, "ERROR READING ACCTFILE", e.file_status())),
        };

        // 5000-CREATE-STATEMENT
        out.s(&[b'*'; 31].iter().chain(b"START OF STATEMENT").chain([b'*'; 31].iter()).copied().collect::<Vec<u8>>())?;
        // 5100-WRITE-HTML-HEADER
        for l in ["<!DOCTYPE html>", "<html lang=\"en\">", "<head>", "<meta charset=\"utf-8\">", "<title>HTML Table Layout</title>", "</head>", "<body style=\"margin:0px;\">", L08, "<tr>", L10] {
            out.h(l.as_bytes())?;
        }
        let mut l11 = b"<h3>Statement for Account Number: ".to_vec();
        let mut acct20 = [b' '; 20];
        text::mv(&mut acct20, acct.acct_id());
        l11.extend_from_slice(&acct20);
        l11.extend_from_slice(b"</h3>");
        out.h(&l11)?;
        for l in ["</td>", "</tr>", "<tr>", L15, "<p style=\"font-size:16px\">Bank of XYZ</p>", "<p>410 Terry Ave N</p>", "<p>Seattle WA 99999</p>", "</td>", "</tr>", "<tr>", L22_35] {
            out.h(l.as_bytes())?;
        }
        let st_name = st(75, &[w(cust.cust_first_name()), b" ", w(cust.cust_middle_name()), b" ", w(cust.cust_last_name()), b" "]);
        let st_add1 = st(50, &[cust.cust_addr_line_1()]);
        let st_add2 = st(50, &[cust.cust_addr_line_2()]);
        let st_add3 = st(80, &[
            w(cust.cust_addr_line_3()), b" ", w(cust.cust_addr_state_cd()), b" ",
            w(cust.cust_addr_country_cd()), b" ", w(cust.cust_addr_zip()), b" ",
        ]);
        let st_acct_id = acct20;
        let st_curr_bal = edit(acct.acct_curr_bal_n(), "9(9).99-");
        let mut st_fico = [b' '; 20];
        text::mv(&mut st_fico, cust.cust_fico_credit_score());

        // 5200-WRITE-HTML-NMADBS
        let mut l23 = [b' '; 50];
        text::mv(&mut l23, &st_name);
        // DELIMITED BY '  ' (two spaces), then the literal '  '
        out.h(&st(100, &[b"<p style=\"font-size:16px\">", text::until(&l23, b"  "), b"  ", b"</p>"]))?;
        for a in [&st_add1[..], &st_add2[..], &st_add3[..]] {
            out.h(&st(100, &[b"<p>", text::until(a, b"  "), b"  ", b"</p>"]))?;
        }
        for l in ["</td>", "</tr>", "<tr>", L30_42, "<p style=\"font-size:16px\">Basic Details</p>", "</td>", "</tr>", "<tr>", L22_35] {
            out.h(l.as_bytes())?;
        }
        out.h(&st(100, &[b"<p>Account ID         : ", &st_acct_id, b"</p>"]))?;
        out.h(&st(100, &[b"<p>Current Balance    : ", st_curr_bal.as_bytes(), b"</p>"]))?;
        out.h(&st(100, &[b"<p>FICO Score         : ", &st_fico, b"</p>"]))?;
        for l in [
            "</td>", "</tr>", "<tr>", L30_42, "<p style=\"font-size:16px\">Transaction Summary</p>", "</td>", "</tr>", "<tr>", L47,
            "<p style=\"font-size:16px\">Tran ID</p>", "</td>", L50, "<p style=\"font-size:16px\">Tran Details</p>", "</td>", L53,
            "<p style=\"font-size:16px\">Amount</p>", "</td>", "</tr>",
        ] {
            out.h(l.as_bytes())?;
        }
        let dashes = [b'-'; 80];
        out.s(&st_name[..75])?;
        out.s(&st_add1)?;
        out.s(&st_add2)?;
        out.s(&st_add3)?;
        out.s(&dashes)?;
        out.s(&st(80, &[&[b' '; 33], b"Basic Details "]))?;
        out.s(&dashes)?;
        out.s(&st(80, &[b"Account ID         :", &st_acct_id]))?;
        out.s(&st(80, &[b"Current Balance    :", st_curr_bal.as_bytes()]))?;
        out.s(&st(80, &[b"FICO Score         :", &st_fico]))?;
        out.s(&dashes)?;
        out.s(&st(80, &[&[b' '; 30], b"TRANSACTION SUMMARY "]))?;
        out.s(&dashes)?;
        out.s(&st(80, &[b"Tran ID         ", b"Tran Details    ", &[b' '; 35], b"  Tran Amount"]))?;
        out.s(&dashes)?;

        // 4000-TRNXFILE-GET
        let mut total = Dec::ZERO;
        let mut cr = 1;
        while !(cr > cr_cnt || table.card(cr) > xref.xref_card_num()) {
            if xref.xref_card_num() == table.card(cr) {
                let n = table.trct(cr).max(0) as usize;
                for tr in 1..=n {
                    let t = table.tran(cr, tr).to_vec();
                    let mut rec = TrnxRecord::new();
                    rec.set_trnx_card_num(table.card(cr));
                    rec.set_trnx_id(&t[..16]);
                    rec.set_trnx_rest(&t[16..]);
                    // 6000-WRITE-TRANS
                    let mut tranid = [b' '; 16];
                    text::mv(&mut tranid, rec.trnx_id());
                    let mut trandt = [b' '; 49];
                    text::mv(&mut trandt, rec.trnx_desc());
                    let amt = edit(rec.trnx_amt_n(), "Z(9).99-");
                    out.s(&st(80, &[&tranid, b" ", &trandt, b"$", amt.as_bytes()]))?;
                    out.h(b"<tr>")?;
                    out.h(L58.as_bytes())?;
                    out.h(&st(100, &[b"<p>", &tranid, b"</p>"]))?;
                    out.h(b"</td>")?;
                    out.h(L61.as_bytes())?;
                    out.h(&st(100, &[b"<p>", &trandt, b"</p>"]))?;
                    out.h(b"</td>")?;
                    out.h(L64.as_bytes())?;
                    out.h(&st(100, &[b"<p>", amt.as_bytes(), b"</p>"]))?;
                    out.h(b"</td>")?;
                    out.h(b"</tr>")?;
                    total = cobol::numeric::fit(total + rec.trnx_amt_n(), cobol::Pic::comp3(11, 2, true));
                }
            }
            cr += 1;
        }
        out.s(&dashes)?;
        out.s(&st(80, &[b"Total EXP:", &[b' '; 56], b"$", edit(total, "Z(9).99-").as_bytes()]))?;
        out.s(&[b'*'; 32].iter().chain(b"END OF STATEMENT").chain([b'*'; 32].iter()).copied().collect::<Vec<u8>>())?;
        for l in ["<tr>", L10, "<h3>End of Statement</h3>", "</td>", "</tr>", "</table>", "</body>", "</html>"] {
            out.h(l.as_bytes())?;
        }
    }
    out.stmt.close().map_err(|_| Abend(999))?;
    out.html.close().map_err(|_| Abend(999))?;
    Ok(0)
}
