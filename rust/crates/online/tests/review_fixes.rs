//! Regression tests for the code-review findings (storage, layouts, bill payment, units of work).
mod common;
use cics::Aid;
use cobol::copybooks::*;
use common::*;
use std::path::PathBuf;

fn fix_account(t: &mut Term) {
    t.typ("ACSTFCO", "750").typ("ACSSTTE", "NY").typ("ACSZIPC", "10001")
        .typ("ACSPH1A", "212").typ("ACSPH1B", "555").typ("ACSPH1C", "0100")
        .typ("ACSPH2A", "212").typ("ACSPH2B", "555").typ("ACSPH2C", "0101");
}

/// KD-13: COACTUPC rewrites the account through ACCT-UPDATE-RECORD, which has no ADDR-ZIP field.
#[test]
fn account_update_uses_the_update_record_layout() {
    let r = region();
    let before = AccountRecord::from_bytes(&r.vsam.read("ACCTDAT", b"00000000001").unwrap());
    // the sample data already holds its group value in the ADDR-ZIP slot (offset 102)
    assert_eq!(before.acct_addr_zip(), b"A000000000");
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "2").enter();
    t.typ("ACCTSID", "00000000001").enter();
    fix_account(&mut t);
    t.typ("ACSPFLG", "Y").typ("ACRDLIM", "5000").typ("AADDGRP", "GOLD").enter();
    assert_eq!(t.field("INFOMSG"), "Changes validated.Press F5 to save");
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database");
    let raw = r.vsam.read("ACCTDAT", b"00000000001").unwrap();
    // the group id lands in the ADDR-ZIP slot; the master's group id and the rest are X'00'
    assert_eq!(&raw[102..112], b"GOLD      ", "group id written at offset 102");
    assert!(raw[112..300].iter().all(|&b| b == 0), "ACCT-GROUP-ID and FILLER are binary zeros");
    let cust = r.vsam.read("CUSTDAT", b"000000001").unwrap();
    assert!(cust[332..500].iter().all(|&b| b == 0), "customer FILLER X(168) is binary zeros");
    // the account view now shows an empty group (as the original would)
    t.enter();
    t.key(Aid::Pf(3));
    t.typ("OPTION", "1").enter();
    t.typ("ACCTSID", "00000000001").enter();
    assert_eq!(t.field("AADDGRP"), "");
}

#[test]
fn card_update_and_user_add_leave_filler_as_binary_zeros() {
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "3").enter();
    let card = t.field("CRDNUM1");
    t.typ("CRDSEL1", "U").enter();
    t.typ("CRDSTCD", "N").enter();
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database");
    let raw = r.vsam.read("CARDDAT", card.as_bytes()).unwrap();
    assert!(raw[91..150].iter().all(|&b| b == 0), "CARD-UPDATE-RECORD FILLER X(59): {:?}", &raw[91..150]);

    let mut a = Term::signon(&r, "ADMIN001", "PASSWORD");
    a.typ("OPTION", "2").enter();
    a.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "secret").typ("USRTYPE", "U").enter();
    assert_eq!(a.msg(), "User USER0099 has been added ...");
    let raw = r.vsam.read("USRSEC", b"USER0099").unwrap();
    assert!(raw[57..80].iter().all(|&b| b == 0), "SEC-USR-FILLER X(23)");
}

fn pay(r: &cics::Region, acct: &str) -> String {
    let mut t = Term::signon(r, "USER0001", "PASSWORD");
    t.typ("OPTION", "10").enter();
    t.typ("ACTIDIN", acct).typ("CONFIRM", "Y").enter();
    t.msg()
}
fn last_tran(r: &cics::Region) -> TranRecord {
    TranRecord::from_bytes(r.vsam.unload("TRANSACT").unwrap().last().unwrap())
}

/// DUPKEY on CXACAIX: CICS still returns the first record, so the payment carries its card number.
#[test]
fn bill_payment_with_two_cards_uses_the_first_card() {
    let r = region();
    let mut x = CardXrefRecord::from_bytes(&r.vsam.read("CXACAIX", b"00000000001").unwrap());
    x.set_xref_card_num(b"0000000000000000"); // sorts before the account's own card
    r.vsam.write("CCXREF", &x.buf).unwrap();
    let m = pay(&r, "00000000001");
    assert!(m.starts_with("Payment successful."), "{m}");
    assert_eq!(last_tran(&r).tran_card_num(), b"0000000000000000");
}

/// NOTFND on CXACAIX: CARD-XREF-RECORD keeps its initial binary zeros; the payment is still written.
#[test]
fn bill_payment_without_xref_writes_low_values_card_number() {
    let r = region();
    let x = r.vsam.read("CXACAIX", b"00000000001").unwrap();
    r.vsam.delete("CCXREF", &x[..16]).unwrap();
    let m = pay(&r, "00000000001");
    assert!(m.starts_with("Payment successful."), "{m}");
    assert_eq!(last_tran(&r).tran_card_num(), &[0u8; 16]);
}

/// Only the X'00' initialisation record in TRANSACT: the new id follows it, and INITIALIZE keeps the
/// FILLER that READPREV read.
#[test]
fn bill_payment_transaction_id_and_filler() {
    let r = region();
    let init = r.vsam.unload("TRANSACT").unwrap();
    assert_eq!(init.len(), 1);
    pay(&r, "00000000001");
    let t = last_tran(&r);
    assert_eq!(t.tran_id(), b"0000000000000001");
    assert_eq!(&t.buf[330..350], &init[0][330..350], "FILLER kept from the READPREV record");
}

/// A terminal task that cannot open its unit of work is not run at all.
#[test]
fn busy_store_is_reported_and_nothing_is_written() {
    let dir = std::env::temp_dir().join(format!("carddemo-busy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("datasets")).unwrap();
    let db = dir.join("carddemo.db");
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cobol/app/data/EBCDIC");
    let holder = vsam::Vsam::open(&db).unwrap();
    batch::repro::reset(&holder, &seed).unwrap();
    let v = vsam::Vsam::open(&db).unwrap();
    v.set_busy_timeout(std::time::Duration::from_millis(50)).unwrap();
    let r = online::region(v);
    holder.begin().unwrap(); // another terminal or job holds the store
    let t = Term::new(&r);
    let txt = t.s.screen.text.clone().unwrap_or_default();
    assert!(txt.contains("data store is busy") && txt.contains("No updates were made"), "{txt}");

    let env = batch::jobs::Env { vsam: &r.vsam, root: dir.clone(), seed, echo: false };
    let res = batch::jobs::run_job(&env, "POSTTRAN", &batch::jobs::JobOpts::default());
    assert!(matches!(res, Err((_, batch::ctx::Abend(batch::jobs::UOW_ABEND)))), "batch step must not run unprotected");
    holder.rollback().unwrap();
    let t = Term::new(&r);
    assert_eq!(t.map(), "COSGN0A", "works again once the store is free");
}
