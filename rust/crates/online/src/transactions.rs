//! Transactions: COTRN00C (CT00 list), COTRN01C (CT01 view), COTRN02C (CT02 add),
//! COBIL00C (CB00 bill payment), CORPT00C (CR00 report submission).
//! Requirements FR-TRL, FR-TRV, FR-TRA, FR-BIL, FR-RPT.

use crate::common::*;
use crate::csutldtc::csutldtc;
use chrono::{Datelike, Duration, NaiveDate, Timelike};
use cics::{Aid, Color, Cr, Map, Task, Transfer};
use cobol::copybooks::{AccountRecord, CardXrefRecord, CarddemoCommarea, TranRecord};
use cobol::edit::edit;
use cobol::numeric::{fit, is_numeric, Pic};
use cobol::{text, Dec};
use vsam::{Browse, Resp};

const FILE: &str = "TRANSACT";
/// CDEMO-CT0x / CB00 extension: TRNID-FIRST(16) TRNID-LAST(16) PAGE-NUM 9(8)
/// NEXT-PAGE-FLG(1) TRN-SEL-FLG(1) TRN-SELECTED(16)
const CA_LEN: usize = CarddemoCommarea::LEN + 58;
const X_FIRST: (usize, usize) = (0, 16);
const X_LAST: (usize, usize) = (16, 16);
const X_PAGE: (usize, usize) = (32, 8);
const X_NEXT: (usize, usize) = (40, 1);
const X_SELFLG: (usize, usize) = (41, 1);
const X_SEL: (usize, usize) = (42, 16);

fn xget(ca: &[u8], f: (usize, usize)) -> Vec<u8> {
    ext(ca, f.0, f.1).to_vec()
}
fn xset(ca: &mut [u8], f: (usize, usize), v: &[u8]) {
    set_ext(ca, f.0, f.1, v)
}
fn set_to(ca: &mut [u8], pgm: &str) {
    let mut c = cdemo(ca);
    c.set_cdemo_to_program(pgm.as_bytes());
    put_cdemo(ca, &c);
}
fn return_to_prev(t: &Task, ca: &mut [u8], tran: &str, pgm: &str) -> Transfer {
    let mut c = cdemo(ca);
    if blank(c.cdemo_to_program()) {
        c.set_cdemo_to_program(b"COSGN00C");
    }
    c.set_cdemo_from_tranid(tran.as_bytes());
    c.set_cdemo_from_program(pgm.as_bytes());
    c.set_cdemo_pgm_context(b"0");
    put_cdemo(ca, &c);
    t.xctl(&text::s(c.cdemo_to_program()), Some(ca))
}
fn entry(t: &Task, tran: &str, pgm: &str, len: usize) -> Result<Vec<u8>, Transfer> {
    let mut ca = vec![b' '; len];
    if t.eibcalen == 0 {
        set_to(&mut ca, "COSGN00C");
        return Err(return_to_prev(t, &mut ca, tran, pgm));
    }
    Ok(load_commarea(&t.commarea, len))
}
fn first_entry(ca: &mut [u8]) -> bool {
    let mut c = cdemo(ca);
    if reenter(&c) {
        return false;
    }
    c.set_cdemo_pgm_context(b"1");
    put_cdemo(ca, &c);
    true
}
fn back_to_caller(ca: &mut [u8], default: &str) {
    let from = cdemo(ca).cdemo_from_program().to_vec();
    let to = if blank(&from) { default.to_string() } else { text::s(&from) };
    set_to(ca, &to);
}

// ===================================================================== COTRN00C
const PGM0: &str = "COTRN00C";
const TRAN0: &str = "CT00";

struct LSt {
    err: bool,
    eof: bool,
    msg: String,
    erase: bool,
}

pub fn cotrn00c(t: &mut Task) -> Cr {
    let mut st = LSt { err: false, eof: false, msg: String::new(), erase: true };
    let mut ca = entry(t, TRAN0, PGM0, CA_LEN)?;
    let mut map = t.map("COTRN0A");
    map.cursor("TRNIDIN");
    if first_entry(&mut ca) {
        map.clear();
        tl_enter(t, &mut ca, &mut map, &mut st)?;
        tl_send(t, &mut map, &st);
    } else {
        map = t.receive_map("COTRN0A");
        map.cursor("TRNIDIN");
        match t.eibaid {
            Aid::Enter => tl_enter(t, &mut ca, &mut map, &mut st)?,
            Aid::Pf(3) => {
                set_to(&mut ca, "COMEN01C");
                return Err(return_to_prev(t, &mut ca, TRAN0, PGM0));
            }
            Aid::Pf(7) => {
                let first = xget(&ca, X_FIRST);
                let key = if blank(&first) { vec![0u8; 16] } else { first };
                xset(&mut ca, X_NEXT, b"Y");
                if page_num(&ca) > 1 {
                    tl_backward(t, &mut ca, &mut map, &mut st, key)?;
                } else {
                    st.msg = "You are already at the top of the page...".into();
                    st.erase = false;
                    tl_send(t, &mut map, &st);
                }
            }
            Aid::Pf(8) => {
                let last = xget(&ca, X_LAST);
                let key = if blank(&last) { vec![0xFFu8; 16] } else { last };
                if xget(&ca, X_NEXT) == b"Y" {
                    tl_forward(t, &mut ca, &mut map, &mut st, key)?;
                } else {
                    st.msg = "You are already at the bottom of the page...".into();
                    st.erase = false;
                    tl_send(t, &mut map, &st);
                }
            }
            _ => {
                st.err = true;
                st.msg = CCDA_MSG_INVALID_KEY.into();
                tl_send(t, &mut map, &st);
            }
        }
    }
    Err(t.ret(Some(TRAN0), Some(&ca)))
}

fn tl_send(t: &mut Task, map: &mut Map, st: &LSt) {
    header(map, TRAN0, PGM0);
    map.set_str("ERRMSG", &st.msg);
    t.send_map(map, st.erase);
}
fn page_num(ca: &[u8]) -> u64 {
    text::s(ext(ca, X_PAGE.0, X_PAGE.1)).trim().parse().unwrap_or(0)
}
fn set_page(ca: &mut [u8], n: u64) {
    xset(ca, X_PAGE, format!("{:08}", n % 100_000_000).as_bytes());
}

fn tl_enter(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut LSt) -> Cr {
    let mut flg = b" ".to_vec();
    let mut sel = vec![b' '; 16];
    for i in 1..=10 {
        let s = map.get(&format!("SEL{i:04}")).to_vec();
        if !blank(&s) {
            flg = s;
            sel = map.get(&format!("TRNID{i:02}")).to_vec();
            break;
        }
    }
    xset(ca, X_SELFLG, &flg);
    xset(ca, X_SEL, &sel);
    let (f, s) = (xget(ca, X_SELFLG), xget(ca, X_SEL));
    if !blank(&f) && !blank(&s) {
        if f[0] == b'S' || f[0] == b's' {
            let mut c = cdemo(ca);
            c.set_cdemo_to_program(b"COTRN01C");
            c.set_cdemo_from_tranid(TRAN0.as_bytes());
            c.set_cdemo_from_program(PGM0.as_bytes());
            c.set_cdemo_pgm_context(b"0");
            put_cdemo(ca, &c);
            return Err(t.xctl("COTRN01C", Some(ca)));
        }
        st.msg = "Invalid selection. Valid value is S".into();
        map.cursor("TRNIDIN");
    }
    // TRAN-ID (WORKING-STORAGE, no VALUE) is binary zeros in a fresh task; a non-numeric search
    // leaves it so, and the browse below starts at the lowest key
    let mut key = vec![0u8; 16];
    if blank(map.get("TRNIDIN")) {
        key = vec![0u8; 16];
    } else if is_numeric(map.get("TRNIDIN"), false) {
        key = map.get("TRNIDIN").to_vec();
    } else {
        st.err = true;
        st.msg = "Tran ID must be Numeric ...".into();
        map.cursor("TRNIDIN");
        tl_send(t, map, st);
    }
    map.cursor("TRNIDIN");
    set_page(ca, 0);
    tl_forward(t, ca, map, st, key)?;
    if !st.err {
        map.set_str("TRNIDIN", " ");
    }
    Ok(())
}

fn tl_startbr(t: &mut Task, map: &mut Map, st: &mut LSt, key: &[u8]) -> Option<Browse> {
    match t.startbr(FILE, key, true) {
        Ok(b) => Some(b),
        Err(Resp::NotFnd) => {
            st.eof = true;
            st.msg = "You are at the top of the page...".into();
            map.cursor("TRNIDIN");
            tl_send(t, map, st);
            None
        }
        Err(_) => {
            st.err = true;
            st.msg = "Unable to lookup transaction...".into();
            map.cursor("TRNIDIN");
            tl_send(t, map, st);
            None
        }
    }
}

fn tl_read(t: &mut Task, map: &mut Map, st: &mut LSt, br: &mut Option<Browse>, fwd: bool) -> Option<TranRecord> {
    let r = match br.as_mut() {
        Some(b) => if fwd { t.readnext(b) } else { t.readprev(b) },
        None => Err(Resp::InvReq),
    };
    match r {
        Ok(rec) => Some(TranRecord::from_bytes(&rec)),
        Err(Resp::EndFile) => {
            st.eof = true;
            st.msg = if fwd { "You have reached the bottom of the page..." } else { "You have reached the top of the page..." }.into();
            map.cursor("TRNIDIN");
            tl_send(t, map, st);
            None
        }
        Err(_) => {
            st.err = true;
            st.msg = "Unable to lookup transaction...".into();
            map.cursor("TRNIDIN");
            tl_send(t, map, st);
            None
        }
    }
}

fn tl_populate(map: &mut Map, ca: &mut [u8], idx: usize, tr: &TranRecord) {
    let ts = tr.tran_orig_ts();
    // WS-CURDATE-MM-DD-YY: the timestamp bytes are moved as they are (an X'00' record gives NULs, and a
    // field starting with X'00' is not transmitted)
    let date = [&ts[5..7], b"/", &ts[8..10], b"/", &ts[2..4]].concat();
    map.set(&format!("TRNID{idx:02}"), tr.tran_id());
    map.set(&format!("TDATE{idx:02}"), &date);
    map.set(&format!("TDESC{idx:02}"), tr.tran_desc());
    map.set_str(&format!("TAMT{idx:03}"), &edit(tr.tran_amt_n(), "+99999999.99"));
    if idx == 1 {
        xset(ca, X_FIRST, tr.tran_id());
    }
    if idx == 10 {
        xset(ca, X_LAST, tr.tran_id());
    }
}

fn tl_init(map: &mut Map) {
    for i in 1..=10 {
        map.set_str(&format!("TRNID{i:02}"), " ");
        map.set_str(&format!("TDATE{i:02}"), " ");
        map.set_str(&format!("TDESC{i:02}"), " ");
        map.set_str(&format!("TAMT{i:03}"), " ");
    }
}

fn endbr(t: &Task, br: Option<Browse>) -> Cr {
    if br.is_some() { Ok(()) } else { Err(t.abend("AEIP")) }
}

fn tl_forward(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut LSt, key: Vec<u8>) -> Cr {
    let mut br = tl_startbr(t, map, st, &key);
    if st.err {
        return Ok(());
    }
    if t.eibaid != Aid::Enter && t.eibaid != Aid::Pf(7) && t.eibaid != Aid::Pf(3) {
        tl_read(t, map, st, &mut br, true);
    }
    if !st.eof && !st.err {
        tl_init(map);
    }
    let mut idx = 1;
    while !(idx >= 11 || st.eof || st.err) {
        if let Some(tr) = tl_read(t, map, st, &mut br, true) {
            tl_populate(map, ca, idx, &tr);
            idx += 1;
        }
    }
    if !st.eof && !st.err {
        let n = page_num(ca) + 1;
        set_page(ca, n);
        let more = tl_read(t, map, st, &mut br, true).is_some();
        xset(ca, X_NEXT, if more && !st.err { b"Y" } else { b"N" });
    } else {
        xset(ca, X_NEXT, b"N");
        if idx > 1 {
            let n = page_num(ca) + 1;
            set_page(ca, n);
        }
    }
    endbr(t, br)?;
    map.set_str("PAGENUM", &format!("{:08}", page_num(ca)));
    map.set_str("TRNIDIN", " ");
    tl_send(t, map, st);
    Ok(())
}

fn tl_backward(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut LSt, key: Vec<u8>) -> Cr {
    let mut br = tl_startbr(t, map, st, &key);
    if st.err {
        return Ok(());
    }
    if t.eibaid != Aid::Enter && t.eibaid != Aid::Pf(8) {
        tl_read(t, map, st, &mut br, false);
    }
    if !st.eof && !st.err {
        tl_init(map);
    }
    let mut idx: i32 = 10;
    while !(idx <= 0 || st.eof || st.err) {
        if let Some(tr) = tl_read(t, map, st, &mut br, false) {
            tl_populate(map, ca, idx as usize, &tr);
            idx -= 1;
        }
    }
    if !st.eof && !st.err {
        let more = tl_read(t, map, st, &mut br, false).is_some();
        if xget(ca, X_NEXT) == b"Y" {
            let n = if more && !st.err && page_num(ca) > 1 { page_num(ca) - 1 } else { 1 };
            set_page(ca, n);
        }
    }
    endbr(t, br)?;
    map.set_str("PAGENUM", &format!("{:08}", page_num(ca)));
    tl_send(t, map, st);
    Ok(())
}

// ===================================================================== COTRN01C
pub fn cotrn01c(t: &mut Task) -> Cr {
    const PGM: &str = "COTRN01C";
    const TRAN: &str = "CT01";
    let mut ca = entry(t, TRAN, PGM, CA_LEN)?;
    let mut map = t.map("COTRN1A");
    let mut msg = String::new();
    let send = |t: &mut Task, map: &mut Map, msg: &str| {
        header(map, TRAN, PGM);
        map.set_str("ERRMSG", msg);
        t.send_map(map, true);
    };
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("TRNIDIN");
        let sel = xget(&ca, X_SEL);
        if !blank(&sel) {
            map.set("TRNIDIN", &sel);
            tv_enter(t, &mut map, &mut msg);
        }
        send(t, &mut map, &msg);
    } else {
        map = t.receive_map("COTRN1A");
        match t.eibaid {
            Aid::Enter => {
                tv_enter(t, &mut map, &mut msg);
            }
            Aid::Pf(3) => {
                back_to_caller(&mut ca, "COMEN01C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            Aid::Pf(4) => {
                map.cursor("TRNIDIN");
                for f in ["TRNIDIN", "TRNID", "CARDNUM", "TTYPCD", "TCATCD", "TRNSRC", "TRNAMT", "TDESC", "TORIGDT", "TPROCDT", "MID", "MNAME", "MCITY", "MZIP"] {
                    map.set_str(f, " ");
                }
                send(t, &mut map, "");
            }
            Aid::Pf(5) => {
                set_to(&mut ca, "COTRN00C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            _ => send(t, &mut map, CCDA_MSG_INVALID_KEY),
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca)))
}

fn tv_enter(t: &mut Task, map: &mut Map, msg: &mut String) {
    let send = |t: &mut Task, map: &mut Map, msg: &str| {
        header(map, "CT01", "COTRN01C");
        map.set_str("ERRMSG", msg);
        t.send_map(map, true);
    };
    map.cursor("TRNIDIN");
    if blank(map.get("TRNIDIN")) {
        *msg = "Tran ID can NOT be empty...".into();
        send(t, map, msg);
        return;
    }
    for f in ["TRNID", "CARDNUM", "TTYPCD", "TCATCD", "TRNSRC", "TRNAMT", "TDESC", "TORIGDT", "TPROCDT", "MID", "MNAME", "MCITY", "MZIP"] {
        map.set_str(f, " ");
    }
    let key = map.get("TRNIDIN").to_vec();
    match t.read(FILE, &key, true) {
        Ok(r) => {
            let tr = TranRecord::from_bytes(&r);
            map.set("TRNID", tr.tran_id());
            map.set("CARDNUM", tr.tran_card_num());
            map.set("TTYPCD", tr.tran_type_cd());
            map.set("TCATCD", tr.tran_cat_cd());
            map.set("TRNSRC", tr.tran_source());
            map.set_str("TRNAMT", &edit(tr.tran_amt_n(), "+99999999.99"));
            map.set("TDESC", tr.tran_desc());
            map.set("TORIGDT", tr.tran_orig_ts());
            map.set("TPROCDT", tr.tran_proc_ts());
            map.set("MID", tr.tran_merchant_id());
            map.set("MNAME", tr.tran_merchant_name());
            map.set("MCITY", tr.tran_merchant_city());
            map.set("MZIP", tr.tran_merchant_zip());
            send(t, map, msg);
        }
        Err(Resp::NotFnd) => {
            *msg = "Transaction ID NOT found...".into();
            send(t, map, msg);
        }
        Err(_) => {
            *msg = "Unable to lookup Transaction...".into();
            send(t, map, msg);
        }
    }
}

// ===================================================================== COTRN02C
// Every SEND in this program is followed by RETURN TRANSID (BR-TRA-0).
const PGM2: &str = "COTRN02C";
const TRAN2: &str = "CT02";
const DATA_FIELDS: [&str; 11] = ["TTYPCD", "TCATCD", "TRNSRC", "TRNAMT", "TDESC", "TORIGDT", "TPROCDT", "MID", "MNAME", "MCITY", "MZIP"];

fn ta_send(t: &mut Task, map: &mut Map, msg: &str, ca: &[u8]) -> Transfer {
    header(map, TRAN2, PGM2);
    map.set_str("ERRMSG", msg);
    t.send_map(map, true);
    t.ret(Some(TRAN2), Some(ca))
}

pub fn cotrn02c(t: &mut Task) -> Cr {
    let mut ca = entry(t, TRAN2, PGM2, CA_LEN)?;
    let mut map = t.map("COTRN2A");
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("ACTIDIN");
        let sel = xget(&ca, X_SEL);
        if !blank(&sel) {
            map.set("CARDNIN", &sel);
            ta_enter(t, &mut map, &ca)?;
        }
        return Err(ta_send(t, &mut map, "", &ca));
    }
    map = t.receive_map("COTRN2A");
    match t.eibaid {
        Aid::Enter => ta_enter(t, &mut map, &ca)?,
        Aid::Pf(3) => {
            back_to_caller(&mut ca, "COMEN01C");
            return Err(return_to_prev(t, &mut ca, TRAN2, PGM2));
        }
        Aid::Pf(4) => {
            ta_clear(&mut map);
            return Err(ta_send(t, &mut map, "", &ca));
        }
        Aid::Pf(5) => {
            // COPY-LAST-TRAN-DATA
            ta_keys(t, &mut map, &ca)?;
            let last = last_tran(t, &mut map, &ca, true)?;
            map.set_str("TRNAMT", &edit(last.tran_amt_n(), "+99999999.99"));
            map.set("TTYPCD", last.tran_type_cd());
            map.set("TCATCD", last.tran_cat_cd());
            map.set("TRNSRC", last.tran_source());
            map.set("TDESC", last.tran_desc());
            map.set("TORIGDT", last.tran_orig_ts());
            map.set("TPROCDT", last.tran_proc_ts());
            map.set("MID", last.tran_merchant_id());
            map.set("MNAME", last.tran_merchant_name());
            map.set("MCITY", last.tran_merchant_city());
            map.set("MZIP", last.tran_merchant_zip());
            ta_enter(t, &mut map, &ca)?;
        }
        _ => return Err(ta_send(t, &mut map, CCDA_MSG_INVALID_KEY, &ca)),
    }
    Err(t.ret(Some(TRAN2), Some(&ca)))
}

fn ta_clear(map: &mut Map) {
    map.cursor("ACTIDIN");
    for f in ["ACTIDIN", "CARDNIN", "CONFIRM"].iter().chain(DATA_FIELDS.iter()) {
        map.set_str(f, " ");
    }
}

/// VALIDATE-INPUT-KEY-FIELDS
fn ta_keys(t: &mut Task, map: &mut Map, ca: &[u8]) -> Cr {
    if !blank(map.get("ACTIDIN")) {
        if !is_numeric(map.get("ACTIDIN"), false) {
            map.cursor("ACTIDIN");
            return Err(ta_send(t, map, "Account ID must be Numeric...", ca));
        }
        let n = fit(text::numval(map.get("ACTIDIN")), Pic::display(11, 0, false));
        let k = format!("{:011}", n);
        map.set_str("ACTIDIN", &k);
        match t.read("CXACAIX", k.as_bytes(), false) {
            Ok(r) => {
                let x = CardXrefRecord::from_bytes(&r);
                map.set("CARDNIN", x.xref_card_num());
            }
            Err(Resp::NotFnd) => {
                map.cursor("ACTIDIN");
                return Err(ta_send(t, map, "Account ID NOT found...", ca));
            }
            Err(_) => {
                map.cursor("ACTIDIN");
                return Err(ta_send(t, map, "Unable to lookup Acct in XREF AIX file...", ca));
            }
        }
    } else if !blank(map.get("CARDNIN")) {
        if !is_numeric(map.get("CARDNIN"), false) {
            map.cursor("CARDNIN");
            return Err(ta_send(t, map, "Card Number must be Numeric...", ca));
        }
        let n = fit(text::numval(map.get("CARDNIN")), Pic::display(16, 0, false));
        let k = format!("{:016}", n);
        map.set_str("CARDNIN", &k);
        match t.read("CCXREF", k.as_bytes(), false) {
            Ok(r) => {
                let x = CardXrefRecord::from_bytes(&r);
                map.set("ACTIDIN", x.xref_acct_id());
            }
            Err(Resp::NotFnd) => {
                map.cursor("CARDNIN");
                return Err(ta_send(t, map, "Card Number NOT found...", ca));
            }
            Err(_) => {
                map.cursor("CARDNIN");
                return Err(ta_send(t, map, "Unable to lookup Card # in XREF file...", ca));
            }
        }
    } else {
        map.cursor("ACTIDIN");
        return Err(ta_send(t, map, "Account or Card Number must be entered...", ca));
    }
    Ok(())
}

fn ta_enter(t: &mut Task, map: &mut Map, ca: &[u8]) -> Cr {
    ta_keys(t, map, ca)?;
    // VALIDATE-INPUT-DATA-FIELDS
    let empty = [
        ("TTYPCD", "Type CD can NOT be empty..."),
        ("TCATCD", "Category CD can NOT be empty..."),
        ("TRNSRC", "Source can NOT be empty..."),
        ("TDESC", "Description can NOT be empty..."),
        ("TRNAMT", "Amount can NOT be empty..."),
        ("TORIGDT", "Orig Date can NOT be empty..."),
        ("TPROCDT", "Proc Date can NOT be empty..."),
        ("MID", "Merchant ID can NOT be empty..."),
        ("MNAME", "Merchant Name can NOT be empty..."),
        ("MCITY", "Merchant City can NOT be empty..."),
        ("MZIP", "Merchant Zip can NOT be empty..."),
    ];
    for (f, m) in empty {
        if blank(map.get(f)) {
            map.cursor(f);
            return Err(ta_send(t, map, m, ca));
        }
    }
    if !is_numeric(map.get("TTYPCD"), false) {
        map.cursor("TTYPCD");
        return Err(ta_send(t, map, "Type CD must be Numeric...", ca));
    }
    if !is_numeric(map.get("TCATCD"), false) {
        map.cursor("TCATCD");
        return Err(ta_send(t, map, "Category CD must be Numeric...", ca));
    }
    let a = map.get("TRNAMT").to_vec();
    if !(a[0] == b'-' || a[0] == b'+') || !is_numeric(&a[1..9], false) || a[9] != b'.' || !is_numeric(&a[10..12], false) {
        map.cursor("TRNAMT");
        return Err(ta_send(t, map, "Amount should be in format -99999999.99", ca));
    }
    for (f, m) in [("TORIGDT", "Orig Date should be in format YYYY-MM-DD"), ("TPROCDT", "Proc Date should be in format YYYY-MM-DD")] {
        let d = map.get(f).to_vec();
        if !is_numeric(&d[0..4], false) || d[4] != b'-' || !is_numeric(&d[5..7], false) || d[7] != b'-' || !is_numeric(&d[8..10], false) {
            map.cursor(f);
            return Err(ta_send(t, map, m, ca));
        }
    }
    let amt = fit(text::numval_c(&a), Pic::display(11, 2, true));
    map.set_str("TRNAMT", &edit(amt, "+99999999.99"));
    for (f, m) in [("TORIGDT", "Orig Date - Not a valid date..."), ("TPROCDT", "Proc Date - Not a valid date...")] {
        let r = csutldtc(&map.get(f)[..10], b"YYYY-MM-DD");
        if r.severity != 0 && r.msg_no != 2513 {
            map.cursor(f);
            return Err(ta_send(t, map, m, ca));
        }
    }
    if !is_numeric(map.get("MID"), false) {
        map.cursor("MID");
        return Err(ta_send(t, map, "Merchant ID must be Numeric...", ca));
    }
    match map.get("CONFIRM")[0] {
        b'Y' | b'y' => ta_add(t, map, ca),
        b'N' | b'n' | b' ' | 0 => {
            map.cursor("CONFIRM");
            Err(ta_send(t, map, "Confirm to add this transaction...", ca))
        }
        _ => {
            map.cursor("CONFIRM");
            Err(ta_send(t, map, "Invalid value. Valid values are (Y/N)...", ca))
        }
    }
}

/// STARTBR HIGH-VALUES / READPREV / ENDBR: the last (highest) transaction.
fn last_tran(t: &mut Task, map: &mut Map, ca: &[u8], cursor_acct: bool) -> Result<TranRecord, Transfer> {
    let _ = cursor_acct;
    let mut br = match t.startbr(FILE, &[0xFF; 16], true) {
        Ok(b) => b,
        Err(Resp::NotFnd) => {
            map.cursor("ACTIDIN");
            return Err(ta_send(t, map, "Transaction ID NOT found...", ca));
        }
        Err(_) => {
            map.cursor("ACTIDIN");
            return Err(ta_send(t, map, "Unable to lookup Transaction...", ca));
        }
    };
    match t.readprev(&mut br) {
        Ok(r) => Ok(TranRecord::from_bytes(&r)),
        Err(Resp::EndFile) => {
            // READPREV INTO TRAN-RECORD did not happen: the area keeps its initial binary zeros
            let mut tr = TranRecord::from_bytes(&[0u8; TranRecord::LEN]);
            tr.set_tran_id(b"0000000000000000");
            Ok(tr)
        }
        Err(_) => {
            map.cursor("ACTIDIN");
            Err(ta_send(t, map, "Unable to lookup Transaction...", ca))
        }
    }
}

fn next_tran_id(last: &TranRecord) -> String {
    let n = fit(cobol::numeric::decode(last.tran_id(), Pic::display(16, 0, false)) + Dec::ONE, Pic::display(16, 0, false));
    format!("{:016}", n)
}

fn ta_add(t: &mut Task, map: &mut Map, ca: &[u8]) -> Cr {
    let last = last_tran(t, map, ca, true)?;
    let id = next_tran_id(&last);
    // INITIALIZE TRAN-RECORD: named fields only; the FILLER keeps what READPREV put there
    let mut tr = last.clone();
    tr.initialize();
    tr.set_tran_id(id.as_bytes());
    tr.set_tran_type_cd(map.get("TTYPCD"));
    tr.set_tran_cat_cd(map.get("TCATCD"));
    tr.set_tran_source(map.get("TRNSRC"));
    tr.set_tran_desc(map.get("TDESC"));
    tr.set_tran_amt_n(fit(text::numval_c(map.get("TRNAMT")), Pic::display(11, 2, true)));
    tr.set_tran_card_num(map.get("CARDNIN"));
    tr.set_tran_merchant_id(map.get("MID"));
    tr.set_tran_merchant_name(map.get("MNAME"));
    tr.set_tran_merchant_city(map.get("MCITY"));
    tr.set_tran_merchant_zip(map.get("MZIP"));
    tr.set_tran_orig_ts(map.get("TORIGDT"));
    tr.set_tran_proc_ts(map.get("TPROCDT"));
    match t.write(FILE, &tr.buf) {
        Ok(()) => {
            ta_clear(map);
            map.color("ERRMSG", Color::Green);
            let msg = format!("Transaction added successfully.  Your Tran ID is {}.", text::s(text::until(tr.tran_id(), b" ")));
            Err(ta_send(t, map, &msg, ca))
        }
        Err(Resp::DupRec) | Err(Resp::DupKey) => {
            map.cursor("ACTIDIN");
            Err(ta_send(t, map, "Tran ID already exist...", ca))
        }
        Err(_) => {
            map.cursor("ACTIDIN");
            Err(ta_send(t, map, "Unable to Add Transaction...", ca))
        }
    }
}

// ===================================================================== COBIL00C
pub fn cobil00c(t: &mut Task) -> Cr {
    const PGM: &str = "COBIL00C";
    const TRAN: &str = "CB00";
    let mut ca = entry(t, TRAN, PGM, CA_LEN)?;
    let mut map = t.map("COBIL0A");
    let mut msg = String::new();
    let mut err = false;
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("ACTIDIN");
        let sel = xget(&ca, X_SEL);
        if !blank(&sel) {
            map.set("ACTIDIN", &sel);
            bil_enter(t, &mut map, &mut msg, &mut err)?;
        }
        bil_send(t, &mut map, &msg);
    } else {
        map = t.receive_map("COBIL0A");
        match t.eibaid {
            Aid::Enter => bil_enter(t, &mut map, &mut msg, &mut err)?,
            Aid::Pf(3) => {
                back_to_caller(&mut ca, "COMEN01C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            Aid::Pf(4) => {
                bil_clear(&mut map);
                bil_send(t, &mut map, "");
            }
            _ => bil_send(t, &mut map, CCDA_MSG_INVALID_KEY),
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca)))
}

fn bil_send(t: &mut Task, map: &mut Map, msg: &str) {
    header(map, "CB00", "COBIL00C");
    map.set_str("ERRMSG", msg);
    t.send_map(map, true);
}
fn bil_clear(map: &mut Map) {
    map.cursor("ACTIDIN");
    for f in ["ACTIDIN", "CURBAL", "CONFIRM"] {
        map.set_str(f, " ");
    }
}

fn bil_enter(t: &mut Task, map: &mut Map, msg: &mut String, err: &mut bool) -> Cr {
    let mut conf_pay = false;
    if blank(map.get("ACTIDIN")) {
        *err = true;
        *msg = "Acct ID can NOT be empty...".into();
        map.cursor("ACTIDIN");
        bil_send(t, map, msg);
    }
    let mut acct = AccountRecord::new();
    if !*err {
        let key = map.get("ACTIDIN").to_vec();
        let mut read = |t: &mut Task, map: &mut Map, msg: &mut String, err: &mut bool| match t.read("ACCTDAT", &key, true) {
            Ok(r) => acct = AccountRecord::from_bytes(&r),
            Err(e) => {
                *err = true;
                *msg = if e == Resp::NotFnd { "Account ID NOT found..." } else { "Unable to lookup Account..." }.into();
                map.cursor("ACTIDIN");
                bil_send(t, map, msg);
            }
        };
        match map.get("CONFIRM")[0] {
            b'Y' | b'y' => {
                conf_pay = true;
                read(t, map, msg, err);
            }
            b'N' | b'n' => {
                bil_clear(map);
                bil_send(t, map, "");
                *err = true;
            }
            b' ' | 0 => read(t, map, msg, err),
            _ => {
                *err = true;
                *msg = "Invalid value. Valid values are (Y/N)...".into();
                map.cursor("CONFIRM");
                bil_send(t, map, msg);
            }
        }
        map.set_str("CURBAL", &edit(acct.acct_curr_bal_n(), "+9999999999.99"));
    }
    if !*err && acct.acct_curr_bal_n() <= Dec::ZERO && !blank(map.get("ACTIDIN")) {
        *err = true;
        *msg = "You have nothing to pay...".into();
        map.cursor("ACTIDIN");
        bil_send(t, map, msg);
    }
    if *err {
        return Ok(());
    }
    if conf_pay {
        // READ-CXACAIX-FILE INTO CARD-XREF-RECORD (WORKING-STORAGE, no VALUE: binary zeros, with
        // XREF-ACCT-ID = the account id moved there as RIDFLD). DUPKEY still returns the first record;
        // NOTFND leaves the area as it was. Either way the message is sent and the payment goes on.
        let mut xref = CardXrefRecord::from_bytes(&[0u8; CardXrefRecord::LEN]);
        xref.set_xref_acct_id(acct.acct_id());
        match t.read_dup("CXACAIX", acct.acct_id()) {
            Ok((r, false)) => xref = CardXrefRecord::from_bytes(&r),
            Ok((r, true)) => {
                xref = CardXrefRecord::from_bytes(&r);
                *msg = "Unable to lookup XREF AIX file...".into();
                map.cursor("ACTIDIN");
                bil_send(t, map, msg);
            }
            Err(e) => {
                *msg = if e == Resp::NotFnd { "Account ID NOT found..." } else { "Unable to lookup XREF AIX file..." }.into();
                map.cursor("ACTIDIN");
                bil_send(t, map, msg);
            }
        }
        // STARTBR (HIGH-VALUES) / READPREV / ENDBR. ENDBR has no RESP: when STARTBR failed there is no
        // browse to end, CICS raises INVREQ and the task abends AEIP (as KD-10).
        let mut last = TranRecord::from_bytes(&[0u8; TranRecord::LEN]);
        match t.startbr(FILE, &[0xFF; 16], true) {
            Ok(mut b) => match t.readprev(&mut b) {
                Ok(r) => last = TranRecord::from_bytes(&r),
                Err(Resp::EndFile) => last.set_tran_id(b"0000000000000000"),
                Err(_) => {
                    *msg = "Unable to lookup Transaction...".into();
                    map.cursor("ACTIDIN");
                    bil_send(t, map, msg);
                    last.set_tran_id(&[0xFF; 16]);
                }
            },
            Err(e) => {
                *msg = if e == Resp::NotFnd { "Transaction ID NOT found..." } else { "Unable to lookup Transaction..." }.into();
                map.cursor("ACTIDIN");
                bil_send(t, map, msg);
                // READPREV without a browse: INVREQ -> 'Unable to lookup Transaction...'; then ENDBR abends
                return Err(t.abend("AEIP"));
            }
        }
        let id = next_tran_id(&last);
        // INITIALIZE TRAN-RECORD: named fields only; the FILLER keeps the READPREV data (or zeros)
        let mut tr = last.clone();
        tr.initialize();
        tr.set_tran_id(id.as_bytes());
        tr.set_tran_type_cd(b"02");
        tr.set_tran_cat_cd_n(Dec::from(2));
        tr.set_tran_source(b"POS TERM");
        tr.set_tran_desc(b"BILL PAYMENT - ONLINE");
        tr.set_tran_amt_n(acct.acct_curr_bal_n());
        tr.set_tran_card_num(xref.xref_card_num());
        tr.set_tran_merchant_id_n(Dec::from(999_999_999));
        tr.set_tran_merchant_name(b"BILL PAYMENT");
        tr.set_tran_merchant_city(b"N/A");
        tr.set_tran_merchant_zip(b"N/A");
        let now = now();
        let ts = format!("{} {:02}:{:02}:{:02}.000000", now.format("%Y-%m-%d"), now.hour(), now.minute(), now.second());
        tr.set_tran_orig_ts(ts.as_bytes());
        tr.set_tran_proc_ts(ts.as_bytes());
        match t.write(FILE, &tr.buf) {
            Ok(()) => {
                bil_clear(map);
                map.color("ERRMSG", Color::Green);
                *msg = format!("Payment successful.  Your Transaction ID is {}.", text::s(text::until(tr.tran_id(), b" ")));
                bil_send(t, map, msg);
            }
            Err(e) => {
                *msg = if matches!(e, Resp::DupRec | Resp::DupKey) { "Tran ID already exist..." } else { "Unable to Add Bill pay Transaction..." }.into();
                map.cursor("ACTIDIN");
                bil_send(t, map, msg);
            }
        }
        acct.set_acct_curr_bal_n(acct.acct_curr_bal_n() - tr.tran_amt_n());
        // UPDATE-ACCTDAT-FILE
        if let Err(e) = t.rewrite("ACCTDAT", &acct.buf) {
            *msg = if e == Resp::NotFnd { "Account ID NOT found..." } else { "Unable to Update Account..." }.into();
            map.cursor("ACTIDIN");
            bil_send(t, map, msg);
        }
    } else {
        *msg = "Confirm to make a bill payment...".into();
        map.cursor("CONFIRM");
    }
    bil_send(t, map, msg);
    Ok(())
}

// ===================================================================== CORPT00C
// Every SEND in this program is followed by RETURN TRANSID (BR-RPT-0).
const PGMR: &str = "CORPT00C";
const TRANR: &str = "CR00";

fn rp_send(t: &mut Task, map: &mut Map, msg: &str, ca: &[u8]) -> Transfer {
    header(map, TRANR, PGMR);
    map.set_str("ERRMSG", msg);
    t.send_map(map, true);
    t.ret(Some(TRANR), Some(ca))
}

fn rp_clear(map: &mut Map) {
    map.cursor("MONTHLY");
    for f in ["MONTHLY", "YEARLY", "CUSTOM", "SDTMM", "SDTDD", "SDTYYYY", "EDTMM", "EDTDD", "EDTYYYY", "CONFIRM"] {
        map.set_str(f, " ");
    }
}

/// The JCL deck CORPT00C writes to the internal reader (TD queue JOBS).
pub fn report_jcl(start: &str, end: &str) -> Vec<String> {
    vec![
        "//TRNRPT00 JOB 'TRAN REPORT',CLASS=A,MSGCLASS=0,".into(),
        "// NOTIFY=&SYSUID".into(),
        "//*".into(),
        "//JOBLIB JCLLIB ORDER=('AWS.M2.CARDDEMO.PROC')".into(),
        "//*".into(),
        "//STEP10 EXEC PROC=TRANREPT".into(),
        "//*".into(),
        "//STEP05R.SYMNAMES DD *".into(),
        "TRAN-CARD-NUM,263,16,ZD".into(),
        "TRAN-PROC-DT,305,10,CH".into(),
        format!("PARM-START-DATE,C'{start:<10}'"),
        format!("PARM-END-DATE,C'{end:<10}'"),
        "/*".into(),
        "//STEP10R.DATEPARM DD *".into(),
        format!("{start:<10} {end:<10}"),
        "/*".into(),
        "/*EOF".into(),
    ]
}

pub fn corpt00c(t: &mut Task) -> Cr {
    let mut ca = entry(t, TRANR, PGMR, CarddemoCommarea::LEN)?;
    let mut map = t.map("CORPT0A");
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("MONTHLY");
        return Err(rp_send(t, &mut map, "", &ca));
    }
    map = t.receive_map("CORPT0A");
    match t.eibaid {
        Aid::Enter => {}
        Aid::Pf(3) => {
            set_to(&mut ca, "COMEN01C");
            return Err(return_to_prev(t, &mut ca, TRANR, PGMR));
        }
        _ => {
            map.cursor("MONTHLY");
            return Err(rp_send(t, &mut map, CCDA_MSG_INVALID_KEY, &ca));
        }
    }
    let today = now().date();
    let (name, start, end);
    if !blank(map.get("MONTHLY")) {
        name = "Monthly";
        let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
        let next = if today.month() == 12 {
            NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
        };
        start = first.format("%Y-%m-%d").to_string();
        end = (next - Duration::days(1)).format("%Y-%m-%d").to_string();
    } else if !blank(map.get("YEARLY")) {
        name = "Yearly";
        start = format!("{:04}-01-01", today.year());
        end = format!("{:04}-12-31", today.year());
    } else if !blank(map.get("CUSTOM")) {
        for (f, m) in [
            ("SDTMM", "Start Date - Month can NOT be empty..."),
            ("SDTDD", "Start Date - Day can NOT be empty..."),
            ("SDTYYYY", "Start Date - Year can NOT be empty..."),
            ("EDTMM", "End Date - Month can NOT be empty..."),
            ("EDTDD", "End Date - Day can NOT be empty..."),
            ("EDTYYYY", "End Date - Year can NOT be empty..."),
        ] {
            if blank(map.get(f)) {
                map.cursor(f);
                return Err(rp_send(t, &mut map, m, &ca));
            }
        }
        // NUMVAL-C normalisation into 99 / 9999
        for (f, w) in [("SDTMM", 2), ("SDTDD", 2), ("SDTYYYY", 4), ("EDTMM", 2), ("EDTDD", 2), ("EDTYYYY", 4)] {
            let v = fit(text::numval_c(map.get(f)), Pic::display(w, 0, false));
            map.set_str(f, &format!("{:0w$}", v, w = w as usize));
        }
        for (f, lim, m) in [
            ("SDTMM", Some(&b"12"[..]), "Start Date - Not a valid Month..."),
            ("SDTDD", Some(&b"31"[..]), "Start Date - Not a valid Day..."),
            ("SDTYYYY", None, "Start Date - Not a valid Year..."),
            ("EDTMM", Some(&b"12"[..]), "End Date - Not a valid Month..."),
            ("EDTDD", Some(&b"31"[..]), "End Date - Not a valid Day..."),
            ("EDTYYYY", None, "End Date - Not a valid Year..."),
        ] {
            let v = map.get(f).to_vec();
            if !is_numeric(&v, false) || lim.is_some_and(|l| &v[..] > l) {
                map.cursor(f);
                return Err(rp_send(t, &mut map, m, &ca));
            }
        }
        start = format!("{}-{}-{}", text::s(map.get("SDTYYYY")), text::s(map.get("SDTMM")), text::s(map.get("SDTDD")));
        end = format!("{}-{}-{}", text::s(map.get("EDTYYYY")), text::s(map.get("EDTMM")), text::s(map.get("EDTDD")));
        for (d, f, m) in [(&start, "SDTMM", "Start Date - Not a valid date..."), (&end, "EDTMM", "End Date - Not a valid date...")] {
            let r = csutldtc(d.as_bytes(), b"YYYY-MM-DD");
            if r.severity != 0 && r.msg_no != 2513 {
                map.cursor(f);
                return Err(rp_send(t, &mut map, m, &ca));
            }
        }
        name = "Custom";
    } else {
        map.cursor("MONTHLY");
        return Err(rp_send(t, &mut map, "Select a report type to print report...", &ca));
    }
    // SUBMIT-JOB-TO-INTRDR
    let conf = map.get("CONFIRM").to_vec();
    if blank(&conf) {
        map.cursor("CONFIRM");
        let m = format!("Please confirm to print the {name} report...");
        return Err(rp_send(t, &mut map, &m, &ca));
    }
    match conf[0] {
        b'Y' | b'y' => {}
        b'N' | b'n' => {
            rp_clear(&mut map);
            return Err(rp_send(t, &mut map, "", &ca));
        }
        _ => {
            map.cursor("CONFIRM");
            let m = format!("\"{}\" is not a valid value to confirm...", text::s(text::until(&conf, b" ")));
            return Err(rp_send(t, &mut map, &m, &ca));
        }
    }
    for line in report_jcl(&start, &end) {
        let mut rec = [b' '; 80];
        text::mv(&mut rec, line.as_bytes());
        t.writeq_td("JOBS", &rec);
    }
    rp_clear(&mut map);
    map.color("ERRMSG", Color::Green);
    map.cursor("MONTHLY");
    let m = format!("{name} report submitted for printing ...");
    Err(rp_send(t, &mut map, &m, &ca))
}
