//! CBTRN03C — daily transaction detail report (job TRANREPT). FR-BAT-REPT-01..08, KD-02, KD-03.
//!
//! DD names: TRANFILE (sequential, sorted by card), CARDXREF, TRANTYPE, TRANCATG (KSDS),
//! DATEPARM (80-byte parameter record), TRANREPT (133-byte report lines).

use crate::ctx::{abend, Abend, StepCtx, StepResult};
use crate::seqfile::SeqOut;
use cobol::copybooks::*;
use cobol::numeric::{fit, Pic};
use cobol::{text, Dec};

struct Rpt {
    out: SeqOut,
    line_counter: u64,
    page_total: Dec,
    account_total: Dec,
    grand_total: Dec,
}

const PAGE_SIZE: u64 = 20;
const TOT: Pic = Pic::display(11, 2, true);

impl Rpt {
    /// 1111-WRITE-REPORT-REC
    fn write(&mut self, ctx: &mut StepCtx, rec: &[u8]) -> Result<(), Abend> {
        let mut line = [b' '; 133];
        text::mv(&mut line, rec);
        self.out.write(&line).map_err(|_| abend(ctx, "ERROR WRITING REPTFILE", "30"))
    }
    /// 1110-WRITE-PAGE-TOTALS
    fn page_totals(&mut self, ctx: &mut StepCtx) -> Result<(), Abend> {
        let mut r = ReportPageTotals::new();
        r.set_rept_page_total_n(self.page_total);
        self.write(ctx, &r.buf)?;
        self.grand_total = fit(self.grand_total + self.page_total, TOT);
        self.page_total = Dec::ZERO;
        self.line_counter += 1;
        self.write(ctx, &TransactionHeader2::new().buf)?;
        self.line_counter += 1;
        Ok(())
    }
    /// 1120-WRITE-ACCOUNT-TOTALS
    fn account_totals(&mut self, ctx: &mut StepCtx) -> Result<(), Abend> {
        let mut r = ReportAccountTotals::new();
        r.set_rept_account_total_n(self.account_total);
        self.write(ctx, &r.buf)?;
        self.account_total = Dec::ZERO;
        self.line_counter += 1;
        self.write(ctx, &TransactionHeader2::new().buf)?;
        self.line_counter += 1;
        Ok(())
    }
    /// 1110-WRITE-GRAND-TOTALS
    fn grand_totals(&mut self, ctx: &mut StepCtx) -> Result<(), Abend> {
        let mut r = ReportGrandTotals::new();
        r.set_rept_grand_total_n(self.grand_total);
        self.write(ctx, &r.buf)
    }
    /// 1120-WRITE-HEADERS
    fn headers(&mut self, ctx: &mut StepCtx, hdr: &ReportNameHeader) -> Result<(), Abend> {
        self.write(ctx, &hdr.buf)?;
        self.line_counter += 1;
        self.write(ctx, b"")?;
        self.line_counter += 1;
        self.write(ctx, &TransactionHeader1::new().buf)?;
        self.line_counter += 1;
        self.write(ctx, &TransactionHeader2::new().buf)?;
        self.line_counter += 1;
        Ok(())
    }
}

#[allow(unused_assignments)]
pub fn run(ctx: &mut StepCtx) -> StepResult {
    ctx.display("START OF EXECUTION OF PROGRAM CBTRN03C");
    let mut tranfile = ctx.input("TRANFILE", TranRecord::LEN).map_err(|_| abend(ctx, "ERROR OPENING TRANFILE", "35"))?;
    let out = ctx.output("TRANREPT").map_err(|_| abend(ctx, "ERROR OPENING REPTFILE", "35"))?;
    let xref_ds = ctx.vsam_name("CARDXREF").map_err(|_| abend(ctx, "ERROR OPENING CROSS REF FILE", "35"))?;
    let type_ds = ctx.vsam_name("TRANTYPE").map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION TYPE FILE", "35"))?;
    let catg_ds = ctx.vsam_name("TRANCATG").map_err(|_| abend(ctx, "ERROR OPENING TRANSACTION CATG FILE", "35"))?;
    let mut dateparm = ctx.input("DATEPARM", 80).map_err(|_| abend(ctx, "ERROR OPENING DATE PARM FILE", "35"))?;

    let mut rpt = Rpt { out, line_counter: 0, page_total: Dec::ZERO, account_total: Dec::ZERO, grand_total: Dec::ZERO };
    let mut end_of_file = false;
    // 0550-DATEPARM-READ
    let (start, end) = match dateparm.read() {
        Some(p) => {
            let (s, e) = (p[0..10].to_vec(), p[11..21].to_vec());
            ctx.display(format!("Reporting from {} to {}", text::s(&s), text::s(&e)));
            (s, e)
        }
        None => {
            end_of_file = true;
            (vec![b' '; 10], vec![b' '; 10])
        }
    };

    let mut first_time = true;
    let mut curr_card = [b' '; 16];
    let mut tran = TranRecord::new();
    let mut xref = CardXrefRecord::new();
    let mut ttype = TranTypeRecord::new();
    let mut tcat = TranCatRecord::new();
    let mut hdr = ReportNameHeader::new();

    while !end_of_file {
        // 1000-TRANFILE-GET-NEXT
        match tranfile.read() {
            Some(r) => tran = TranRecord::from_bytes(&r),
            None => end_of_file = true,
        }
        let ts = &tran.tran_proc_ts()[..10];
        if !(ts >= &start[..] && ts <= &end[..]) {
            // NEXT SENTENCE: control passes beyond the period that ends the inline
            // PERFORM, i.e. the loop is left without writing totals.
            break;
        }
        if !end_of_file {
            ctx.display(text::s(&tran.buf));
            if &curr_card[..] != tran.tran_card_num() {
                if !first_time {
                    rpt.account_totals(ctx)?;
                }
                curr_card.copy_from_slice(tran.tran_card_num());
                // 1500-A-LOOKUP-XREF
                match ctx.vsam.read(&xref_ds, tran.tran_card_num()) {
                    Ok(r) => xref = CardXrefRecord::from_bytes(&r),
                    Err(_) => {
                        ctx.display(format!("INVALID CARD NUMBER : {}", text::s(tran.tran_card_num())));
                        return Err(abend_23(ctx));
                    }
                }
            }
            // 1500-B-LOOKUP-TRANTYPE
            match ctx.vsam.read(&type_ds, tran.tran_type_cd()) {
                Ok(r) => ttype = TranTypeRecord::from_bytes(&r),
                Err(_) => {
                    ctx.display(format!("INVALID TRANSACTION TYPE : {}", text::s(tran.tran_type_cd())));
                    return Err(abend_23(ctx));
                }
            }
            // 1500-C-LOOKUP-TRANCATG
            let mut ck = tran.tran_type_cd().to_vec();
            ck.extend_from_slice(tran.tran_cat_cd());
            match ctx.vsam.read(&catg_ds, &ck) {
                Ok(r) => tcat = TranCatRecord::from_bytes(&r),
                Err(_) => {
                    ctx.display(format!("INVALID TRAN CATG KEY : {}", text::s(&ck)));
                    return Err(abend_23(ctx));
                }
            }
            // 1100-WRITE-TRANSACTION-REPORT
            if first_time {
                first_time = false;
                hdr.set_rept_start_date(&start);
                hdr.set_rept_end_date(&end);
                rpt.headers(ctx, &hdr)?;
            }
            if rpt.line_counter % PAGE_SIZE == 0 {
                rpt.page_totals(ctx)?;
                rpt.headers(ctx, &hdr)?;
            }
            rpt.page_total = fit(rpt.page_total + tran.tran_amt_n(), TOT);
            rpt.account_total = fit(rpt.account_total + tran.tran_amt_n(), TOT);
            // 1120-WRITE-DETAIL
            let mut d = TransactionDetailReport::new();
            d.set_tran_report_trans_id(tran.tran_id());
            d.set_tran_report_account_id(xref.xref_acct_id());
            d.set_tran_report_type_cd(tran.tran_type_cd());
            d.set_tran_report_type_desc(ttype.tran_type_desc());
            d.set_tran_report_cat_cd(tran.tran_cat_cd());
            d.set_tran_report_cat_desc(tcat.tran_cat_type_desc());
            d.set_tran_report_source(tran.tran_source());
            d.set_tran_report_amt_n(tran.tran_amt_n());
            rpt.write(ctx, &d.buf)?;
            rpt.line_counter += 1;
        } else {
            ctx.display(format!("TRAN-AMT {}", text::s(tran.tran_amt())));
            ctx.display(format!("WS-PAGE-TOTAL{}", display_signed(rpt.page_total)));
            // KD-02: the last record's amount is added again
            rpt.page_total = fit(rpt.page_total + tran.tran_amt_n(), TOT);
            rpt.account_total = fit(rpt.account_total + tran.tran_amt_n(), TOT);
            rpt.page_totals(ctx)?;
            rpt.grand_totals(ctx)?;
            // KD-03: no final account totals line
        }
    }
    rpt.out.close().map_err(|_| abend(ctx, "ERROR CLOSING REPORT FILE", "30"))?;
    ctx.display("END OF EXECUTION OF PROGRAM CBTRN03C");
    Ok(0)
}

fn abend_23(ctx: &mut StepCtx) -> Abend {
    ctx.display(crate::ctx::io_status_line("23"));
    ctx.display("ABENDING PROGRAM");
    Abend(999)
}

/// DISPLAY of a PIC S9(09)V99 item (zoned, overpunched sign).
fn display_signed(v: Dec) -> String {
    let mut b = [0u8; 11];
    cobol::numeric::encode(v, TOT, &mut b);
    text::s(&b)
}
