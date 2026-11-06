mod common;
use cics::Aid;
use common::*;

#[test]
fn signon_flows() {
    let r = region();
    let mut t = Term::new(&r);
    assert_eq!(t.map(), "COSGN0A");
    println!("{}", t.dump());
    t.enter();
    assert_eq!(t.msg(), "Please enter User ID ...");
    // the password field is FSET with INITIAL '________': left untouched it is sent
    t.typ("USERID", "user0001").enter();
    assert_eq!(t.msg(), "Wrong Password. Try again ...");
    t.typ("USERID", "user0001").typ("PASSWD", "").enter();
    assert_eq!(t.msg(), "Please enter Password ...");
    t.typ("USERID", "nobody").typ("PASSWD", "x").enter();
    assert_eq!(t.msg(), "User not found. Try again ...");
    t.typ("USERID", "user0001").typ("PASSWD", "wrong").enter();
    assert_eq!(t.msg(), "Wrong Password. Try again ...");
    t.typ("USERID", "user0001").typ("PASSWD", "password").enter();
    assert_eq!(t.map(), "COMEN1A");
    assert_eq!(t.field("OPTN001"), "01. Account View");
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "Invalid key pressed. Please see below...");
    t.typ("OPTION", "99").enter();
    assert_eq!(t.msg(), "Please enter a valid option number...");
    t.typ("OPTION", "11").enter();
    assert_eq!(t.msg(), "This option Pending is not installed...");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COSGN0A");
    t.key(Aid::Pf(3));
    assert!(t.s.screen.text.as_deref().unwrap().starts_with("Thank you for using CardDemo"));
}

#[test]
fn admin_menu() {
    let r = region();
    let mut t = Term::signon(&r, "ADMIN001", "PASSWORD");
    assert_eq!(t.map(), "COADM1A");
    t.typ("OPTION", "5").enter();
    assert_eq!(t.msg(), "This option is not installed ...");
}
