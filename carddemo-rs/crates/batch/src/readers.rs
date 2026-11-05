//! File print / verification utilities: CBACT01C (READACCT), CBACT02C (READCARD),
//! CBACT03C (READXREF), CBCUS01C (READCUST), CBTRN01C (daily transaction verify),
//! COBSWAIT + MVSWAIT (WAITSTEP). Requirements FR-BAT-RDR-01..03, FR-BAT-VER, FR-BAT-UTIL-01.

use crate::ctx::{abend, StepCtx, StepResult};
use cobol::copybooks::*;
use cobol::edit::edit;
use cobol::numeric::{encode, Pic};
use cobol::{text, Dec};

fn dump(ctx: &mut StepCtx, pgm: &str, dd: &str, len: usize, twice: bool) -> StepResult {
    ctx.display(format!("START OF EXECUTION OF PROGRAM {pgm}"));
    let mut f = ctx.input(dd, len).map_err(|_| abend(ctx, &format!("ERROR OPENING {dd}"), "35"))?;
    while let Some(r) = f.read() {
        ctx.display(text::s(&r));
        if twice {
            ctx.display(text::s(&r));
        }
    }
    ctx.display(format!("END OF EXECUTION OF PROGRAM {pgm}"));
    Ok(0)
}

/// CBACT02C — print card file
pub fn cbact02c(ctx: &mut StepCtx) -> StepResult {
    dump(ctx, "CBACT02C", "CARDFILE", CardRecord::LEN, false)
}
/// CBACT03C — print cross-reference file (each record is displayed twice)
pub fn cbact03c(ctx: &mut StepCtx) -> StepResult {
    dump(ctx, "CBACT03C", "XREFFILE", CardXrefRecord::LEN, true)
}
/// CBCUS01C — print customer file
pub fn cbcus01c(ctx: &mut StepCtx) -> StepResult {
    dump(ctx, "CBCUS01C", "CUSTFILE", CustomerRecord::LEN, false)
}

/// COBDATFT (assembler date conversion). Returns (output date 20, error message).
pub fn cobdatft(in_type: u8, out_type: u8, input: &[u8]) -> ([u8; 20], &'static str) {
    let mut out = [b' '; 20];
    match in_type {
        b'1' if input[4] != b'-' && out_type != b'2' => {
            out[..4].copy_from_slice(&input[..4]);
            out[4] = b'-';
            out[5..7].copy_from_slice(&input[4..6]);
            out[7] = b'-';
            out[8..10].copy_from_slice(&input[6..8]);
            (out, "")
        }
        b'2' if out_type != b'1' => {
            out[..4].copy_from_slice(&input[..4]);
            out[4..6].copy_from_slice(&input[5..7]);
            out[6..8].copy_from_slice(&input[8..10]);
            (out, "")
        }
        _ => (out, "INVALID INPUT"),
    }
}

/// CBACT01C — print accounts and write the three demo output formats
pub fn cbact01c(ctx: &mut StepCtx) -> StepResult {
    ctx.display("START OF EXECUTION OF PROGRAM CBACT01C");
    let mut f = ctx.input("ACCTFILE", AccountRecord::LEN).map_err(|_| abend(ctx, "ERROR OPENING ACCTFILE", "35"))?;
    let mut out = ctx.output("OUTFILE").map_err(|_| abend(ctx, "ERROR OPENING OUTFILE", "35"))?;
    let mut arr = ctx.output("ARRYFILE").map_err(|_| abend(ctx, "ERROR OPENING ARRYFILE", "35"))?;
    let mut vb = ctx.output("VBRCFILE").map_err(|_| abend(ctx, "ERROR OPENING VBRCFILE", "35"))?;
    let p12 = Pic::display(12, 2, true);
    let c3 = Pic::comp3(12, 2, true);
    // OUT-ACCT-REC (FD record area persists between records: KD-05)
    let mut out_rec = vec![b' '; 11 + 1 + 12 * 3 + 30 + 12 + 7 + 10];
    let enc = |v: Dec, p: Pic| {
        let mut b = vec![0u8; p.len()];
        encode(v, p, &mut b);
        b
    };
    while let Some(r) = f.read() {
        let a = AccountRecord::from_bytes(&r);
        for (l, v) in [
            ("ACCT-ID                 :", a.acct_id()),
            ("ACCT-ACTIVE-STATUS      :", a.acct_active_status()),
            ("ACCT-CURR-BAL           :", a.acct_curr_bal()),
            ("ACCT-CREDIT-LIMIT       :", a.acct_credit_limit()),
            ("ACCT-CASH-CREDIT-LIMIT  :", a.acct_cash_credit_limit()),
            ("ACCT-OPEN-DATE          :", a.acct_open_date()),
            ("ACCT-EXPIRAION-DATE     :", a.acct_expiraion_date()),
            ("ACCT-REISSUE-DATE       :", a.acct_reissue_date()),
            ("ACCT-CURR-CYC-CREDIT    :", a.acct_curr_cyc_credit()),
            ("ACCT-CURR-CYC-DEBIT     :", a.acct_curr_cyc_debit()),
            ("ACCT-GROUP-ID           :", a.acct_group_id()),
        ] {
            ctx.display(format!("{l}{}", text::s(v)));
        }
        ctx.display("-------------------------------------------------");
        // 1300-POPUL-ACCT-RECORD (OUT-ACCT-REC field offsets)
        out_rec[0..11].copy_from_slice(a.acct_id());
        out_rec[11..12].copy_from_slice(a.acct_active_status());
        out_rec[12..24].copy_from_slice(a.acct_curr_bal());
        out_rec[24..36].copy_from_slice(a.acct_credit_limit());
        out_rec[36..48].copy_from_slice(a.acct_cash_credit_limit());
        out_rec[48..58].copy_from_slice(a.acct_open_date());
        out_rec[58..68].copy_from_slice(a.acct_expiraion_date());
        let (reissue, _) = cobdatft(b'2', b'2', a.acct_reissue_date());
        out_rec[68..78].copy_from_slice(&reissue[..10]);
        out_rec[78..90].copy_from_slice(a.acct_curr_cyc_credit());
        if a.acct_curr_cyc_debit_n() == Dec::ZERO {
            out_rec[90..97].copy_from_slice(&enc(Dec::new(252500, 2), c3));
        } // else KD-05: value left from the previous record
        out_rec[97..107].copy_from_slice(a.acct_group_id());
        out.write(&out_rec).map_err(|_| abend(ctx, "ACCOUNT FILE WRITE STATUS IS:30", "30"))?;
        // 1400-POPUL-ARRAY-RECORD
        let mut ar = Vec::new();
        ar.extend_from_slice(a.acct_id());
        let slots = [
            (a.acct_curr_bal_n(), Dec::new(100500, 2)),
            (a.acct_curr_bal_n(), Dec::new(152500, 2)),
            (Dec::new(-102500, 2), Dec::new(-250000, 2)),
            (Dec::ZERO, Dec::ZERO),
            (Dec::ZERO, Dec::ZERO),
        ];
        for (bal, deb) in slots {
            ar.extend(enc(bal, p12));
            ar.extend(enc(deb, c3));
        }
        ar.extend_from_slice(b"    ");
        arr.write(&ar).map_err(|_| abend(ctx, "ACCOUNT FILE WRITE STATUS IS:30", "30"))?;
        // 1500..1575 variable-length records (written with a 2-byte length prefix)
        let mut v1 = a.acct_id().to_vec();
        v1.extend_from_slice(a.acct_active_status());
        let mut v2 = a.acct_id().to_vec();
        v2.extend_from_slice(a.acct_curr_bal());
        v2.extend_from_slice(a.acct_credit_limit());
        v2.extend_from_slice(&a.acct_reissue_date()[..4]);
        ctx.display(format!("VBRC-REC1:{}", text::s(&v1)));
        ctx.display(format!("VBRC-REC2:{}", text::s(&v2)));
        for v in [&v1, &v2] {
            let mut rec = (v.len() as u16).to_be_bytes().to_vec();
            rec.extend_from_slice(v);
            vb.write(&rec).map_err(|_| abend(ctx, "ACCOUNT FILE WRITE STATUS IS:30", "30"))?;
        }
    }
    for o in [out, arr, vb] {
        o.close().map_err(|_| abend(ctx, "ERROR CLOSING FILE", "30"))?;
    }
    ctx.display("END OF EXECUTION OF PROGRAM CBACT01C");
    Ok(0)
}

/// CBTRN01C — verify daily transactions against XREF and accounts (no updates)
pub fn cbtrn01c(ctx: &mut StepCtx) -> StepResult {
    ctx.display("START OF EXECUTION OF PROGRAM CBTRN01C");
    let mut f = ctx.input("DALYTRAN", DalytranRecord::LEN).map_err(|_| abend(ctx, "ERROR OPENING DAILY TRANSACTION FILE", "35"))?;
    let xref_ds = ctx.vsam_name("XREFFILE")?;
    let acct_ds = ctx.vsam_name("ACCTFILE")?;
    let mut last = DalytranRecord::new();
    let mut eof = false;
    while !eof {
        match f.read() {
            Some(r) => {
                last = DalytranRecord::from_bytes(&r);
                ctx.display(text::s(&last.buf));
            }
            None => eof = true,
        }
        // the lookup below also runs once more after end of file (as in the original)
        match ctx.vsam.read(&xref_ds, last.dalytran_card_num()) {
            Ok(r) => {
                let x = CardXrefRecord::from_bytes(&r);
                ctx.display("SUCCESSFUL READ OF XREF");
                ctx.display(format!("CARD NUMBER: {}", text::s(x.xref_card_num())));
                ctx.display(format!("ACCOUNT ID : {}", text::s(x.xref_acct_id())));
                ctx.display(format!("CUSTOMER ID: {}", text::s(x.xref_cust_id())));
                match ctx.vsam.read(&acct_ds, x.xref_acct_id()) {
                    Ok(_) => ctx.display("SUCCESSFUL READ OF ACCOUNT FILE"),
                    Err(_) => {
                        ctx.display("INVALID ACCOUNT NUMBER FOUND");
                        ctx.display(format!("ACCOUNT {} NOT FOUND", text::s(x.xref_acct_id())));
                    }
                }
            }
            Err(_) => {
                ctx.display("INVALID CARD NUMBER FOR XREF");
                ctx.display(format!(
                    "CARD NUMBER {} COULD NOT BE VERIFIED. SKIPPING TRANSACTION ID-{}",
                    text::s(last.dalytran_card_num()),
                    text::s(last.dalytran_id())
                ));
            }
        }
    }
    ctx.display("END OF EXECUTION OF PROGRAM CBTRN01C");
    Ok(0)
}

/// COBSWAIT — wait the number of centiseconds given on SYSIN
pub fn cobswait(ctx: &mut StepCtx) -> StepResult {
    let mut sysin = ctx.input("SYSIN", 80)?;
    let parm = sysin.read().unwrap_or_default();
    let cs: u64 = text::s(&parm[..8.min(parm.len())]).trim().parse().unwrap_or(0);
    std::thread::sleep(std::time::Duration::from_millis(cs * 10));
    Ok(0)
}

#[allow(dead_code)]
fn edited(v: Dec, pic: &str) -> String {
    edit(v, pic)
}
