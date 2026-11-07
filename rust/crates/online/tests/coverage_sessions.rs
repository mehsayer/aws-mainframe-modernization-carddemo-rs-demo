//! Sessions for the paragraphs of the original programs that no other scripted session entered
//! (`difftest/coverage.py`): card update opened from the menu, transaction add by card number, the PF3 and
//! PF4 keys of the add, report, payment and user screens, and paging back in the user list. Like the other
//! test files they run on the Rust programs and on the original COBOL programs, and `difftest/online.py`
//! compares every screen; the assertions here only check that each session gets where it means to.
mod common;
use cics::Aid;
use common::*;

/// The card number and account of one card of the sample data (cross reference).
const CARD: &str = "0500024453765740";
const ACCT: &str = "00000000050";

#[test]
fn card_update_opened_from_the_menu() {
    // COCRDUPC 1210-EDIT-ACCOUNT / 1220-EDIT-CARD: the search fields typed on the update screen itself
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "5").enter();
    assert_eq!(t.map(), "CCRDUPA");
    t.enter(); // nothing typed
    t.typ("ACCTSID", "abc").typ("CARDSID", "xyz").enter(); // not numeric
    t.typ("ACCTSID", ACCT).typ("CARDSID", CARD).enter(); // the card's details
    assert_eq!(t.map(), "CCRDUPA");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A");
}

#[test]
fn transaction_add_by_card_number_and_its_keys() {
    // COTRN02C READ-CCXREF-FILE (card instead of account), CLEAR-CURRENT-SCREEN (PF4), RETURN-TO-PREV-SCREEN (PF3)
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "8").enter();
    assert_eq!(t.map(), "COTRN2A");
    t.typ("CARDNIN", CARD).enter();
    t.key(Aid::Pf(4));
    t.typ("CARDNIN", "9999999999999999").enter(); // a card that is not in the cross reference
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A");
}

#[test]
fn report_and_bill_payment_keys() {
    // CORPT00C and COBIL00C RETURN-TO-PREV-SCREEN (PF3), COBIL00C CLEAR-CURRENT-SCREEN (PF4)
    let r = region();
    let mut t = Term::signon(&r, "USER0001", "PASSWORD");
    t.typ("OPTION", "9").enter();
    assert_eq!(t.map(), "CORPT0A");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A");
    t.typ("OPTION", "10").enter();
    assert_eq!(t.map(), "COBIL0A");
    t.typ("ACTIDIN", ACCT).enter();
    t.key(Aid::Pf(4));
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A");
}

#[test]
fn user_list_paging_back_and_user_screen_keys() {
    // COUSR00C PROCESS-PAGE-BACKWARD / READPREV-USER-SEC-FILE (a second page needs an 11th user),
    // COUSR02C CLEAR-CURRENT-SCREEN / INITIALIZE-ALL-FIELDS and COUSR03C CLEAR-CURRENT-SCREEN (PF4)
    let r = region();
    let mut t = Term::signon(&r, "ADMIN001", "PASSWORD");
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "2").enter();
    assert_eq!(t.map(), "COUSR1A");
    t.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "PASSWORD").typ("USRTYPE", "U");
    t.enter();
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "1").enter();
    assert_eq!(t.map(), "COUSR0A");
    t.key(Aid::Pf(8)); // the 11th user on page 2
    t.key(Aid::Pf(7)); // back to page 1
    t.key(Aid::Pf(7)); // already at the top
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "3").enter();
    assert_eq!(t.map(), "COUSR2A");
    t.typ("USRIDIN", "USER0099").enter();
    t.key(Aid::Pf(4));
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "4").enter();
    assert_eq!(t.map(), "COUSR3A");
    t.typ("USRIDIN", "USER0099").enter();
    t.key(Aid::Pf(4));
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COADM1A");
}
