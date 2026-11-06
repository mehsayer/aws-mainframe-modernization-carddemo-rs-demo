//! Phase 2: the original COBOL sign-on and menu programs run on the bridge.
use cics::{Aid, Region, Session};
use std::path::PathBuf;
use std::sync::Mutex;

/// libcob is not thread-safe: COBOL tasks run one at a time.
static COB: Mutex<()> = Mutex::new(());

fn region() -> Region {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    let v = vsam::Vsam::open_in_memory().unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cobol/app/data/EBCDIC");
    batch::repro::reset(&v, &seed).unwrap();
    cobridge::region(v)
}

fn field(s: &Session, n: &str) -> String {
    s.screen.field(n).map(|f| cobol::text::s(&f.data).trim_end().to_string()).unwrap_or_default()
}

#[test]
fn cobol_signon_and_menu() {
    let _g = COB.lock().unwrap_or_else(|e| e.into_inner());
    assert!(cobridge::available(), "build the modules first: python3 rust/cobridge/cicsprep.py");
    let r = region();
    let mut s = Session::default();
    r.run(&mut s, Some("CC00"), Aid::Enter);
    println!("{:?}\n{}", s.screen.text, s.screen.render().join("\n"));
    assert_eq!(s.screen.map, "COSGN0A");
    s.screen.type_into("USERID", "USER0001");
    s.screen.type_into("PASSWD", "PASSWORD");
    r.run(&mut s, None, Aid::Enter);
    println!("{:?} {:?}\n{}", s.log, s.screen.text, s.screen.render().join("\n"));
    assert_eq!(s.screen.map, "COMEN1A");
    assert_eq!(field(&s, "OPTN001"), "01. Account View");
}
