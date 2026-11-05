//! CBACT04C — monthly interest calculation (job INTCALC). Requirements FR-BAT-INT-01..07, KD-01.
//!
//! DD names: TCATBALF (KSDS, sequential), XREFFILE (KSDS; read by alternate key
//! account id), DISCGRP (KSDS), ACCTFILE (I-O), TRANSACT (sequential output).

use crate::ctx::{abend, db2_timestamp, Abend, StepCtx, StepResult};
use cobol::copybooks::{AccountRecord, CardXrefRecord, DisGroupRecord, TranCatBalRecord, TranRecord};
use cobol::numeric::{fit, Pic};
use cobol::{text, Dec};

#[allow(unused_assignments)]
pub fn run(ctx: &mut StepCtx) -> StepResult {
    ctx.display("START OF EXECUTION OF PROGRAM CBACT04C");
    // PARM-DATE PIC X(10)
    let mut parm_date = [b' '; 10];
    text::mv(&mut parm_date, ctx.parm.as_bytes());

    let mut tcatbal = ctx.input("TCATBALF", TranCatBalRecord::LEN).map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION CATEGORY BALANCE", "35"))?;
    let xref_ds = ctx.vsam_name("XREFFILE").map_err(|_| abend(ctx, "ERROR OPENING CROSS REF FILE", "35"))?;
    let discgrp = ctx.vsam_name("DISCGRP").map_err(|_| abend(ctx, "ERROR OPENING DALY REJECTS FILE", "35"))?;
    let acctfile = ctx.vsam_name("ACCTFILE").map_err(|_| abend(ctx, "ERROR OPENING ACCOUNT MASTER FILE", "35"))?;
    let mut transact = ctx.output("TRANSACT").map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION FILE", "35"))?;
    // alternate-key path over the XREF cluster (DD XREFFIL1 / CXACAIX)
    let xref_by_acct = if xref_ds == "CARDXREF" { "CXACAIX".to_string() } else { xref_ds.clone() };

    let mut last_acct_num = [b' '; 11];
    let mut total_int = Dec::ZERO;
    let mut first_time = true;
    let mut tranid_suffix: u32 = 0;
    let mut acct = AccountRecord::new();
    let mut xref = CardXrefRecord::new();
    let mut dis = DisGroupRecord::new();
    let mut tran = TranRecord::new();
    let int_pic = Pic::display(11, 2, true);

    while let Some(raw) = tcatbal.read() {
        let tcb = TranCatBalRecord::from_bytes(&raw);
        ctx.display(text::s(&tcb.buf));
        if tcb.trancat_acct_id() != &last_acct_num[..] {
            if !first_time {
                update_account(ctx, &mut acct, total_int, &acctfile)?;
            } else {
                first_time = false;
            }
            total_int = Dec::ZERO;
            last_acct_num.copy_from_slice(tcb.trancat_acct_id());
            // 1100-GET-ACCT-DATA
            match ctx.vsam.read(&acctfile, tcb.trancat_acct_id()) {
                Ok(r) => acct = AccountRecord::from_bytes(&r),
                Err(e) => {
                    ctx.display(format!("ACCOUNT NOT FOUND: {}", text::s(tcb.trancat_acct_id())));
                    return Err(abend(ctx, "ERROR READING ACCOUNT FILE", e.file_status()));
                }
            }
            // 1110-GET-XREF-DATA (alternate key)
            match ctx.vsam.read_ex(&xref_by_acct, tcb.trancat_acct_id()) {
                Ok((r, false)) => xref = CardXrefRecord::from_bytes(&r),
                Ok((_, true)) => return Err(abend(ctx, "ERROR READING XREF FILE", "02")),
                Err(e) => {
                    ctx.display(format!("ACCOUNT NOT FOUND: {}", text::s(tcb.trancat_acct_id())));
                    return Err(abend(ctx, "ERROR READING XREF FILE", e.file_status()));
                }
            }
        }
        // 1200-GET-INTEREST-RATE
        let mut key = Vec::with_capacity(16);
        key.extend_from_slice(acct.acct_group_id());
        key.extend_from_slice(tcb.trancat_type_cd());
        key.extend_from_slice(tcb.trancat_cd());
        match ctx.vsam.read(&discgrp, &key) {
            Ok(r) => dis = DisGroupRecord::from_bytes(&r),
            Err(vsam::Resp::NotFnd) => {
                ctx.display("DISCLOSURE GROUP RECORD MISSING");
                ctx.display("TRY WITH DEFAULT GROUP CODE");
                // 1200-A-GET-DEFAULT-INT-RATE
                let mut dkey = [b' '; 16];
                text::mv(&mut dkey[..10], b"DEFAULT");
                dkey[10..].copy_from_slice(&key[10..]);
                match ctx.vsam.read(&discgrp, &dkey) {
                    Ok(r) => dis = DisGroupRecord::from_bytes(&r),
                    Err(e) => return Err(abend(ctx, "ERROR READING DEFAULT DISCLOSURE GROUP", e.file_status())),
                }
            }
            Err(e) => return Err(abend(ctx, "ERROR READING DISCLOSURE GROUP FILE", e.file_status())),
        }
        if dis.dis_int_rate_n() != Dec::ZERO {
            // 1300-COMPUTE-INTEREST
            let monthly = fit(tcb.tran_cat_bal_n() * dis.dis_int_rate_n() / Dec::from(1200), int_pic);
            total_int = fit(total_int + monthly, int_pic);
            // 1300-B-WRITE-TX
            tranid_suffix = (tranid_suffix + 1) % 1_000_000;
            let mut id = Vec::with_capacity(16);
            id.extend_from_slice(&parm_date);
            id.extend_from_slice(format!("{tranid_suffix:06}").as_bytes());
            tran.set_tran_id(&id);
            tran.set_tran_type_cd(b"01");
            tran.set_tran_cat_cd_n(Dec::from(5));
            tran.set_tran_source(b"System");
            let mut desc = tran.tran_desc().to_vec();
            text::string_into(&mut desc, &[b"Int. for a/c ", acct.acct_id()]);
            tran.set_tran_desc(&desc);
            tran.set_tran_amt_n(monthly);
            tran.set_tran_merchant_id_n(Dec::ZERO);
            tran.set_tran_merchant_name(b"");
            tran.set_tran_merchant_city(b"");
            tran.set_tran_merchant_zip(b"");
            tran.set_tran_card_num(xref.xref_card_num());
            let ts = db2_timestamp();
            tran.set_tran_orig_ts(ts.as_bytes());
            tran.set_tran_proc_ts(ts.as_bytes());
            transact.write(&tran.buf).map_err(|_| abend(ctx, "ERROR WRITING TRANSACTION RECORD", "30"))?;
            // 1400-COMPUTE-FEES: not implemented in the original (FR-BAT-INT-07)
        }
    }
    // KD-01: the end-of-file branch that would update the last account is unreachable.
    transact.close().map_err(|_| abend(ctx, "ERROR CLOSING TRANSACTION FILE", "30"))?;
    ctx.display("END OF EXECUTION OF PROGRAM CBACT04C");
    Ok(0)
}

/// 1050-UPDATE-ACCOUNT
fn update_account(ctx: &mut StepCtx, acct: &mut AccountRecord, total_int: Dec, acctfile: &str) -> Result<(), Abend> {
    acct.set_acct_curr_bal_n(acct.acct_curr_bal_n() + total_int);
    acct.set_acct_curr_cyc_credit_n(Dec::ZERO);
    acct.set_acct_curr_cyc_debit_n(Dec::ZERO);
    ctx.vsam.rewrite(acctfile, &acct.buf).map_err(|e| abend(ctx, "ERROR RE-WRITING ACCOUNT FILE", e.file_status()))
}
