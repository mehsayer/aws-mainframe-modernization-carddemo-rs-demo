mod common;
use cics::Aid;
use common::*;

#[test]
fn user_admin_journey() {
    let r = region();
    let mut t = Term::signon(&r, "ADMIN001", "PASSWORD");
    t.typ("OPTION", "1").enter();
    assert_eq!(t.map(), "COUSR0A");
    println!("{}", t.dump());
    assert_eq!(t.field("USRID01"), "ADMIN001");
    assert_eq!(t.field("USRID10"), "USER0005");
    assert_eq!(t.field("PAGENUM"), "00000001");
    t.key(Aid::Pf(7));
    assert_eq!(t.msg(), "You are already at the top of the page...");
    // PF7 on page 1 sets NEXT-PAGE-YES (as in COUSR00C), so PF8 reads on and hits the end
    t.key(Aid::Pf(8));
    assert_eq!(t.msg(), "You have reached the bottom of the page...");
    t.key(Aid::Pf(8));
    assert_eq!(t.msg(), "You are already at the bottom of the page...");
    // that PF8 cleared the rows before reaching end of file (original behaviour): re-list
    assert_eq!(t.field("USRID01"), "");
    t.enter();
    // select U on row 6 -> update screen with the user pre-filled
    t.typ("SEL0006", "U").enter();
    assert_eq!(t.map(), "COUSR2A");
    assert_eq!(t.field("FNAME"), "LAWRENCE");
    assert_eq!(t.msg(), "Press PF5 key to save your updates ...");
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "Please modify to update ...");
    t.typ("FNAME", "LARRY").key(Aid::Pf(5));
    assert_eq!(t.msg(), "User USER0001 has been updated ...");
    t.key(Aid::Pf(3)); // saves again (no change) and returns to the list
    assert_eq!(t.map(), "COUSR0A");
    // add a user
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "2").enter();
    assert_eq!(t.map(), "COUSR1A");
    t.enter();
    assert_eq!(t.msg(), "First Name can NOT be empty...");
    t.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "secret").typ("USRTYPE", "U").enter();
    assert_eq!(t.msg(), "User USER0099 has been added ...");
    t.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "secret").typ("USRTYPE", "U").enter();
    assert_eq!(t.msg(), "User ID already exist...");
    // delete it
    t.key(Aid::Pf(3));
    t.typ("OPTION", "4").enter();
    assert_eq!(t.map(), "COUSR3A");
    t.typ("USRIDIN", "USER0099").enter();
    assert_eq!(t.field("LNAME"), "Lovelace");
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "User USER0099 has been deleted ...");
    t.typ("USRIDIN", "USER0099").enter();
    assert_eq!(t.msg(), "User ID NOT found...");
    // user list now has 11 users -> page 2 exists
    t.key(Aid::Pf(3));
    t.typ("OPTION", "1").enter();
    assert_eq!(t.field("USRID10"), "USER0005");
}

#[test]
fn search_beyond_last_user_abends_like_the_original() {
    let r = region();
    let mut t = Term::signon(&r, "ADMIN001", "PASSWORD");
    t.typ("OPTION", "1").enter();
    t.typ("USRIDIN", "ZZZZZZZZ").enter();
    assert!(t.s.screen.text.as_deref().unwrap_or("").contains("AEIP"));
}
