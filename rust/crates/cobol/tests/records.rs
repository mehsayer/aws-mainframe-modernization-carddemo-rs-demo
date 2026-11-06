use cobol::copybooks::*;
use cobol::{ebcdic, Dec};
use std::str::FromStr;

fn ebcdic_file(name: &str) -> Vec<u8> {
    let p = format!("{}/../../../cobol/app/data/EBCDIC/{name}", env!("CARGO_MANIFEST_DIR"));
    let mut b = std::fs::read(p).unwrap();
    ebcdic::to_ascii(&mut b);
    b
}

#[test]
fn account_record_from_sample_data() {
    let data = ebcdic_file("AWS.M2.CARDDEMO.ACCTDATA.PS");
    assert_eq!(data.len() % AccountRecord::LEN, 0);
    let a = AccountRecord::from_bytes(&data[..AccountRecord::LEN]);
    assert_eq!(a.acct_id_n(), Dec::from(1));
    assert_eq!(a.acct_active_status(), b"Y");
    assert_eq!(a.acct_curr_bal_n(), Dec::from_str("194.00").unwrap());
    assert_eq!(a.acct_credit_limit_n(), Dec::from_str("2020.00").unwrap());
    assert_eq!(a.acct_open_date(), b"2014-11-20");
}

#[test]
fn user_security_records() {
    let data = ebcdic_file("AWS.M2.CARDDEMO.USRSEC.PS");
    let u = SecUserData::from_bytes(&data[..80]);
    assert_eq!(u.sec_usr_id(), b"ADMIN001");
    assert_eq!(u.sec_usr_type(), b"A");
}

#[test]
fn value_clauses_and_initialize() {
    let h = ReportNameHeader::new();
    assert_eq!(&h.buf[..8], b"DALYREPT");
    let h2 = TransactionHeader2::new();
    assert!(h2.buf.iter().all(|&c| c == b'-'));
    let mut t = TranRecord::new();
    assert_eq!(t.tran_amt(), b"0000000000{");
    t.set_tran_amt_n(Dec::from_str("-1.5").unwrap());
    assert_eq!(t.tran_amt(), b"0000000015}");
    let mut d = TransactionDetailReport::new();
    d.set_tran_report_amt_n(Dec::from_str("-1234.5").unwrap());
    assert_eq!(d.tran_report_amt(), b"-      1,234.50");
}

#[test]
fn export_record_occurs_and_comp() {
    let mut e = ExportRecord::new();
    assert_eq!(ExportRecord::LEN, 500);
    e.set_exp_cust_addr_line(2, b"LINE TWO");
    assert_eq!(&e.exp_cust_addr_line(2)[..8], b"LINE TWO");
    e.set_export_sequence_num_n(Dec::from(258));
    assert_eq!(e.export_sequence_num(), &[0, 0, 1, 2]);
}
