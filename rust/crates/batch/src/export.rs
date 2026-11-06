//! CBEXPORT / CBIMPORT — multi-record export file for branch migration
//! (FR-BAT-XPRT-01..04). The export file holds 500-byte CVEXPORT records with mixed
//! DISPLAY / COMP / COMP-3 fields; it is written as a fixed-length binary file in
//! sequence order (the original KSDS is keyed on the sequence number).

use crate::ctx::{current_date, Abend, StepCtx, StepResult};
use cobol::copybooks::*;
use cobol::text;

fn fail(ctx: &mut StepCtx, msg: &str) -> Abend {
    ctx.display(msg);
    ctx.display("CBEXPORT: ABENDING PROGRAM");
    Abend(999)
}

pub fn cbexport(ctx: &mut StepCtx) -> StepResult {
    ctx.display("CBEXPORT: Starting Customer Data Export");
    let c = current_date();
    let date = format!("{}-{}-{}", &c[0..4], &c[4..6], &c[6..8]);
    let time = format!("{}:{}:{}", &c[8..10], &c[10..12], &c[12..14]);
    // STRING ... INTO WS-FORMATTED-TIMESTAMP (X(26), no VALUE): the 4 bytes after the 22 written stay X'00'
    let mut ts = [0u8; 26];
    text::string_into(&mut ts, &[date.as_bytes(), b" ", time.as_bytes(), b".00"]);
    let mut out = ctx.output("EXPFILE").map_err(|_| fail(ctx, "ERROR: Cannot open EXPORT-OUTPUT, Status: 35"))?;
    ctx.display(format!("CBEXPORT: Export Date: {date}"));
    ctx.display(format!("CBEXPORT: Export Time: {time}"));
    let mut seq: u64 = 0;
    let mut counts = [0u64; 5];
    let base = |seq: u64, t: u8| {
        let mut e = ExportRecord::new();
        e.set_export_rec_type(&[t]);
        e.set_export_timestamp(&ts);
        e.set_export_sequence_num_n(seq.into());
        e.set_export_branch_id(b"0001");
        e.set_export_region_code(b"NORTH");
        e
    };
    let steps: [(&str, &str, &str, &str, usize); 5] = [
        ("CUSTFILE", "customer records", "Customers exported: ", "C", CustomerRecord::LEN),
        ("ACCTFILE", "account records", "Accounts exported: ", "A", AccountRecord::LEN),
        ("XREFFILE", "cross-reference records", "Cross-references exported: ", "X", CardXrefRecord::LEN),
        ("TRANSACT", "transaction records", "Transactions exported: ", "T", TranRecord::LEN),
        ("CARDFILE", "card records", "Cards exported: ", "D", CardRecord::LEN),
    ];
    for (i, (dd, what, done, t, len)) in steps.iter().enumerate() {
        ctx.display(format!("CBEXPORT: Processing {what}"));
        let mut f = ctx.input(dd, *len).map_err(|_| fail(ctx, &format!("ERROR: Cannot open {dd}, Status: 35")))?;
        while let Some(r) = f.read() {
            seq += 1;
            let mut e = base(seq, t.as_bytes()[0]);
            match *t {
                "C" => {
                    let c = CustomerRecord::from_bytes(&r);
                    e.set_exp_cust_id_n(c.cust_id_n());
                    e.set_exp_cust_first_name(c.cust_first_name());
                    e.set_exp_cust_middle_name(c.cust_middle_name());
                    e.set_exp_cust_last_name(c.cust_last_name());
                    e.set_exp_cust_addr_line(1, c.cust_addr_line_1());
                    e.set_exp_cust_addr_line(2, c.cust_addr_line_2());
                    e.set_exp_cust_addr_line(3, c.cust_addr_line_3());
                    e.set_exp_cust_addr_state_cd(c.cust_addr_state_cd());
                    e.set_exp_cust_addr_country_cd(c.cust_addr_country_cd());
                    e.set_exp_cust_addr_zip(c.cust_addr_zip());
                    e.set_exp_cust_phone_num(1, c.cust_phone_num_1());
                    e.set_exp_cust_phone_num(2, c.cust_phone_num_2());
                    e.set_exp_cust_ssn_n(c.cust_ssn_n());
                    e.set_exp_cust_govt_issued_id(c.cust_govt_issued_id());
                    e.set_exp_cust_dob_yyyy_mm_dd(c.cust_dob_yyyy_mm_dd());
                    e.set_exp_cust_eft_account_id(c.cust_eft_account_id());
                    e.set_exp_cust_pri_card_holder_ind(c.cust_pri_card_holder_ind());
                    e.set_exp_cust_fico_credit_score_n(c.cust_fico_credit_score_n());
                }
                "A" => {
                    let a = AccountRecord::from_bytes(&r);
                    e.set_exp_acct_id_n(a.acct_id_n());
                    e.set_exp_acct_active_status(a.acct_active_status());
                    e.set_exp_acct_curr_bal_n(a.acct_curr_bal_n());
                    e.set_exp_acct_credit_limit_n(a.acct_credit_limit_n());
                    e.set_exp_acct_cash_credit_limit_n(a.acct_cash_credit_limit_n());
                    e.set_exp_acct_open_date(a.acct_open_date());
                    e.set_exp_acct_expiraion_date(a.acct_expiraion_date());
                    e.set_exp_acct_reissue_date(a.acct_reissue_date());
                    e.set_exp_acct_curr_cyc_credit_n(a.acct_curr_cyc_credit_n());
                    e.set_exp_acct_curr_cyc_debit_n(a.acct_curr_cyc_debit_n());
                    e.set_exp_acct_addr_zip(a.acct_addr_zip());
                    e.set_exp_acct_group_id(a.acct_group_id());
                }
                "X" => {
                    let x = CardXrefRecord::from_bytes(&r);
                    e.set_exp_xref_card_num(x.xref_card_num());
                    e.set_exp_xref_cust_id_n(x.xref_cust_id_n());
                    e.set_exp_xref_acct_id_n(x.xref_acct_id_n());
                }
                "T" => {
                    let tr = TranRecord::from_bytes(&r);
                    e.set_exp_tran_id(tr.tran_id());
                    e.set_exp_tran_type_cd(tr.tran_type_cd());
                    e.set_exp_tran_cat_cd_n(tr.tran_cat_cd_n());
                    e.set_exp_tran_source(tr.tran_source());
                    e.set_exp_tran_desc(tr.tran_desc());
                    e.set_exp_tran_amt_n(tr.tran_amt_n());
                    e.set_exp_tran_merchant_id_n(tr.tran_merchant_id_n());
                    e.set_exp_tran_merchant_name(tr.tran_merchant_name());
                    e.set_exp_tran_merchant_city(tr.tran_merchant_city());
                    e.set_exp_tran_merchant_zip(tr.tran_merchant_zip());
                    e.set_exp_tran_card_num(tr.tran_card_num());
                    e.set_exp_tran_orig_ts(tr.tran_orig_ts());
                    e.set_exp_tran_proc_ts(tr.tran_proc_ts());
                }
                _ => {
                    let d = CardRecord::from_bytes(&r);
                    e.set_exp_card_num(d.card_num());
                    e.set_exp_card_acct_id_n(d.card_acct_id_n());
                    e.set_exp_card_cvv_cd_n(d.card_cvv_cd_n());
                    e.set_exp_card_embossed_name(d.card_embossed_name());
                    e.set_exp_card_expiraion_date(d.card_expiraion_date());
                    e.set_exp_card_active_status(d.card_active_status());
                }
            }
            out.write(&e.buf).map_err(|_| fail(ctx, "ERROR: Writing export record, Status: 30"))?;
            counts[i] += 1;
        }
        ctx.display(format!("CBEXPORT: {done}{:09}", counts[i]));
    }
    out.close().map_err(|_| Abend(999))?;
    ctx.display("CBEXPORT: Export completed");
    for (l, n) in ["Customers Exported: ", "Accounts Exported: ", "XRefs Exported: ", "Transactions Exported: ", "Cards Exported: "].iter().zip(counts) {
        ctx.display(format!("CBEXPORT: {l}{n:09}"));
    }
    ctx.display(format!("CBEXPORT: Total Records Exported: {:09}", counts.iter().sum::<u64>()));
    Ok(0)
}

pub fn cbimport(ctx: &mut StepCtx) -> StepResult {
    ctx.display("CBIMPORT: Starting Customer Data Import");
    let c = current_date();
    ctx.display(format!("CBIMPORT: Import Date: {}-{}-{}", &c[0..4], &c[4..6], &c[6..8]));
    ctx.display(format!("CBIMPORT: Import Time: {}:{}:{}", &c[8..10], &c[10..12], &c[12..14]));
    let mut inp = ctx.input("EXPFILE", ExportRecord::LEN)?;
    let mut outs = Vec::new();
    for dd in ["CUSTOUT", "ACCTOUT", "XREFOUT", "TRNXOUT", "CARDOUT", "ERROUT"] {
        outs.push(ctx.output(dd).map_err(|_| fail(ctx, &format!("ERROR: Cannot open {dd}, Status: 35")))?);
    }
    let mut n = [0u64; 7]; // read, C, A, X, T, D, unknown
    // FD record areas: binary zeros at OPEN; INITIALIZE before each record leaves the FILLERs alone
    let mut c = CustomerRecord::from_bytes(&[0u8; CustomerRecord::LEN]);
    let mut a = AccountRecord::from_bytes(&[0u8; AccountRecord::LEN]);
    let mut x = CardXrefRecord::from_bytes(&[0u8; CardXrefRecord::LEN]);
    let mut t = TranRecord::from_bytes(&[0u8; TranRecord::LEN]);
    let mut d = CardRecord::from_bytes(&[0u8; CardRecord::LEN]);
    while let Some(r) = inp.read() {
        n[0] += 1;
        let e = ExportRecord::from_bytes(&r);
        match e.export_rec_type() {
            b"C" => {
                c.initialize();
                c.set_cust_id_n(e.exp_cust_id_n());
                c.set_cust_first_name(e.exp_cust_first_name());
                c.set_cust_middle_name(e.exp_cust_middle_name());
                c.set_cust_last_name(e.exp_cust_last_name());
                c.set_cust_addr_line_1(e.exp_cust_addr_line(1));
                c.set_cust_addr_line_2(e.exp_cust_addr_line(2));
                c.set_cust_addr_line_3(e.exp_cust_addr_line(3));
                c.set_cust_addr_state_cd(e.exp_cust_addr_state_cd());
                c.set_cust_addr_country_cd(e.exp_cust_addr_country_cd());
                c.set_cust_addr_zip(e.exp_cust_addr_zip());
                c.set_cust_phone_num_1(e.exp_cust_phone_num(1));
                c.set_cust_phone_num_2(e.exp_cust_phone_num(2));
                c.set_cust_ssn_n(e.exp_cust_ssn_n());
                c.set_cust_govt_issued_id(e.exp_cust_govt_issued_id());
                c.set_cust_dob_yyyy_mm_dd(e.exp_cust_dob_yyyy_mm_dd());
                c.set_cust_eft_account_id(e.exp_cust_eft_account_id());
                c.set_cust_pri_card_holder_ind(e.exp_cust_pri_card_holder_ind());
                c.set_cust_fico_credit_score_n(e.exp_cust_fico_credit_score_n());
                outs[0].write(&c.buf).map_err(|_| Abend(999))?;
                n[1] += 1;
            }
            b"A" => {
                a.initialize();
                a.set_acct_id_n(e.exp_acct_id_n());
                a.set_acct_active_status(e.exp_acct_active_status());
                a.set_acct_curr_bal_n(e.exp_acct_curr_bal_n());
                a.set_acct_credit_limit_n(e.exp_acct_credit_limit_n());
                a.set_acct_cash_credit_limit_n(e.exp_acct_cash_credit_limit_n());
                a.set_acct_open_date(e.exp_acct_open_date());
                a.set_acct_expiraion_date(e.exp_acct_expiraion_date());
                a.set_acct_reissue_date(e.exp_acct_reissue_date());
                a.set_acct_curr_cyc_credit_n(e.exp_acct_curr_cyc_credit_n());
                a.set_acct_curr_cyc_debit_n(e.exp_acct_curr_cyc_debit_n());
                a.set_acct_addr_zip(e.exp_acct_addr_zip());
                a.set_acct_group_id(e.exp_acct_group_id());
                outs[1].write(&a.buf).map_err(|_| Abend(999))?;
                n[2] += 1;
            }
            b"X" => {
                x.initialize();
                x.set_xref_card_num(e.exp_xref_card_num());
                x.set_xref_cust_id_n(e.exp_xref_cust_id_n());
                x.set_xref_acct_id_n(e.exp_xref_acct_id_n());
                outs[2].write(&x.buf).map_err(|_| Abend(999))?;
                n[3] += 1;
            }
            b"T" => {
                t.initialize();
                t.set_tran_id(e.exp_tran_id());
                t.set_tran_type_cd(e.exp_tran_type_cd());
                t.set_tran_cat_cd_n(e.exp_tran_cat_cd_n());
                t.set_tran_source(e.exp_tran_source());
                t.set_tran_desc(e.exp_tran_desc());
                t.set_tran_amt_n(e.exp_tran_amt_n());
                t.set_tran_merchant_id_n(e.exp_tran_merchant_id_n());
                t.set_tran_merchant_name(e.exp_tran_merchant_name());
                t.set_tran_merchant_city(e.exp_tran_merchant_city());
                t.set_tran_merchant_zip(e.exp_tran_merchant_zip());
                t.set_tran_card_num(e.exp_tran_card_num());
                t.set_tran_orig_ts(e.exp_tran_orig_ts());
                t.set_tran_proc_ts(e.exp_tran_proc_ts());
                outs[3].write(&t.buf).map_err(|_| Abend(999))?;
                n[4] += 1;
            }
            b"D" => {
                d.initialize();
                d.set_card_num(e.exp_card_num());
                d.set_card_acct_id_n(e.exp_card_acct_id_n());
                d.set_card_cvv_cd_n(e.exp_card_cvv_cd_n());
                d.set_card_embossed_name(e.exp_card_embossed_name());
                d.set_card_expiraion_date(e.exp_card_expiraion_date());
                d.set_card_active_status(e.exp_card_active_status());
                outs[4].write(&d.buf).map_err(|_| Abend(999))?;
                n[5] += 1;
            }
            t => {
                n[6] += 1;
                let mut er = vec![b' '; 132];
                let ts = current_date();
                let seq = format!("{:07}", cobol::numeric::to_i64(e.export_sequence_num_n()) % 10_000_000);
                text::string_into(&mut er, &[ts.as_bytes(), &[b' '; 5], b"|", t, b"|", seq.as_bytes(), b"|", b"Unknown record type encountered"]);
                outs[5].write(&er).map_err(|_| Abend(999))?;
            }
        }
    }
    for o in outs {
        o.close().map_err(|_| Abend(999))?;
    }
    ctx.display("CBIMPORT: Import validation completed");
    ctx.display("CBIMPORT: No validation errors detected");
    ctx.display("CBIMPORT: Import completed");
    // WS-ERROR-RECORDS-WRITTEN: one error record per unknown record type (2700 -> 2750)
    let shown = [n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[6]];
    for (l, v) in ["Total Records Read: ", "Customers Imported: ", "Accounts Imported: ", "XRefs Imported: ", "Transactions Imported: ", "Cards Imported: ", "Errors Written: ", "Unknown Record Types: "].iter().zip(shown) {
        ctx.display(format!("CBIMPORT: {l}{v:09}"));
    }
    Ok(0)
}
