mod common;
use cics::Aid;
use common::*;

fn post_daily(r: &cics::Region) {
    // give the transaction master some content: run the posting program
    let seed = batch::repro::sample_seed_dir();
    let root = std::env::temp_dir().join(format!("carddemo-online-{}", std::process::id()));
    let env = batch::jobs::Env { vsam: &r.vsam, root, seed, echo: false };
    batch::jobs::run_job(&env, "POSTTRAN", &batch::jobs::JobOpts::default()).unwrap();
}

#[test]
fn transaction_list_view_add() {
    let r = region();
    post_daily(&r);
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "6").enter();
    assert_eq!(t.map(), "COTRN0A");
    println!("{}", t.dump());
    assert_eq!(t.field("PAGENUM"), "00000001");
    let first = t.field("TRNID01");
    t.key(Aid::Pf(8));
    assert_eq!(t.field("PAGENUM"), "00000002");
    t.key(Aid::Pf(7));
    assert_eq!(t.field("TRNID01"), first);
    t.typ("TRNIDIN", "abc").enter();
    assert_eq!(t.msg(), "Tran ID must be Numeric ...");
    // row 1 is the blank initialisation record loaded by job TRANFILE (DALYTRAN.PS.INIT)
    assert_eq!(first, "");
    let second = t.field("TRNID02");
    t.typ("SEL0002", "S").enter();
    assert_eq!(t.map(), "COTRN1A");
    assert_eq!(t.field("TRNID"), second);
    println!("{}", t.dump());
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COTRN0A");
    t.key(Aid::Pf(3));
    // transaction add
    t.typ("OPTION", "8").enter();
    assert_eq!(t.map(), "COTRN2A");
    t.enter();
    assert_eq!(t.msg(), "Account or Card Number must be entered...");
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.msg(), "Type CD can NOT be empty...");
    assert_eq!(t.field("CARDNIN").len(), 16);
    // PF5 copies the last transaction, then asks for confirmation
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "Confirm to add this transaction...");
    t.typ("CONFIRM", "Y").enter();
    assert!(t.msg().starts_with("Transaction added successfully.  Your Tran ID is "), "{}", t.msg());
    t.typ("ACTIDIN", "00000000001").typ("TTYPCD", "01").typ("TCATCD", "0001").typ("TRNSRC", "POS TERM").typ("TDESC", "Coffee")
        .typ("TRNAMT", "+00000004.50").typ("TORIGDT", "2022-02-30").typ("TPROCDT", "2022-02-28").typ("MID", "000000001")
        .typ("MNAME", "Cafe").typ("MCITY", "Seattle").typ("MZIP", "98101").typ("CONFIRM", "Y").enter();
    assert_eq!(t.msg(), "Orig Date - Not a valid date...");
}

#[test]
fn bill_payment() {
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "10").enter();
    assert_eq!(t.map(), "COBIL0A");
    t.enter();
    assert_eq!(t.msg(), "Acct ID can NOT be empty...");
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.msg(), "Confirm to make a bill payment...");
    assert_eq!(t.field("CURBAL"), "+0000000194.00");
    t.typ("CONFIRM", "Y").enter();
    assert!(t.msg().starts_with("Payment successful.  Your Transaction ID is "), "{}", t.msg());
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.msg(), "You have nothing to pay...");
}

#[test]
fn report_submission_writes_jcl_to_internal_reader() {
    use std::sync::{Arc, Mutex};
    let mut r = region();
    let decks = Arc::new(Mutex::new(Vec::new()));
    let d2 = decks.clone();
    r.on_job = Some(Box::new(move |deck: &[Vec<u8>]| d2.lock().unwrap().push(deck.to_vec())));
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "9").enter();
    assert_eq!(t.map(), "CORPT0A");
    t.enter();
    assert_eq!(t.msg(), "Select a report type to print report...");
    t.typ("MONTHLY", "X").enter();
    assert_eq!(t.msg(), "Please confirm to print the Monthly report...");
    t.typ("MONTHLY", "X").typ("CONFIRM", "Y").enter();
    assert_eq!(t.msg(), "Monthly report submitted for printing ...");
    let d = decks.lock().unwrap();
    let dateparm = cobol::text::s(&d[0][14]);
    assert_eq!(dateparm.trim_end(), "2022-07-01 2022-07-31");
    t.typ("CUSTOM", "X").typ("SDTMM", "13").typ("SDTDD", "1").typ("SDTYYYY", "2022").typ("EDTMM", "1").typ("EDTDD", "1").typ("EDTYYYY", "2022").enter();
    assert_eq!(t.msg(), "Start Date - Not a valid Month...");
}
