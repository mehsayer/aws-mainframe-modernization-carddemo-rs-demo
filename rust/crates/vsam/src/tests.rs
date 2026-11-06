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
    v.begin();
    v.delete("CCXREF", b"0000000000000003").unwrap();
    v.rollback();
    assert!(v.read("CCXREF", b"0000000000000003").is_ok());
}
