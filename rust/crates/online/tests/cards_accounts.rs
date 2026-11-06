mod common;
use cics::Aid;
use common::*;

#[test]
fn card_list_view_update() {
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "3").enter();
    assert_eq!(t.map(), "CCRDLIA");
    println!("{}", t.dump());
    assert_eq!(t.field("INFOMSG"), "TYPE S FOR DETAIL, U TO UPDATE ANY RECORD");
    let card1 = t.field("CRDNUM1");
    assert_eq!(card1.len(), 16);
    t.key(Aid::Pf(7));
    assert_eq!(t.msg(), "NO PREVIOUS PAGES TO DISPLAY");
    t.key(Aid::Pf(8));
    assert_eq!(t.field("PAGENO"), "2");
    t.key(Aid::Pf(7));
    assert_eq!(t.field("CRDNUM1"), card1);
    t.typ("ACCTSID", "abc").enter();
    assert_eq!(t.msg(), "ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER");
    t.typ("ACCTSID", "").typ("CRDSEL1", "S").typ("CRDSEL2", "U").enter();
    assert_eq!(t.msg(), "PLEASE SELECT ONLY ONE RECORD TO VIEW OR UPDATE");
    t.typ("CRDSEL2", "").typ("CRDSEL1", "S").enter();
    assert_eq!(t.map(), "CCRDSLA");
    println!("{}", t.dump());
    assert_eq!(t.field("INFOMSG"), "   Displaying requested details");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "CCRDLIA");
    t.typ("CRDSEL1", "U").enter();
    assert_eq!(t.map(), "CCRDUPA");
    assert_eq!(t.field("INFOMSG"), "Details of selected card shown above");
    t.enter();
    assert_eq!(t.msg(), "No change detected with respect to values fetched.");
    t.typ("CRDSTCD", "X").enter();
    assert_eq!(t.msg(), "Card Active Status must be Y or N");
    t.typ("CRDSTCD", "N").enter();
    assert_eq!(t.field("INFOMSG"), "Changes validated.Press F5 to save");
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database");
    let rec = r.vsam.read("CARDDAT", card1.as_bytes()).unwrap();
    let card = cobol::copybooks::CardRecord::from_bytes(&rec);
    assert_eq!(card.card_active_status(), b"N");
    assert_eq!(card.card_cvv_cd(), b"000"); // KD-07
    t.enter(); // after commit, back to the list (called from the list)
    assert_eq!(t.map(), "CCRDLIA");
}

#[test]
fn account_view_and_update() {
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "1").enter();
    assert_eq!(t.map(), "CACTVWA");
    t.enter();
    assert_eq!(t.msg(), "No input received");
    t.typ("ACCTSID", "00000000001").enter();
    println!("{}", t.dump());
    assert_eq!(t.field("ACSTTUS"), "Y");
    assert!(t.field("ACURBAL").ends_with("194.00"), "{}", t.field("ACURBAL"));
    t.typ("ACCTSID", "99999999999").enter();
    assert!(t.msg().starts_with("Account:99999999999 not found in Cross ref file."), "{}", t.msg());
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A");
    // update
    t.typ("OPTION", "2").enter();
    assert_eq!(t.map(), "CACTUPA");
    assert_eq!(t.field("INFOMSG"), "Enter or update id of account to update");
    t.typ("ACCTSID", "00000000001").enter();
    println!("{}", t.dump());
    assert_eq!(t.field("INFOMSG"), "Update account details presented above.");
    t.enter();
    assert_eq!(t.msg(), "No change detected with respect to values fetched.");
    t.typ("ACSPFLG", "X").enter();
    // the sample customer's FICO score is outside 300..850 and is validated first
    assert_eq!(t.msg(), "FICO Score: should be between 300 and 850");
    // the random sample data also has invalid area codes and state/zip pairs: fix them all
    let fix = |t: &mut Term| {
        t.typ("ACSTFCO", "750").typ("ACSSTTE", "NY").typ("ACSZIPC", "10001")
            .typ("ACSPH1A", "212").typ("ACSPH1B", "555").typ("ACSPH1C", "0100")
            .typ("ACSPH2A", "212").typ("ACSPH2B", "555").typ("ACSPH2C", "0101");
    };
    fix(&mut t);
    t.typ("ACSPFLG", "X").enter();
    assert_eq!(t.msg(), "Primary Card Holder must be Y or N.");
    fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("ACRDLIM", "5000").enter();
    assert_eq!(t.msg(), "", "{}", t.msg());
    assert_eq!(t.field("INFOMSG"), "Changes validated.Press F5 to save");
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database");
    let a = cobol::copybooks::AccountRecord::from_bytes(&r.vsam.read("ACCTDAT", b"00000000001").unwrap());
    assert_eq!(a.acct_credit_limit_n(), cobol::Dec::from(5000));
}
