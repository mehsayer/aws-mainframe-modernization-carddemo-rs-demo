//! Phase 2: the original COBOL sign-on and menu programs run on the bridge (GnuCOBOL; the bridge runs on Linux).
#![cfg(unix)]
use cics::{Aid, Region, Session};
use std::sync::Mutex;

/// libcob is not thread-safe: COBOL tasks run one at a time.
static COB: Mutex<()> = Mutex::new(());

fn region() -> Region {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    cobridge::region(batch::repro::sample_store().unwrap())
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

/// COBOL DISPLAY output goes to the sysout file, not to the terminal (CORPT00C DISPLAYs on Enter).
#[cfg(all(target_os = "linux", target_env = "gnu"))] // set_sysout
#[test]
fn cobol_display_goes_to_sysout_file() {
    let _g = COB.lock().unwrap_or_else(|e| e.into_inner());
    let log = std::env::temp_dir().join(format!("cobridge-sysout-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&log);
    cobridge::set_sysout(&log).unwrap();
    let r = region();
    let mut s = Session::default();
    r.run(&mut s, Some("CC00"), Aid::Enter);
    s.screen.type_into("USERID", "USER0001");
    s.screen.type_into("PASSWD", "PASSWORD");
    r.run(&mut s, None, Aid::Enter);
    s.screen.type_into("OPTION", "9");
    r.run(&mut s, None, Aid::Enter);
    assert_eq!(s.screen.map, "CORPT0A");
    r.run(&mut s, None, Aid::Enter);
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log); // the stream stays open on the unlinked file: nothing else reads it
    assert!(text.contains("PROCESS ENTER KEY"), "sysout file: {text:?}");
}

/// libcob's error messages (written to the C stream stderr) go to the sysout file too, not to the terminal.
/// (Its runtime warnings write their `libcob: ` prefix straight to descriptor 2: see set_sysout.)
#[cfg(all(target_os = "linux", target_env = "gnu"))] // set_sysout
#[test]
fn libcob_messages_go_to_sysout_file() {
    let _g = COB.lock().unwrap_or_else(|e| e.into_inner());
    let log = std::env::temp_dir().join(format!("cobridge-warn-{}.log", std::process::id()));
    let _ = std::fs::remove_file(&log);
    cobridge::set_sysout(&log).unwrap();
    let r = region();
    let mut s = Session::default();
    r.run(&mut s, Some("CC00"), Aid::Enter); // loads and initialises libcob
    // cob_runtime_error only prints (ending the run is up to its caller)
    let sym = unsafe { libc::dlsym(libc::RTLD_DEFAULT, c"cob_runtime_error".as_ptr()) };
    assert!(!sym.is_null(), "libcob loaded with RTLD_GLOBAL");
    let error: unsafe extern "C" fn(*const std::ffi::c_char, ...) = unsafe { std::mem::transmute(sym) };
    unsafe {
        error(c"%s".as_ptr(), c"bridge test error".as_ptr());
        libc::fflush(std::ptr::null_mut());
    }
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(&log);
    assert!(text.contains("libcob: ") && text.contains("bridge test error"), "sysout file: {text:?}");
}
