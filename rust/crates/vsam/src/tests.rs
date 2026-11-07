//! Unit tests of the VSAM layer: keyed read/write, browsing both ways, EBCDIC key order, rollback, and
//! the busy-store report.

use super::*;

fn rec(id: &str, acct: &str) -> Vec<u8> {
    // CARD-XREF-RECORD: card(16) cust(9) acct(11) filler(14)
    format!("{:<16}{:09}{:<11}{:14}", id, 1, acct, "").into_bytes()
}

fn store() -> Vsam {
    let v = Vsam::open_in_memory().unwrap();
    v.write("CCXREF", &rec("0000000000000003", "00000000002")).unwrap();
    v.write("CCXREF", &rec("0000000000000001", "00000000001")).unwrap();
    v.write("CCXREF", &rec("0000000000000002", "00000000001")).unwrap();
    v
}

#[test]
fn keyed_read_write() {
    let v = store();
    assert_eq!(&v.read("CCXREF", b"0000000000000002").unwrap()[..16], b"0000000000000002");
    assert_eq!(v.read("CCXREF", b"0000000000000009"), Err(Resp::NotFnd));
    assert_eq!(v.write("CCXREF", &rec("0000000000000002", "x")), Err(Resp::DupRec));
    // non-unique alternate path
    assert_eq!(v.read("CXACAIX", b"00000000001"), Err(Resp::DupKey));
    assert_eq!(&v.read("CXACAIX", b"00000000002").unwrap()[..16], b"0000000000000003");
    v.delete("CCXREF", b"0000000000000001").unwrap();
    assert_eq!(&v.read("CXACAIX", b"00000000001").unwrap()[..16], b"0000000000000002");
}

#[test]
fn browse_forward_backward() {
    let v = store();
    let mut b = v.start_browse("CCXREF", b"0000000000000002", true).unwrap();
    assert_eq!(&v.read_next(&mut b).unwrap()[..16], b"0000000000000002");
    assert_eq!(&v.read_next(&mut b).unwrap()[..16], b"0000000000000003");
    assert_eq!(v.read_next(&mut b), Err(Resp::EndFile));
    // READPREV straight after STARTBR returns the positioned record
    let mut b = v.start_browse("CCXREF", b"0000000000000002", true).unwrap();
    assert_eq!(&v.read_prev(&mut b).unwrap()[..16], b"0000000000000002");
    assert_eq!(&v.read_prev(&mut b).unwrap()[..16], b"0000000000000001");
    assert_eq!(v.read_prev(&mut b), Err(Resp::EndFile));
    // HIGH-VALUES then READPREV = last record
    let mut b = v.start_browse("CCXREF", &[0xFF; 16], true).unwrap();
    assert_eq!(&v.read_prev(&mut b).unwrap()[..16], b"0000000000000003");
    // GTEQ beyond the end
    assert_eq!(v.start_browse("CCXREF", b"9", true).err(), Some(Resp::NotFnd));
}

#[test]
fn ebcdic_key_order() {
    let v = Vsam::open_in_memory().unwrap();
    let mk = |k: &str| format!("{:<8}{:72}", k, "").into_bytes();
    v.write("USRSEC", &mk("1USER")).unwrap();
    v.write("USRSEC", &mk("ADMIN")).unwrap();
    // in EBCDIC, letters sort before digits
    assert_eq!(&v.unload("USRSEC").unwrap()[0][..5], b"ADMIN");
}

#[test]
fn transactions_roll_back() {
    let v = store();
    v.begin().unwrap();
    v.delete("CCXREF", b"0000000000000003").unwrap();
    v.rollback().unwrap();
    assert!(v.read("CCXREF", b"0000000000000003").is_ok());
}

#[test]
fn failed_operation_leaves_no_partial_index_rows() {
    // simulates a REWRITE/WRITE that fails after an alternate-index row was already changed
    let v = store();
    let r: VResult<()> = v.atomic(|| {
        v.conn.execute("DELETE FROM \"a_CARDXREF_ACCTID\"", []).map_err(|_| Resp::IoErr)?;
        Err(Resp::DupRec)
    });
    assert_eq!(r, Err(Resp::DupRec));
    let n: i64 = v.conn.query_row("SELECT count(*) FROM \"a_CARDXREF_ACCTID\"", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 3, "index rows restored");
    assert_eq!(v.read_ex("CXACAIX", b"00000000002").unwrap().0[..16], b"0000000000000003"[..]);
}

#[test]
fn begin_reports_a_busy_store() {
    let dir = std::env::temp_dir().join(format!("vsam-busy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = Vsam::open(dir.join("db")).unwrap();
    let b = Vsam::open(dir.join("db")).unwrap();
    b.set_busy_timeout(std::time::Duration::from_millis(50)).unwrap();
    a.begin().unwrap();
    assert!(b.begin().is_err(), "second unit of work must fail, not run in autocommit");
    assert!(!b.in_unit_of_work());
    a.commit().unwrap();
    assert!(b.begin().is_ok());
    b.rollback().unwrap();
}
