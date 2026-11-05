//! CBTRN02C — post the daily transaction file (job POSTTRAN). Requirements FR-BAT-POST-01..11.
//!
//! DD names: DALYTRAN (in), TRANFILE (TRANSACT KSDS, output), XREFFILE, DALYREJS (out),
//! ACCTFILE (I-O), TCATBALF (I-O).

use crate::ctx::{abend, db2_timestamp, StepCtx, StepResult};
use cobol::copybooks::{AccountRecord, CardXrefRecord, DalytranRecord, TranCatBalRecord, TranRecord};
use cobol::numeric::{fit, Pic};
use cobol::text;

struct Ws {
    transaction_count: u64,
    reject_count: u64,
    fail_reason: u32,
    fail_reason_desc: String,
    xref: CardXrefRecord,
    acct: AccountRecord,
}

pub fn run(ctx: &mut StepCtx) -> StepResult {
    ctx.display("START OF EXECUTION OF PROGRAM CBTRN02C");
    // 0000..0500 OPEN
    let mut dalytran = ctx.input("DALYTRAN", DalytranRecord::LEN).map_err(|_| abend(ctx, "ERROR OPENING DALYTRAN", "35"))?;
    let tranfile = ctx.vsam_name("TRANFILE").map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION FILE", "35"))?;
    let xreffile = ctx.vsam_name("XREFFILE").map_err(|_| abend(ctx, "ERROR OPENING CROSS REF FILE", "35"))?;
    let mut dalyrejs = ctx.output("DALYREJS").map_err(|_| abend(ctx, "ERROR OPENING DALY REJECTS FILE", "35"))?;
    let acctfile = ctx.vsam_name("ACCTFILE").map_err(|_| abend(ctx, "ERROR OPENING ACCOUNT MASTER FILE", "35"))?;
    let tcatbalf = ctx.vsam_name("TCATBALF").map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION BALANCE FILE", "35"))?;

    let mut ws = Ws {
        transaction_count: 0,
        reject_count: 0,
        fail_reason: 0,
        fail_reason_desc: String::new(),
        xref: CardXrefRecord::new(),
        acct: AccountRecord::new(),
    };

    // PERFORM UNTIL END-OF-FILE = 'Y'
    while let Some(raw) = dalytran.read() {
        let dly = DalytranRecord::from_bytes(&raw);
        ws.transaction_count += 1;
        ws.fail_reason = 0;
        ws.fail_reason_desc.clear();
        validate_tran(ctx, &mut ws, &dly, &xreffile, &acctfile);
        if ws.fail_reason == 0 {
            post_transaction(ctx, &mut ws, &dly, &tranfile, &acctfile, &tcatbalf)?;
        } else {
            ws.reject_count += 1;
            // 2500-WRITE-REJECT-REC
            let mut rej = Vec::with_capacity(430);
            rej.extend_from_slice(&dly.buf);
            let mut trailer = [b' '; 80];
            trailer[..4].copy_from_slice(format!("{:04}", ws.fail_reason % 10000).as_bytes());
            text::mv(&mut trailer[4..], ws.fail_reason_desc.as_bytes());
            rej.extend_from_slice(&trailer);
            dalyrejs.write(&rej).map_err(|_| abend(ctx, "ERROR WRITING TO REJECTS FILE", "30"))?;
        }
    }
    dalyrejs.close().map_err(|_| abend(ctx, "ERROR CLOSING DAILY REJECTS FILE", "30"))?;
    ctx.display(format!("TRANSACTIONS PROCESSED :{:09}", ws.transaction_count));
    ctx.display(format!("TRANSACTIONS REJECTED  :{:09}", ws.reject_count));
    let rc = if ws.reject_count > 0 { 4 } else { 0 };
    ctx.display("END OF EXECUTION OF PROGRAM CBTRN02C");
    Ok(rc)
}

/// 1500-VALIDATE-TRAN
fn validate_tran(ctx: &mut StepCtx, ws: &mut Ws, dly: &DalytranRecord, xreffile: &str, acctfile: &str) {
    // 1500-A-LOOKUP-XREF
    match ctx.vsam.read(xreffile, dly.dalytran_card_num()) {
        Ok(r) => ws.xref = CardXrefRecord::from_bytes(&r),
        Err(_) => {
            ws.fail_reason = 100;
            ws.fail_reason_desc = "INVALID CARD NUMBER FOUND".into();
        }
    }
    if ws.fail_reason != 0 {
        return;
    }
    // 1500-B-LOOKUP-ACCT
    match ctx.vsam.read(acctfile, ws.xref.xref_acct_id()) {
        Err(_) => {
            ws.fail_reason = 101;
            ws.fail_reason_desc = "ACCOUNT RECORD NOT FOUND".into();
        }
        Ok(r) => {
            ws.acct = AccountRecord::from_bytes(&r);
            let temp_bal = fit(
                ws.acct.acct_curr_cyc_credit_n() - ws.acct.acct_curr_cyc_debit_n() + dly.dalytran_amt_n(),
                Pic::display(11, 2, true),
            );
            if ws.acct.acct_credit_limit_n() < temp_bal {
                ws.fail_reason = 102;
                ws.fail_reason_desc = "OVERLIMIT TRANSACTION".into();
            }
            if ws.acct.acct_expiraion_date() < &dly.dalytran_orig_ts()[..10] {
                ws.fail_reason = 103;
                ws.fail_reason_desc = "TRANSACTION RECEIVED AFTER ACCT EXPIRATION".into();
            }
        }
    }
}

/// 2000-POST-TRANSACTION
fn post_transaction(
    ctx: &mut StepCtx,
    ws: &mut Ws,
    dly: &DalytranRecord,
    tranfile: &str,
    acctfile: &str,
    tcatbalf: &str,
) -> Result<(), crate::ctx::Abend> {
    let mut tran = TranRecord::new();
    tran.set_tran_id(dly.dalytran_id());
    tran.set_tran_type_cd(dly.dalytran_type_cd());
    tran.set_tran_cat_cd(dly.dalytran_cat_cd());
    tran.set_tran_source(dly.dalytran_source());
    tran.set_tran_desc(dly.dalytran_desc());
    tran.set_tran_amt(dly.dalytran_amt());
    tran.set_tran_merchant_id(dly.dalytran_merchant_id());
    tran.set_tran_merchant_name(dly.dalytran_merchant_name());
    tran.set_tran_merchant_city(dly.dalytran_merchant_city());
    tran.set_tran_merchant_zip(dly.dalytran_merchant_zip());
    tran.set_tran_card_num(dly.dalytran_card_num());
    tran.set_tran_orig_ts(dly.dalytran_orig_ts());
    tran.set_tran_proc_ts(db2_timestamp().as_bytes());

    // 2700-UPDATE-TCATBAL
    let mut key = Vec::with_capacity(17);
    key.extend_from_slice(ws.xref.xref_acct_id());
    key.extend_from_slice(dly.dalytran_type_cd());
    key.extend_from_slice(dly.dalytran_cat_cd());
    match ctx.vsam.read(tcatbalf, &key) {
        Err(vsam::Resp::NotFnd) => {
            ctx.display(format!("TCATBAL record not found for key : {}.. Creating.", text::s(&key)));
            // 2700-A-CREATE-TCATBAL-REC
            let mut tc = TranCatBalRecord::new();
            tc.set_trancat_acct_id(ws.xref.xref_acct_id());
            tc.set_trancat_type_cd(dly.dalytran_type_cd());
            tc.set_trancat_cd(dly.dalytran_cat_cd());
            tc.set_tran_cat_bal_n(tc.tran_cat_bal_n() + dly.dalytran_amt_n());
            ctx.vsam.write(tcatbalf, &tc.buf).map_err(|e| abend(ctx, "ERROR WRITING TRANSACTION BALANCE FILE", e.file_status()))?;
        }
        Err(e) => return Err(abend(ctx, "ERROR READING TRANSACTION BALANCE FILE", e.file_status())),
        Ok(r) => {
            // 2700-B-UPDATE-TCATBAL-REC
            let mut tc = TranCatBalRecord::from_bytes(&r);
            tc.set_tran_cat_bal_n(tc.tran_cat_bal_n() + dly.dalytran_amt_n());
            ctx.vsam.rewrite(tcatbalf, &tc.buf).map_err(|e| abend(ctx, "ERROR REWRITING TRANSACTION BALANCE FILE", e.file_status()))?;
        }
    }

    // 2800-UPDATE-ACCOUNT-REC
    let amt = dly.dalytran_amt_n();
    ws.acct.set_acct_curr_bal_n(ws.acct.acct_curr_bal_n() + amt);
    if amt >= cobol::Dec::ZERO {
        ws.acct.set_acct_curr_cyc_credit_n(ws.acct.acct_curr_cyc_credit_n() + amt);
    } else {
        ws.acct.set_acct_curr_cyc_debit_n(ws.acct.acct_curr_cyc_debit_n() + amt);
    }
    if ctx.vsam.rewrite(acctfile, &ws.acct.buf).is_err() {
        ws.fail_reason = 109;
        ws.fail_reason_desc = "ACCOUNT RECORD NOT FOUND".into();
    }

    // 2900-WRITE-TRANSACTION-FILE
    ctx.vsam.write(tranfile, &tran.buf).map_err(|e| abend(ctx, "ERROR WRITING TO TRANSACTION FILE", e.file_status()))?;
    Ok(())
}
