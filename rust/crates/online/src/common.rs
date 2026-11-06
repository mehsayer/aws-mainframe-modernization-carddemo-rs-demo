//! Shared copybooks of the online programs: COTTL01Y (titles), CSMSG01Y (messages),
//! CSDAT01Y (date/time), COCOM01Y (commarea) helpers and POPULATE-HEADER-INFO.

use chrono::{NaiveDateTime, Timelike};
use cics::Map;
use cobol::copybooks::CarddemoCommarea;

pub const CCDA_TITLE01: &str = "      AWS Mainframe Modernization       ";
pub const CCDA_TITLE02: &str = "              CardDemo                  ";
pub const CCDA_MSG_THANK_YOU: &str = "Thank you for using CardDemo application...      ";
pub const CCDA_MSG_INVALID_KEY: &str = "Invalid key pressed. Please see below...         ";

/// Clock (FUNCTION CURRENT-DATE / ASKTIME); `CARDDEMO_NOW` overrides it for tests.
pub fn now() -> NaiveDateTime {
    fixed_now().unwrap_or_else(|| chrono::Local::now().naive_local())
}

/// The test clock: `CARDDEMO_NOW` when it is set and valid (`YYYY-MM-DDTHH:MM:SS[.ff]`), else None.
pub fn fixed_now() -> Option<NaiveDateTime> {
    let s = std::env::var("CARDDEMO_NOW").ok()?;
    ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"].iter().find_map(|f| NaiveDateTime::parse_from_str(&s, f).ok())
}

/// POPULATE-HEADER-INFO
pub fn header(map: &mut Map, tranid: &str, pgm: &str) {
    let t = now();
    map.set_str("TITLE01", CCDA_TITLE01);
    map.set_str("TITLE02", CCDA_TITLE02);
    map.set_str("TRNNAME", tranid);
    map.set_str("PGMNAME", pgm);
    map.set_str("CURDATE", &t.format("%m/%d/%y").to_string());
    map.set_str("CURTIME", &format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()));
}

/// `MOVE DFHCOMMAREA(1:EIBCALEN) TO <commarea of total length len>`
pub fn load_commarea(ca: &[u8], len: usize) -> Vec<u8> {
    let mut v = vec![b' '; len];
    let n = ca.len().min(len);
    v[..n].copy_from_slice(&ca[..n]);
    v
}

/// The 160-byte CARDDEMO-COMMAREA view of a commarea buffer.
pub fn cdemo(buf: &[u8]) -> CarddemoCommarea {
    CarddemoCommarea::from_bytes(&buf[..CarddemoCommarea::LEN.min(buf.len())])
}
pub fn put_cdemo(buf: &mut [u8], c: &CarddemoCommarea) {
    buf[..CarddemoCommarea::LEN].copy_from_slice(&c.buf);
}

/// Program-specific commarea extension field at `off` (relative to the end of
/// CARDDEMO-COMMAREA) of `len` bytes.
pub fn ext(buf: &[u8], off: usize, len: usize) -> &[u8] {
    &buf[CarddemoCommarea::LEN + off..CarddemoCommarea::LEN + off + len]
}
pub fn set_ext(buf: &mut [u8], off: usize, len: usize, v: &[u8]) {
    cobol::text::mv(&mut buf[CarddemoCommarea::LEN + off..CarddemoCommarea::LEN + off + len], v);
}

pub fn blank(b: &[u8]) -> bool {
    cobol::text::is_blank(b)
}

/// The pseudo-conversational context flag (CDEMO-PGM-CONTEXT: 0 enter, 1 re-enter).
pub fn reenter(c: &CarddemoCommarea) -> bool {
    c.cdemo_pgm_context() == b"1"
}

/// COMEN02Y main menu options: (number, name, program, user type).
pub const MENU_OPTIONS: &[(u8, &str, &str, &str)] = &[
    (1, "Account View", "COACTVWC", "U"),
    (2, "Account Update", "COACTUPC", "U"),
    (3, "Credit Card List", "COCRDLIC", "U"),
    (4, "Credit Card View", "COCRDSLC", "U"),
    (5, "Credit Card Update", "COCRDUPC", "U"),
    (6, "Transaction List", "COTRN00C", "U"),
    (7, "Transaction View", "COTRN01C", "U"),
    (8, "Transaction Add", "COTRN02C", "U"),
    (9, "Transaction Reports", "CORPT00C", "U"),
    (10, "Bill Payment", "COBIL00C", "U"),
    (11, "Pending Authorization View", "COPAUS0C", "U"),
];

/// COADM02Y admin menu options: (number, name, program).
pub const ADMIN_OPTIONS: &[(u8, &str, &str)] = &[
    (1, "User List (Security)", "COUSR00C"),
    (2, "User Add (Security)", "COUSR01C"),
    (3, "User Update (Security)", "COUSR02C"),
    (4, "User Delete (Security)", "COUSR03C"),
    (5, "Transaction Type List/Update (Db2)", "COTRTLIC"),
    (6, "Transaction Type Maintenance (Db2)", "COTRTUPC"),
];
