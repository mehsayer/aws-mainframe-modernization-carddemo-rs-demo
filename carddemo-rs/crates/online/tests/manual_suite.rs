//! Scripted execution of the manual test suite (docs/tests/cases.py, docs/manual-test-suite.pdf).
//! One test per test case, named after its id (tc_sgn_01 = TC-SGN-01); each step's expected result
//! is asserted with the step number. `docs/tests/execute.sh` runs this file and fills the execution log.
mod common;
use batch::jobs::{run_job, Env, JobOpts};
use cics::{Aid, Color, Region};
use common::*;
use std::path::PathBuf;

fn seed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../app/data/EBCDIC")
}

/// The terminal UI's rule: after the session ended (plain text screen, no next transaction),
/// Enter starts CC00 again.
fn tui_enter(t: &mut Term) {
    if t.s.next_transid.is_none() && t.s.screen.text.is_some() {
        t.r.run(&mut t.s, Some("CC00"), Aid::Enter);
    } else {
        t.enter();
    }
}

fn color(t: &Term, f: &str) -> Color {
    t.s.screen.field(f).map(|x| x.color).unwrap_or_default()
}
fn cursor_in(t: &Term, f: &str) -> bool {
    let (r, c) = t.s.screen.cursor;
    t.s.screen.field(f).is_some_and(|x| x.row == r && c >= x.col && c <= x.col + x.len)
}
fn user(r: &Region) -> Term<'_> {
    let t = Term::signon(r, "USER0001", "PASSWORD");
    assert_eq!(t.map(), "COMEN1A", "precondition: main menu");
    t
}
fn admin(r: &Region) -> Term<'_> {
    let t = Term::signon(r, "ADMIN001", "PASSWORD");
    assert_eq!(t.map(), "COADM1A", "precondition: admin menu");
    t
}
fn job_env(r: &Region, name: &str) -> (PathBuf, JobOpts) {
    let root = std::env::temp_dir().join(format!("carddemo-suite-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("datasets")).unwrap();
    let _ = r;
    (root, JobOpts::default())
}
fn run(r: &Region, root: &std::path::Path, o: &JobOpts, job: &str) -> Vec<batch::jobs::StepLog> {
    let env = Env { vsam: &r.vsam, root: root.to_path_buf(), seed: seed(), echo: false };
    run_job(&env, job, o).unwrap_or_else(|(_, a)| panic!("{job} abended: {a}"))
}

// ------------------------------------------------------------------ sign-on

#[test]
fn tc_sgn_01() {
    let r = region();
    let t = Term::new(&r);
    let d = t.dump();
    assert!(d.contains("CC00") && d.contains("COSGN00C") && d.contains("CARDDEMO"), "step 1: header\n{d}");
    assert!(d.contains("07/18/22"), "step 1: date mm/dd/yy\n{d}");
    assert!(cursor_in(&t, "USERID"), "step 1: cursor in User ID");
    assert_eq!(t.field("PASSWD"), "________", "step 1: password underscores");
}

#[test]
fn tc_sgn_02() {
    let r = region();
    let mut t = Term::new(&r);
    t.enter();
    assert_eq!(t.msg(), "Please enter User ID ...", "step 1");
    assert!(cursor_in(&t, "USERID"), "step 1: cursor");
    t.typ("USERID", "USER0001").typ("PASSWD", "").enter();
    assert_eq!(t.msg(), "Please enter Password ...", "step 2");
    assert!(cursor_in(&t, "PASSWD"), "step 2: cursor");
    let mut t = Term::new(&r);
    t.typ("USERID", "USER0001").enter();
    assert_eq!(t.msg(), "Wrong Password. Try again ...", "step 3");
}

#[test]
fn tc_sgn_03() {
    let r = region();
    let mut t = Term::new(&r);
    t.typ("USERID", "NOBODY").typ("PASSWD", "X").enter();
    assert_eq!(t.msg(), "User not found. Try again ...", "step 1");
    t.typ("USERID", "USER0001").typ("PASSWD", "WRONG").enter();
    assert_eq!(t.msg(), "Wrong Password. Try again ...", "step 2");
    assert!(cursor_in(&t, "PASSWD"), "step 2: cursor");
}

#[test]
fn tc_sgn_04() {
    let r = region();
    let mut t = Term::signon(&r, "user0001", "password");
    assert_eq!(t.map(), "COMEN1A", "step 1");
    assert!(t.dump().contains("CM00") && t.dump().contains("COMEN01C"), "step 1: header");
    t.key(Aid::Pf(3));
    t.typ("USERID", "admin001").typ("PASSWD", "password").enter();
    assert_eq!(t.map(), "COADM1A", "step 2");
    assert!(t.dump().contains("CA00") && t.dump().contains("COADM01C"), "step 2: header");
}

#[test]
fn tc_sgn_05() {
    let r = region();
    let mut t = Term::new(&r);
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "Invalid key pressed. Please see below...", "step 1");
    t.key(Aid::Pf(3));
    assert!(t.s.screen.text.as_deref().unwrap_or("").starts_with("Thank you for using CardDemo application..."), "step 2");
    tui_enter(&mut t);
    assert_eq!(t.map(), "COSGN0A", "step 3");
}

// ------------------------------------------------------------------ menus

#[test]
fn tc_men_01() {
    let r = region();
    let t = user(&r);
    assert_eq!(t.field("OPTN001"), "01. Account View", "step 1");
    assert_eq!(t.field("OPTN011"), "11. Pending Authorization View", "step 1");
}

#[test]
fn tc_men_02() {
    let r = region();
    let mut t = user(&r);
    for (i, o) in ["99", "0", "ab"].iter().enumerate() {
        t.typ("OPTION", o).enter();
        assert_eq!(t.msg(), "Please enter a valid option number...", "step {}", i + 1);
    }
    t.key(Aid::Pf(7));
    assert_eq!(t.msg(), "Invalid key pressed. Please see below...", "step 4");
}

#[test]
fn tc_men_03() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "11").enter();
    assert_eq!(t.msg(), "This option Pending is not installed...", "step 1");
    assert_eq!(color(&t, "ERRMSG"), Color::Red, "step 1: red");
}

#[test]
fn tc_men_04() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "1").enter();
    assert_eq!(t.map(), "CACTVWA", "step 1");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A", "step 2");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COSGN0A", "step 3");
}

#[test]
fn tc_adm_01() {
    let r = region();
    let mut t = admin(&r);
    let d = t.dump();
    assert!(d.contains("1. User List (Security)") && d.contains("4. User Delete (Security)") && d.contains("(Db2)"), "step 1\n{d}");
    t.typ("OPTION", "5").enter();
    assert_eq!(t.msg(), "This option is not installed ...", "step 2");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 2: green");
    t.typ("OPTION", "7").enter();
    assert_eq!(t.msg(), "Please enter a valid option number...", "step 3");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COSGN0A", "step 4");
}

// ------------------------------------------------------------------ users

fn user_list(r: &Region) -> Term<'_> {
    let mut t = admin(r);
    t.typ("OPTION", "1").enter();
    assert_eq!(t.map(), "COUSR0A");
    t
}

#[test]
fn tc_usr_01() {
    let r = region();
    let mut t = user_list(&r);
    assert_eq!(t.field("PAGENUM"), "00000001", "step 1");
    assert_eq!((t.field("USRID01").as_str(), t.field("USRID05").as_str()), ("ADMIN001", "ADMIN005"), "step 1");
    assert_eq!((t.field("USRID06").as_str(), t.field("USRID10").as_str()), ("USER0001", "USER0005"), "step 1");
    t.key(Aid::Pf(7));
    assert_eq!(t.msg(), "You are already at the top of the page...", "step 2");
    t.key(Aid::Pf(8));
    assert_eq!(t.msg(), "You have reached the bottom of the page...", "step 3");
    assert_eq!(t.field("USRID01"), "", "step 3: rows blank (KD-11)");
    t.key(Aid::Pf(8));
    assert_eq!(t.msg(), "You are already at the bottom of the page...", "step 4");
    t.enter();
    assert_eq!(t.field("USRID01"), "ADMIN001", "step 5");
}

#[test]
fn tc_usr_02() {
    let r = region();
    let mut t = user_list(&r);
    t.typ("USRIDIN", "USER0003").enter();
    assert_eq!(t.field("USRID01"), "USER0003", "step 1");
}

#[test]
fn tc_usr_03() {
    let r = region();
    let mut t = user_list(&r);
    t.typ("USRIDIN", "ZZZZZZZZ").enter();
    let txt = t.s.screen.text.clone().unwrap_or_default();
    assert!(txt.contains("Transaction CU00 failed with abend AEIP"), "step 1: {txt}");
    tui_enter(&mut t);
    assert_eq!(t.map(), "COSGN0A", "step 1: Enter returns to sign-on");
}

#[test]
fn tc_usr_04() {
    let r = region();
    let mut t = user_list(&r);
    t.typ("SEL0006", "X").enter();
    // with exactly 10 users the read-ahead after the page hits end of file, and its message replaces
    // 'Invalid selection...' (COUSR00C sets WS-MESSAGE, then PROCESS-PAGE-FORWARD overwrites it)
    assert_eq!(t.msg(), "You have reached the bottom of the page...", "step 1");
    t.typ("SEL0006", "u").enter();
    assert_eq!(t.map(), "COUSR2A", "step 2");
    assert_eq!(t.field("USRIDIN"), "USER0001", "step 2: pre-filled");
    assert_eq!(t.msg(), "Press PF5 key to save your updates ...", "step 2");
}

#[test]
fn tc_usr_05() {
    let r = region();
    let mut t = admin(&r);
    t.typ("OPTION", "2").enter();
    t.enter();
    assert_eq!(t.msg(), "First Name can NOT be empty...", "step 1");
    let fill = |t: &mut Term| {
        t.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "secret").typ("USRTYPE", "U").enter();
    };
    fill(&mut t);
    assert_eq!(t.msg(), "User USER0099 has been added ...", "step 2");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 2: green");
    assert_eq!(t.field("FNAME"), "", "step 2: fields cleared");
    fill(&mut t);
    assert_eq!(t.msg(), "User ID already exist...", "step 3");
    t.key(Aid::Pf(4));
    assert_eq!((t.field("FNAME").as_str(), t.field("USERID").as_str()), ("", ""), "step 4");
}

#[test]
fn tc_usr_06() {
    let r = region();
    let mut t = admin(&r);
    t.typ("OPTION", "3").enter();
    t.typ("USRIDIN", "USER0002").enter();
    assert_eq!(t.msg(), "Press PF5 key to save your updates ...", "step 1");
    assert_ne!(t.field("FNAME"), "", "step 1: fields shown");
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "Please modify to update ...", "step 2");
    assert_eq!(color(&t, "ERRMSG"), Color::Red, "step 2: red");
    t.typ("FNAME", "CHANGED").key(Aid::Pf(5));
    assert_eq!(t.msg(), "User USER0002 has been updated ...", "step 3");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 3: green");
    t.typ("USRIDIN", "NOBODY99").enter();
    assert_eq!(t.msg(), "User ID NOT found...", "step 4");
}

#[test]
fn tc_usr_07() {
    let r = region();
    let mut t = admin(&r);
    t.typ("OPTION", "2").enter();
    t.typ("FNAME", "Ada").typ("LNAME", "Lovelace").typ("USERID", "USER0099").typ("PASSWD", "secret").typ("USRTYPE", "U").enter();
    assert_eq!(t.msg(), "User USER0099 has been added ...", "precondition");
    t.key(Aid::Pf(3));
    t.typ("OPTION", "4").enter();
    t.typ("USRIDIN", "USER0099").enter();
    assert_eq!((t.field("FNAME").as_str(), t.field("LNAME").as_str(), t.field("USRTYPE").as_str()), ("Ada", "Lovelace", "U"), "step 1");
    assert!(!t.dump().contains("secret"), "step 1: no password shown");
    assert_eq!(t.msg(), "Press PF5 key to delete this user ...", "step 1");
    t.key(Aid::Pf(5));
    assert_eq!(t.msg(), "User USER0099 has been deleted ...", "step 2");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 2: green");
    t.typ("USRIDIN", "USER0099").enter();
    assert_eq!(t.msg(), "User ID NOT found...", "step 3");
}

// ------------------------------------------------------------------ accounts

#[test]
fn tc_acv_01() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "1").enter();
    assert_eq!(t.field("INFOMSG"), "Enter or update id of account to display", "step 1");
    let row1 = t.s.screen.render()[0].clone();
    assert!(row1.trim_start().starts_with('W') && !row1.contains("Tran:"), "step 1: only 'W' of the header (map quirk): {row1}");
    t.enter();
    assert_eq!(t.msg(), "Account number not provided", "step 2");
    assert_eq!(t.field("ACCTSID"), "*", "step 2: '*' in the account field");
    assert_eq!(color(&t, "ACCTSID"), Color::Red, "step 2: red");
    t.typ("ACCTSID", "abc").enter();
    assert_eq!(t.msg(), "Account Filter must be a non-zero 11 digit number", "step 3");
}

#[test]
fn tc_acv_02() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "1").enter();
    t.typ("ACCTSID", "00000000001").enter();
    assert_eq!(t.field("ACSTTUS"), "Y", "step 1: status");
    assert_eq!(t.field("ACURBAL"), "+        194.00", "step 1: balance +ZZZ,ZZZ,ZZZ.99");
    let ssn = t.field("ACSTSSN");
    assert!(ssn.len() == 11 && &ssn[3..4] == "-" && &ssn[6..7] == "-", "step 1: SSN 999-99-9999: {ssn}");
    assert_ne!(t.field("ACSFNAM"), "", "step 1: customer name");
    t.typ("ACCTSID", "99999999999").enter();
    assert_eq!(t.msg(), "Account:99999999999 not found in Cross ref file.  Resp:000000013  Reas:0000", "step 2");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "COMEN1A", "step 3");
}

fn acu_fix(t: &mut Term) {
    t.typ("ACSTFCO", "750").typ("ACSSTTE", "NY").typ("ACSZIPC", "10001")
        .typ("ACSPH1A", "212").typ("ACSPH1B", "555").typ("ACSPH1C", "0100")
        .typ("ACSPH2A", "212").typ("ACSPH2B", "555").typ("ACSPH2C", "0101");
}
fn acu_fetched(r: &Region) -> Term<'_> {
    let mut t = user(r);
    t.typ("OPTION", "2").enter();
    t.typ("ACCTSID", "00000000001").enter();
    assert_eq!(t.field("INFOMSG"), "Update account details presented above.", "precondition");
    t
}

#[test]
fn tc_acu_01() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "2").enter();
    assert_eq!(t.field("INFOMSG"), "Enter or update id of account to update", "step 1");
    t.typ("ACCTSID", "0").enter();
    assert_eq!(t.msg(), "Account Number if supplied must be a 11 digit Non-Zero Number", "step 2");
    t.typ("ACCTSID", "00000000001").enter();
    assert_eq!(t.field("INFOMSG"), "Update account details presented above.", "step 3");
    t.enter();
    assert_eq!(t.msg(), "No change detected with respect to values fetched.", "step 4");
}

#[test]
fn tc_acu_02() {
    let r = region();
    let mut t = acu_fetched(&r);
    t.typ("ACSPFLG", "X").enter();
    assert_eq!(t.msg(), "FICO Score: should be between 300 and 850", "step 1");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "X").enter();
    assert_eq!(t.msg(), "Primary Card Holder must be Y or N.", "step 2");
    assert_eq!(color(&t, "ACSPFLG"), Color::Red, "step 2: red");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("OPNMON", "13").enter();
    assert_eq!(t.msg(), "Open Date: Month must be a number between 1 and 12.", "step 3");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("OPNMON", "01").typ("DOBYEAR", "2099").enter();
    assert_eq!(t.msg(), "Date of Birth:cannot be in the future", "step 4 (message ends with a space; trimmed here)");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("OPNMON", "01").typ("DOBYEAR", "1980").typ("ACTSSN1", "666").enter();
    assert_eq!(t.msg(), "SSN: First 3 chars: should not be 000, 666, or between 900 and 999", "step 5");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("OPNMON", "01").typ("DOBYEAR", "1980").typ("ACTSSN1", "123").typ("ACSPH1A", "123").enter();
    assert_eq!(t.msg(), "Phone Number 1: Not valid North America general purpose area code", "step 6");
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("OPNMON", "01").typ("DOBYEAR", "1980").typ("ACTSSN1", "123").typ("ACSZIPC", "90210").enter();
    assert_eq!(t.msg(), "Invalid zip code for state", "step 7");
}

#[test]
fn tc_acu_03() {
    let r = region();
    let mut t = acu_fetched(&r);
    acu_fix(&mut t);
    t.typ("ACSPFLG", "Y").typ("ACRDLIM", "5000").enter();
    assert_eq!(t.field("INFOMSG"), "Changes validated.Press F5 to save", "step 1");
    let prot = t.s.screen.field("ACRDLIM").map(|f| f.attr.protected).unwrap_or(false);
    assert!(prot, "step 1: fields protected");
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database", "step 2");
    t.enter();
    assert_eq!(t.field("INFOMSG"), "Enter or update id of account to update", "step 3: empty search screen");
    t.key(Aid::Pf(3));
    t.typ("OPTION", "1").enter();
    t.typ("ACCTSID", "00000000001").enter();
    assert_eq!(t.field("ACRDLIM"), "+      5,000.00", "step 3: credit limit");
}

// ------------------------------------------------------------------ cards

fn card_list(r: &Region) -> Term<'_> {
    let mut t = user(r);
    t.typ("OPTION", "3").enter();
    assert_eq!(t.map(), "CCRDLIA");
    t
}

#[test]
fn tc_cdl_01() {
    let r = region();
    let mut t = card_list(&r);
    let cards: Vec<String> = (1..=7).map(|i| t.field(&format!("CRDNUM{i}"))).collect();
    assert!(cards.iter().all(|c| c.len() == 16), "step 1: 7 cards {cards:?}");
    assert!(cards.windows(2).all(|w| w[0] < w[1]), "step 1: card-number order");
    assert_eq!(t.field("PAGENO"), "1", "step 1");
    assert_eq!(t.field("INFOMSG"), "TYPE S FOR DETAIL, U TO UPDATE ANY RECORD", "step 1");
    t.key(Aid::Pf(7));
    assert_eq!(t.msg(), "NO PREVIOUS PAGES TO DISPLAY", "step 2");
    t.key(Aid::Pf(8));
    assert_eq!(t.field("PAGENO"), "2", "step 3: page 2");
    t.key(Aid::Pf(7));
    assert_eq!(t.field("CRDNUM1"), cards[0], "step 3: page 1 again");
}

#[test]
fn tc_cdl_02() {
    let r = region();
    let mut t = card_list(&r);
    t.typ("ACCTSID", "abc").enter();
    assert_eq!(t.msg(), "ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER", "step 1");
    assert_eq!(color(&t, "ACCTSID"), Color::Red, "step 1: red");
    t.typ("ACCTSID", "00000000002").enter();
    assert_eq!(t.field("ACCTNO1"), "00000000002", "step 2: account 2 listed");
    assert_eq!(t.field("CRDNUM2"), "", "step 2: only account 2");
    t.typ("ACCTSID", "").enter();
    t.typ("CRDSEL1", "S").typ("CRDSEL2", "U").enter();
    assert_eq!(t.msg(), "PLEASE SELECT ONLY ONE RECORD TO VIEW OR UPDATE", "step 3");
    assert_eq!((color(&t, "CRDSEL1"), color(&t, "CRDSEL2")), (Color::Red, Color::Red), "step 3: both red");
    t.typ("CRDSEL1", "X").typ("CRDSEL2", "").enter();
    assert_eq!(t.msg(), "INVALID ACTION CODE", "step 4");
}

#[test]
fn tc_cdv_01() {
    let r = region();
    let mut t = card_list(&r);
    t.typ("CRDSEL1", "S").enter();
    assert_eq!(t.map(), "CCRDSLA", "step 1");
    assert_eq!(t.field("INFOMSG"), " Displaying requested details", "step 1");
    assert_ne!(t.field("CRDNAME"), "", "step 1: embossed name");
    assert!(t.s.screen.field("CARDSID").is_some_and(|f| f.attr.protected), "step 1: search fields protected");
    t.key(Aid::Pf(3));
    assert_eq!(t.map(), "CCRDLIA", "step 2");
}

#[test]
fn tc_cdv_02() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "4").enter();
    assert_eq!(t.field("INFOMSG"), "Please enter Account and Card Number", "step 1");
    t.enter();
    assert_eq!(t.msg(), "Account number not provided", "step 2");
    assert_eq!((t.field("ACCTSID").as_str(), t.field("CARDSID").as_str()), ("*", "*"), "step 2: both '*'");
    t.typ("ACCTSID", "00000000001").typ("CARDSID", "1234").enter();
    assert_eq!(t.msg(), "CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER", "step 3");
}

#[test]
fn tc_cdu_01() {
    let r = region();
    let mut t = card_list(&r);
    let card = t.field("CRDNUM1");
    t.typ("CRDSEL1", "U").enter();
    assert_eq!(t.field("INFOMSG"), "Details of selected card shown above", "step 1");
    t.enter();
    assert_eq!(t.msg(), "No change detected with respect to values fetched.", "step 2");
    t.typ("CRDSTCD", "X").enter();
    assert_eq!(t.msg(), "Card Active Status must be Y or N", "step 3");
    t.typ("CRDSTCD", "Y").typ("CRDNAME", "JOHN 2").enter();
    assert_eq!(t.msg(), "Card name can only contain alphabets and spaces", "step 4");
    t.typ("CRDNAME", "JOHN DOE").typ("EXPMON", "13").enter();
    assert_eq!(t.msg(), "Card expiry month must be between 1 and 12", "step 5");
    t.typ("EXPMON", "12").typ("CRDSTCD", "N").enter();
    assert_eq!(t.field("INFOMSG"), "Changes validated.Press F5 to save", "step 6");
    t.key(Aid::Pf(5));
    assert_eq!(t.field("INFOMSG"), "Changes committed to database", "step 7");
    let c = cobol::copybooks::CardRecord::from_bytes(&r.vsam.read("CARDDAT", card.as_bytes()).unwrap());
    assert_eq!(c.card_cvv_cd(), b"000", "step 7: CVV 000 (KD-07)");
    t.enter();
    assert_eq!(t.map(), "CCRDLIA", "step 8");
}

// ------------------------------------------------------------------ transactions

#[test]
fn tc_trl_01() {
    let r = region();
    let (root, o) = job_env(&r, "trl");
    run(&r, &root, &o, "POSTTRAN"); // precondition: posted transactions
    let mut t = user(&r);
    t.typ("OPTION", "6").enter();
    assert_eq!(t.map(), "COTRN0A", "step 1");
    assert_eq!(t.field("PAGENUM"), "00000001", "step 1");
    assert_eq!(t.field("TRNID01"), "", "step 1: row 1 = blank init record");
    t.typ("TRNIDIN", "abc").enter();
    assert_eq!(t.msg(), "Tran ID must be Numeric ...", "step 2");
    t.typ("TRNIDIN", "").enter();
    let first = t.field("TRNID02");
    t.key(Aid::Pf(8));
    assert_eq!(t.field("PAGENUM"), "00000002", "step 3: next page");
    t.key(Aid::Pf(7));
    assert_eq!(t.field("TRNID02"), first, "step 3: previous page");
    t.typ("SEL0002", "S").enter();
    assert_eq!(t.map(), "COTRN1A", "step 4");
    assert_eq!(t.field("TRNID"), first, "step 4");
}

#[test]
fn tc_trv_01() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "7").enter();
    t.enter();
    assert_eq!(t.msg(), "Tran ID can NOT be empty...", "step 1");
    t.typ("TRNIDIN", "9999999999999999").enter();
    assert_eq!(t.msg(), "Transaction ID NOT found...", "step 2");
    // a valid id: post the daily file first so TRANSACT has real transactions
    let (root, o) = job_env(&r, "trv");
    run(&r, &root, &o, "POSTTRAN");
    let id = cobol::copybooks::TranRecord::from_bytes(&r.vsam.unload("TRANSACT").unwrap()[1]).tran_id().to_vec();
    t.typ("TRNIDIN", std::str::from_utf8(&id).unwrap()).enter();
    let amt = t.field("TRNAMT");
    assert!(amt.len() == 12 && (amt.starts_with('+') || amt.starts_with('-')) && &amt[9..10] == ".", "step 3: amount {amt}");
    assert_ne!(t.field("MNAME"), "", "step 3: fields shown");
    t.key(Aid::Pf(4));
    assert_eq!(t.field("TRNID"), "", "step 4: cleared");
    t.key(Aid::Pf(5));
    assert_eq!(t.map(), "COTRN0A", "step 4: transaction list");
}

fn tra_fill(t: &mut Term) {
    t.typ("ACTIDIN", "00000000001").typ("TTYPCD", "01").typ("TCATCD", "0001").typ("TRNSRC", "POS TERM").typ("TDESC", "Coffee")
        .typ("TORIGDT", "2022-02-28").typ("TPROCDT", "2022-02-28").typ("MID", "000000001")
        .typ("MNAME", "Cafe").typ("MCITY", "Seattle").typ("MZIP", "98101");
}

#[test]
fn tc_tra_01() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "8").enter();
    t.enter();
    assert_eq!(t.msg(), "Account or Card Number must be entered...", "step 1");
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.msg(), "Type CD can NOT be empty...", "step 2");
    assert_eq!(t.field("CARDNIN").len(), 16, "step 2: card number from XREF");
    tra_fill(&mut t);
    t.typ("TRNAMT", "12.50").enter();
    assert_eq!(t.msg(), "Amount should be in format -99999999.99", "step 3");
    tra_fill(&mut t);
    t.typ("TRNAMT", "+00000012.50").typ("TORIGDT", "2022-02-30").enter();
    assert_eq!(t.msg(), "Orig Date - Not a valid date...", "step 4");
    tra_fill(&mut t);
    t.typ("TRNAMT", "+00000012.50").typ("CONFIRM", "").enter();
    assert_eq!(t.msg(), "Confirm to add this transaction...", "step 5");
}

#[test]
fn tc_tra_02() {
    let r = region();
    let (root, o) = job_env(&r, "tra");
    run(&r, &root, &o, "POSTTRAN");
    let last = r.vsam.unload("TRANSACT").unwrap().last().map(|x| std::str::from_utf8(cobol::copybooks::TranRecord::from_bytes(x).tran_id()).unwrap().parse::<u64>().unwrap()).unwrap();
    let mut t = user(&r);
    t.typ("OPTION", "8").enter();
    t.typ("ACTIDIN", "00000000001").key(Aid::Pf(5));
    assert_eq!(t.msg(), "Confirm to add this transaction...", "step 1");
    assert_ne!(t.field("TDESC"), "", "step 1: fields copied");
    t.typ("CONFIRM", "Y").enter();
    let expect = format!("Transaction added successfully.  Your Tran ID is {:016}.", last + 1);
    assert_eq!(t.msg(), expect, "step 2");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 2: green");
}

#[test]
fn tc_bil_01() {
    let r = region();
    let mut t = user(&r);
    t.typ("OPTION", "10").enter();
    t.enter();
    assert_eq!(t.msg(), "Acct ID can NOT be empty...", "step 1");
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.field("CURBAL"), "+0000000194.00", "step 2");
    assert_eq!(t.msg(), "Confirm to make a bill payment...", "step 2");
    t.typ("CONFIRM", "Y").enter();
    assert!(t.msg().starts_with("Payment successful.  Your Transaction ID is "), "step 3: {}", t.msg());
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 3: green");
    t.typ("ACTIDIN", "00000000001").enter();
    assert_eq!(t.msg(), "You have nothing to pay...", "step 4");
}

#[test]
fn tc_rpt_01() {
    use std::sync::{Arc, Mutex};
    let mut r = region();
    let decks = Arc::new(Mutex::new(Vec::new()));
    let d2 = decks.clone();
    r.on_job = Some(Box::new(move |deck: &[Vec<u8>]| d2.lock().unwrap().push(deck.to_vec())));
    let mut t = user(&r);
    t.typ("OPTION", "9").enter();
    t.enter();
    assert_eq!(t.msg(), "Select a report type to print report...", "step 1");
    t.typ("MONTHLY", "X").enter();
    assert_eq!(t.msg(), "Please confirm to print the Monthly report...", "step 2");
    t.typ("MONTHLY", "X").typ("CONFIRM", "Y").enter();
    assert_eq!(t.msg(), "Monthly report submitted for printing ...", "step 3");
    assert_eq!(color(&t, "ERRMSG"), Color::Green, "step 3: green");
    assert_eq!(decks.lock().unwrap().len(), 1, "step 3: one job deck written to the internal reader (JOBS TDQ)");
    t.typ("CUSTOM", "X").typ("SDTMM", "13").typ("SDTDD", "1").typ("SDTYYYY", "2022").typ("EDTMM", "1").typ("EDTDD", "1").typ("EDTYYYY", "2022").enter();
    assert_eq!(t.msg(), "Start Date - Not a valid Month...", "step 4");
    t.typ("CUSTOM", "X").typ("SDTMM", "01").typ("SDTDD", "01").typ("SDTYYYY", "2022").typ("EDTMM", "12").typ("EDTDD", "31").typ("EDTYYYY", "2022")
        .typ("CONFIRM", "X").enter();
    assert_eq!(t.msg(), "\"X\" is not a valid value to confirm...", "step 5");
}

// ------------------------------------------------------------------ batch

fn sysout(logs: &[batch::jobs::StepLog]) -> Vec<String> {
    logs.iter().flat_map(|l| l.sysout.clone()).collect()
}

#[test]
fn tc_bat_01() {
    let r = region();
    let (root, o) = job_env(&r, "bat01");
    run(&r, &root, &o, "TRANBKP");
    let logs = run(&r, &root, &o, "POSTTRAN");
    let out = sysout(&logs);
    assert!(out.contains(&"TRANSACTIONS PROCESSED :000000300".to_string()), "step 1");
    assert!(out.contains(&"TRANSACTIONS REJECTED  :000000038".to_string()), "step 1");
    assert_eq!(logs.last().unwrap().rc.as_ref().ok(), Some(&4), "step 1: RC=0004");
    let rej = std::fs::read(root.join("datasets/AWS.M2.CARDDEMO.DALYREJS.G0001V00")).unwrap();
    assert_eq!(rej.len(), 38 * 430, "step 2: 38 records of 430 bytes");
    for rec in rej.chunks(430) {
        let code: u32 = std::str::from_utf8(&rec[350..354]).unwrap().parse().unwrap();
        assert!((100..=103).contains(&code), "step 2: reason {code}");
    }
    let mut t = user(&r);
    t.typ("OPTION", "6").enter();
    assert_ne!(t.field("TRNID02"), "", "step 3: posted transactions listed");
}

#[test]
fn tc_bat_02() {
    let r = region();
    let (root, o) = job_env(&r, "bat02");
    run(&r, &root, &o, "TRANBKP");
    run(&r, &root, &o, "POSTTRAN");
    let before = r.vsam.unload("ACCTDATA").unwrap();
    let logs = run(&r, &root, &JobOpts { intcalc_parm: "2022071800".into(), ..o.clone() }, "INTCALC");
    assert_eq!(logs.last().unwrap().rc.as_ref().ok(), Some(&0), "step 1: RC=0000");
    let sys = std::fs::read(root.join("datasets/AWS.M2.CARDDEMO.SYSTRAN.G0001V00")).unwrap();
    let t1 = cobol::copybooks::TranRecord::from_bytes(&sys[..350]);
    assert_eq!(t1.tran_id(), b"2022071800000001", "step 1: first id");
    assert_eq!((t1.tran_type_cd(), t1.tran_cat_cd()), (&b"01"[..], &b"0005"[..]), "step 1: type/category");
    assert!(t1.tran_desc().starts_with(b"Int. for a/c "), "step 1: description");
    // KD-01: the last account in TCATBAL order keeps its cycle amounts; the others are reset
    let tcat = r.vsam.unload("TCATBALF").unwrap();
    let last_acct = tcat.last().unwrap()[0..11].to_vec();
    let after = r.vsam.unload("ACCTDATA").unwrap();
    let find = |v: &Vec<Vec<u8>>, k: &[u8]| cobol::copybooks::AccountRecord::from_bytes(v.iter().find(|x| &x[0..11] == k).unwrap());
    let (b, a) = (find(&before, &last_acct), find(&after, &last_acct));
    assert_eq!(a.acct_curr_cyc_credit_n(), b.acct_curr_cyc_credit_n(), "step 2: last account not reset (KD-01)");
    let other = cobol::copybooks::AccountRecord::from_bytes(&after[0]);
    assert_eq!(other.acct_curr_cyc_credit_n(), cobol::Dec::ZERO, "step 2: other accounts reset");
}

#[test]
fn tc_bat_03() {
    let r = region();
    let (root, o) = job_env(&r, "bat03");
    for j in ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN", "CREASTMT"] {
        let logs = run(&r, &root, &o, j);
        if ["TRANBKP", "COMBTRAN", "CREASTMT"].contains(&j) {
            assert!(logs.iter().all(|l| l.rc.as_ref().ok() == Some(&0)), "step 1: {j} RC=0000");
        }
    }
    let ps = std::fs::read_to_string(root.join("datasets/AWS.M2.CARDDEMO.STATEMNT.PS")).unwrap();
    let html = std::fs::read_to_string(root.join("datasets/AWS.M2.CARDDEMO.STATEMNT.HTML")).unwrap();
    assert!(ps.contains("Basic Details") && ps.contains("TRANSACTION SUMMARY") && ps.contains("Total EXP:"), "step 2: text statement");
    assert!(html.contains("<table") && html.contains("Transaction Summary"), "step 2: HTML statement (no total line, as in the original)");
    assert_eq!(ps.matches("END OF STATEMENT").count(), 50, "step 2: one statement per card");
}

#[test]
fn tc_bat_04() {
    let r = region();
    let (root, o) = job_env(&r, "bat04");
    for j in ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN"] {
        run(&r, &root, &o, j);
    }
    let o2 = JobOpts { report_start: "2022-01-01".into(), report_end: "2099-12-31".into(), ..o };
    run(&r, &root, &o2, "TRANREPT");
    let rep = std::fs::read_to_string(root.join("datasets/AWS.M2.CARDDEMO.TRANREPT.G0001V00")).unwrap();
    assert!(rep.contains("Page Total") && rep.contains("Account Total") && rep.contains("Grand Total"), "step 1");
    // KD-03: the report ends with page total + grand total, no final account total
    let tail: Vec<&str> = rep.lines().rev().filter(|l| !l.trim().is_empty() && !l.starts_with("---")).take(2).collect();
    assert!(tail[0].contains("Grand Total") && tail[1].contains("Page Total"), "step 2: {tail:?}");
}

#[test]
fn tc_bat_05() {
    let r = region();
    let (root, o) = job_env(&r, "bat05");
    let n = |logs: &[batch::jobs::StepLog]| sysout(logs).len();
    assert_eq!(n(&run(&r, &root, &o, "READCARD")), 52, "step 1: READCARD 50 records + 2");
    assert_eq!(n(&run(&r, &root, &o, "READXREF")), 102, "step 1: READXREF twice");
    assert_eq!(n(&run(&r, &root, &o, "READCUST")), 102, "step 1: READCUST twice");
    assert!(n(&run(&r, &root, &o, "READACCT")) > 700, "step 1: READACCT");
    let v = sysout(&run(&r, &root, &o, "DALYVRFY"));
    // every card in the sample daily file is in XREF: card, account and customer are shown for each
    assert_eq!(v.iter().filter(|l| l.starts_with("CARD NUMBER: ")).count(), 301, "step 1: DALYVRFY verifies all 300 transactions (+1: the lookup repeats after EOF, FR-BAT-VER-02)");
    assert!(!v.iter().any(|l| l.contains("COULD NOT BE VERIFIED")), "step 1: no unverifiable cards in the sample data");
}

#[test]
fn tc_bat_06() {
    let r = region();
    let (root, o) = job_env(&r, "bat06");
    let e = sysout(&run(&r, &root, &o, "CBEXPORT"));
    let total = e.iter().find(|l| l.starts_with("CBEXPORT: Total Records Exported:")).cloned().unwrap_or_default();
    let count: u64 = ["CUSTDATA", "ACCTDATA", "CARDXREF", "TRANSACT", "CARDDATA"].iter().map(|d| r.vsam.count(d).unwrap() as u64).sum();
    assert!(total.ends_with(&format!("{count:09}")), "step 1: {total} vs {count}");
    let i = sysout(&run(&r, &root, &o, "CBIMPORT"));
    assert!(i.contains(&"CBIMPORT: Errors Written: 000000000".into()) && i.contains(&"CBIMPORT: Unknown Record Types: 000000000".into()), "step 2");
}

#[test]
fn tc_bat_07() {
    let r = region();
    let (root, o) = job_env(&r, "bat07");
    r.vsam.clear("ACCTDATA").unwrap();
    run(&r, &root, &o, "ACCTFILE");
    assert_eq!(r.vsam.count("ACCTDATA").unwrap(), 50, "step 1: reloaded");
    let t0 = std::time::Instant::now();
    batch::repro::reset(&r.vsam, &seed()).unwrap();
    assert!(t0.elapsed().as_millis() < 1000, "step 2: reset under a second");
}
