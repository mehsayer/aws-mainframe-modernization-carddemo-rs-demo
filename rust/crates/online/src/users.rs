//! User administration: COUSR00C (CU00 list), COUSR01C (CU01 add), COUSR02C (CU02
//! update), COUSR03C (CU03 delete). Requirements FR-USR-LST/ADD/UPD/DEL.

use crate::common::*;
use cics::{Aid, Color, Cr, Map, Task};
use cobol::copybooks::{CarddemoCommarea, SecUserData};
use cobol::text;
use vsam::{Browse, Resp};

const FILE: &str = "USRSEC";
/// CDEMO-CU0x-INFO extension: USRID-FIRST(8) USRID-LAST(8) PAGE-NUM 9(8)
/// NEXT-PAGE-FLG(1) USR-SEL-FLG(1) USR-SELECTED(8)
const EXT_LEN: usize = 34;
const CA_LEN: usize = CarddemoCommarea::LEN + EXT_LEN;
const X_FIRST: (usize, usize) = (0, 8);
const X_LAST: (usize, usize) = (8, 8);
const X_PAGE: (usize, usize) = (16, 8);
const X_NEXT: (usize, usize) = (24, 1);
const X_SELFLG: (usize, usize) = (25, 1);
const X_SEL: (usize, usize) = (26, 8);

fn xget(ca: &[u8], f: (usize, usize)) -> Vec<u8> {
    ext(ca, f.0, f.1).to_vec()
}
fn xset(ca: &mut [u8], f: (usize, usize), v: &[u8]) {
    set_ext(ca, f.0, f.1, v)
}

/// RETURN-TO-PREV-SCREEN (common to the four programs)
fn return_to_prev(t: &Task, ca: &mut [u8], tran: &str, pgm: &str) -> cics::Transfer {
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

fn set_to(ca: &mut [u8], pgm: &str) {
    let mut c = cdemo(ca);
    c.set_cdemo_to_program(pgm.as_bytes());
    put_cdemo(ca, &c);
}

/// Common entry: EIBCALEN = 0 -> sign-on; otherwise load the commarea.
fn entry(t: &Task, tran: &str, pgm: &str) -> Result<Vec<u8>, cics::Transfer> {
    let mut ca = vec![b' '; CA_LEN];
    if t.eibcalen == 0 {
        set_to(&mut ca, "COSGN00C");
        return Err(return_to_prev(t, &mut ca, tran, pgm));
    }
    Ok(load_commarea(&t.commarea, CA_LEN))
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

// ===================================================================== COUSR00C
struct ListState {
    err: bool,
    eof: bool,
    message: String,
    erase: bool,
}

const PGM0: &str = "COUSR00C";
const TRAN0: &str = "CU00";

pub fn cousr00c(t: &mut Task) -> Cr {
    let mut st = ListState { err: false, eof: false, message: String::new(), erase: true };
    let mut ca = entry(t, TRAN0, PGM0)?;
    let mut map = t.map("COUSR0A");
    map.cursor("USRIDIN");
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("USRIDIN");
        list_enter(t, &mut ca, &mut map, &mut st)?;
        send_list(t, &mut map, &st);
    } else {
        map = t.receive_map("COUSR0A");
        map.cursor("USRIDIN");
        match t.eibaid {
            Aid::Enter => list_enter(t, &mut ca, &mut map, &mut st)?,
            Aid::Pf(3) => {
                set_to(&mut ca, "COADM01C");
                return Err(return_to_prev(t, &mut ca, TRAN0, PGM0));
            }
            Aid::Pf(7) => list_pf7(t, &mut ca, &mut map, &mut st)?,
            Aid::Pf(8) => list_pf8(t, &mut ca, &mut map, &mut st)?,
            _ => {
                st.err = true;
                map.cursor("USRIDIN");
                st.message = CCDA_MSG_INVALID_KEY.into();
                send_list(t, &mut map, &st);
            }
        }
    }
    Err(t.ret(Some(TRAN0), Some(&ca)))
}

fn send_list(t: &mut Task, map: &mut Map, st: &ListState) {
    header(map, TRAN0, PGM0);
    map.set_str("ERRMSG", &st.message);
    t.send_map(map, st.erase);
}

fn list_enter(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut ListState) -> Cr {
    // first row with a selection code wins
    let mut sel_flg = b" ".to_vec();
    let mut selected = vec![b' '; 8];
    for i in 1..=10 {
        let s = map.get(&format!("SEL{i:04}")).to_vec();
        if !blank(&s) {
            sel_flg = s;
            selected = map.get(&format!("USRID{i:02}")).to_vec();
            break;
        }
    }
    xset(ca, X_SELFLG, &sel_flg);
    xset(ca, X_SEL, &selected);
    let f = xget(ca, X_SELFLG);
    let s = xget(ca, X_SEL);
    if !blank(&f) && !blank(&s) {
        match f[0] {
            b'U' | b'u' | b'D' | b'd' => {
                let to = if f[0].eq_ignore_ascii_case(&b'U') { "COUSR02C" } else { "COUSR03C" };
                let mut c = cdemo(ca);
                c.set_cdemo_to_program(to.as_bytes());
                c.set_cdemo_from_tranid(TRAN0.as_bytes());
                c.set_cdemo_from_program(PGM0.as_bytes());
                c.set_cdemo_pgm_context(b"0");
                put_cdemo(ca, &c);
                return Err(t.xctl(to, Some(ca)));
            }
            _ => {
                st.message = "Invalid selection. Valid values are U and D".into();
                map.cursor("USRIDIN");
            }
        }
    }
    let key = if blank(map.get("USRIDIN")) { vec![0u8; 8] } else { map.get("USRIDIN").to_vec() };
    map.cursor("USRIDIN");
    xset(ca, X_PAGE, b"00000000");
    page_forward(t, ca, map, st, key)?;
    if !st.err {
        map.set_str("USRIDIN", " ");
    }
    Ok(())
}

fn list_pf7(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut ListState) -> Cr {
    let first = xget(ca, X_FIRST);
    let key = if blank(&first) { vec![0u8; 8] } else { first };
    xset(ca, X_NEXT, b"Y");
    map.cursor("USRIDIN");
    if page_num(ca) > 1 {
        page_backward(t, ca, map, st, key)?;
    } else {
        st.message = "You are already at the top of the page...".into();
        st.erase = false;
        send_list(t, map, st);
    }
    Ok(())
}

fn list_pf8(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut ListState) -> Cr {
    let last = xget(ca, X_LAST);
    let key = if blank(&last) { vec![0xFFu8; 8] } else { last };
    map.cursor("USRIDIN");
    if xget(ca, X_NEXT) == b"Y" {
        page_forward(t, ca, map, st, key)?;
    } else {
        st.message = "You are already at the bottom of the page...".into();
        st.erase = false;
        send_list(t, map, st);
    }
    Ok(())
}

fn page_num(ca: &[u8]) -> u64 {
    text::s(ext(ca, X_PAGE.0, X_PAGE.1)).trim().parse().unwrap_or(0)
}
fn set_page_num(ca: &mut [u8], n: u64) {
    xset(ca, X_PAGE, format!("{:08}", n % 100_000_000).as_bytes());
}

fn startbr(t: &mut Task, map: &mut Map, st: &mut ListState, key: &[u8]) -> Option<Browse> {
    match t.startbr(FILE, key, true) {
        Ok(b) => Some(b),
        Err(Resp::NotFnd) => {
            st.eof = true;
            st.message = "You are at the top of the page...".into();
            map.cursor("USRIDIN");
            send_list(t, map, st);
            None
        }
        Err(_) => {
            st.err = true;
            st.message = "Unable to lookup User...".into();
            map.cursor("USRIDIN");
            send_list(t, map, st);
            None
        }
    }
}

fn read_dir(t: &mut Task, map: &mut Map, st: &mut ListState, br: &mut Option<Browse>, fwd: bool) -> Option<SecUserData> {
    let r = match br.as_mut() {
        Some(b) => {
            if fwd {
                t.readnext(b)
            } else {
                t.readprev(b)
            }
        }
        None => Err(Resp::InvReq),
    };
    match r {
        Ok(rec) => Some(SecUserData::from_bytes(&rec)),
        Err(Resp::EndFile) => {
            st.eof = true;
            st.message = if fwd { "You have reached the bottom of the page..." } else { "You have reached the top of the page..." }.into();
            map.cursor("USRIDIN");
            send_list(t, map, st);
            None
        }
        Err(_) => {
            st.err = true;
            st.message = "Unable to lookup User...".into();
            map.cursor("USRIDIN");
            send_list(t, map, st);
            None
        }
    }
}

/// ENDBR without RESP: no active browse raises INVREQ -> AEIP abend (KD-10).
fn endbr(t: &Task, br: Option<Browse>) -> Cr {
    match br {
        Some(_) => Ok(()),
        None => Err(t.abend("AEIP")),
    }
}

fn populate(map: &mut Map, ca: &mut [u8], idx: usize, u: &SecUserData) {
    map.set(&format!("USRID{idx:02}"), u.sec_usr_id());
    map.set(&format!("FNAME{idx:02}"), u.sec_usr_fname());
    map.set(&format!("LNAME{idx:02}"), u.sec_usr_lname());
    map.set(&format!("UTYPE{idx:02}"), u.sec_usr_type());
    if idx == 1 {
        xset(ca, X_FIRST, u.sec_usr_id());
    }
    if idx == 10 {
        xset(ca, X_LAST, u.sec_usr_id());
    }
}

fn init_rows(map: &mut Map) {
    for i in 1..=10 {
        for f in ["USRID", "FNAME", "LNAME", "UTYPE"] {
            map.set_str(&format!("{f}{i:02}"), " ");
        }
    }
}

fn page_forward(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut ListState, key: Vec<u8>) -> Cr {
    let mut br = startbr(t, map, st, &key);
    if st.err {
        return Ok(());
    }
    if t.eibaid != Aid::Enter && t.eibaid != Aid::Pf(7) && t.eibaid != Aid::Pf(3) {
        read_dir(t, map, st, &mut br, true);
    }
    if !st.eof && !st.err {
        init_rows(map);
    }
    let mut idx = 1;
    while !(idx >= 11 || st.eof || st.err) {
        if let Some(u) = read_dir(t, map, st, &mut br, true) {
            populate(map, ca, idx, &u);
            idx += 1;
        }
    }
    if !st.eof && !st.err {
        { let n = page_num(ca) + 1; set_page_num(ca, n); }
        let more = read_dir(t, map, st, &mut br, true).is_some();
        xset(ca, X_NEXT, if more && !st.err { b"Y" } else { b"N" });
    } else {
        xset(ca, X_NEXT, b"N");
        if idx > 1 {
            { let n = page_num(ca) + 1; set_page_num(ca, n); }
        }
    }
    endbr(t, br)?;
    map.set_str("PAGENUM", &format!("{:08}", page_num(ca)));
    map.set_str("USRIDIN", " ");
    send_list(t, map, st);
    Ok(())
}

fn page_backward(t: &mut Task, ca: &mut [u8], map: &mut Map, st: &mut ListState, key: Vec<u8>) -> Cr {
    let mut br = startbr(t, map, st, &key);
    if st.err {
        return Ok(());
    }
    if t.eibaid != Aid::Enter && t.eibaid != Aid::Pf(8) {
        read_dir(t, map, st, &mut br, false);
    }
    if !st.eof && !st.err {
        init_rows(map);
    }
    let mut idx: i32 = 10;
    while !(idx <= 0 || st.eof || st.err) {
        if let Some(u) = read_dir(t, map, st, &mut br, false) {
            populate(map, ca, idx as usize, &u);
            idx -= 1;
        }
    }
    if !st.eof && !st.err {
        let more = read_dir(t, map, st, &mut br, false).is_some();
        if xget(ca, X_NEXT) == b"Y" {
            if more && !st.err && page_num(ca) > 1 {
                { let n = page_num(ca) - 1; set_page_num(ca, n); }
            } else {
                set_page_num(ca, 1);
            }
        }
    }
    endbr(t, br)?;
    map.set_str("PAGENUM", &format!("{:08}", page_num(ca)));
    send_list(t, map, st);
    Ok(())
}

// ===================================================================== COUSR01C
pub fn cousr01c(t: &mut Task) -> Cr {
    const PGM: &str = "COUSR01C";
    const TRAN: &str = "CU01";
    let mut ca = entry(t, TRAN, PGM)?;
    let mut map = t.map("COUSR1A");
    let send = |t: &mut Task, map: &mut Map, msg: &str| {
        header(map, TRAN, PGM);
        map.set_str("ERRMSG", msg);
        t.send_map(map, true);
    };
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("FNAME");
        send(t, &mut map, "");
    } else {
        map = t.receive_map("COUSR1A");
        match t.eibaid {
            Aid::Enter => {
                let checks = [
                    ("FNAME", "First Name can NOT be empty..."),
                    ("LNAME", "Last Name can NOT be empty..."),
                    ("USERID", "User ID can NOT be empty..."),
                    ("PASSWD", "Password can NOT be empty..."),
                    ("USRTYPE", "User Type can NOT be empty..."),
                ];
                let mut err = false;
                for (f, m) in checks {
                    if blank(map.get(f)) {
                        err = true;
                        map.cursor(f);
                        send(t, &mut map, m);
                        break;
                    }
                }
                if !err {
                    map.cursor("FNAME");
                    let mut u = SecUserData::new();
                    u.set_sec_usr_id(map.get("USERID"));
                    u.set_sec_usr_fname(map.get("FNAME"));
                    u.set_sec_usr_lname(map.get("LNAME"));
                    u.set_sec_usr_pwd(map.get("PASSWD"));
                    u.set_sec_usr_type(map.get("USRTYPE"));
                    match t.write(FILE, &u.buf) {
                        Ok(()) => {
                            init_add(&mut map);
                            map.color("ERRMSG", Color::Green);
                            let msg = format!("User {} has been added ...", text::s(text::until(u.sec_usr_id(), b" ")));
                            send(t, &mut map, &msg);
                        }
                        Err(Resp::DupRec) | Err(Resp::DupKey) => {
                            map.cursor("USERID");
                            send(t, &mut map, "User ID already exist...");
                        }
                        Err(_) => {
                            map.cursor("FNAME");
                            send(t, &mut map, "Unable to Add User...");
                        }
                    }
                }
            }
            Aid::Pf(3) => {
                set_to(&mut ca, "COADM01C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            Aid::Pf(4) => {
                init_add(&mut map);
                send(t, &mut map, "");
            }
            _ => {
                map.cursor("FNAME");
                send(t, &mut map, CCDA_MSG_INVALID_KEY);
            }
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca)))
}

fn init_add(map: &mut Map) {
    map.cursor("FNAME");
    for f in ["USERID", "FNAME", "LNAME", "PASSWD", "USRTYPE"] {
        map.set_str(f, " ");
    }
}

// ===================================================================== COUSR02C / COUSR03C
struct UpdState {
    err: bool,
    message: String,
}

fn send_upd(t: &mut Task, map: &mut Map, tran: &str, pgm: &str, st: &UpdState) {
    header(map, tran, pgm);
    map.set_str("ERRMSG", &st.message);
    t.send_map(map, true);
}

/// READ-USER-SEC-FILE (READ UPDATE) shared by update and delete
fn read_user(t: &mut Task, map: &mut Map, tran: &str, pgm: &str, st: &mut UpdState, id: &[u8], prompt: &str) -> Option<SecUserData> {
    match t.read(FILE, id, true) {
        Ok(r) => {
            st.message = prompt.into();
            map.color("ERRMSG", Color::Neutral);
            send_upd(t, map, tran, pgm, st);
            Some(SecUserData::from_bytes(&r))
        }
        Err(Resp::NotFnd) => {
            st.err = true;
            st.message = "User ID NOT found...".into();
            map.cursor("USRIDIN");
            send_upd(t, map, tran, pgm, st);
            None
        }
        Err(e) => {
            t.display(format!("RESP:{:09}REAS:{:09}", e.code(), e.code2()));
            st.err = true;
            st.message = "Unable to lookup User...".into();
            map.cursor("FNAME");
            send_upd(t, map, tran, pgm, st);
            None
        }
    }
}

pub fn cousr02c(t: &mut Task) -> Cr {
    const PGM: &str = "COUSR02C";
    const TRAN: &str = "CU02";
    let mut st = UpdState { err: false, message: String::new() };
    let mut ca = entry(t, TRAN, PGM)?;
    let mut map = t.map("COUSR2A");
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("USRIDIN");
        let sel = xget(&ca, X_SEL);
        if !blank(&sel) {
            map.set("USRIDIN", &sel);
            upd_enter(t, &mut map, &mut st, TRAN, PGM);
        }
        send_upd(t, &mut map, TRAN, PGM, &st);
    } else {
        map = t.receive_map("COUSR2A");
        match t.eibaid {
            Aid::Enter => upd_enter(t, &mut map, &mut st, TRAN, PGM),
            Aid::Pf(3) => {
                upd_save(t, &mut map, &mut st, TRAN, PGM);
                let from = cdemo(&ca).cdemo_from_program().to_vec();
                set_to(&mut ca, if blank(&from) { "COADM01C" } else { std::str::from_utf8(&from).unwrap_or("COADM01C") });
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            Aid::Pf(4) => {
                clear_upd(&mut map, true);
                st.message.clear();
                send_upd(t, &mut map, TRAN, PGM, &st);
            }
            Aid::Pf(5) => upd_save(t, &mut map, &mut st, TRAN, PGM),
            Aid::Pf(12) => {
                set_to(&mut ca, "COADM01C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            _ => {
                st.err = true;
                st.message = CCDA_MSG_INVALID_KEY.into();
                send_upd(t, &mut map, TRAN, PGM, &st);
            }
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca)))
}

fn clear_upd(map: &mut Map, with_pwd: bool) {
    map.cursor("USRIDIN");
    map.set_str("USRIDIN", " ");
    for f in ["FNAME", "LNAME", "USRTYPE"] {
        map.set_str(f, " ");
    }
    if with_pwd {
        map.set_str("PASSWD", " ");
    }
}

fn upd_enter(t: &mut Task, map: &mut Map, st: &mut UpdState, tran: &str, pgm: &str) {
    if blank(map.get("USRIDIN")) {
        st.err = true;
        st.message = "User ID can NOT be empty...".into();
        map.cursor("USRIDIN");
        send_upd(t, map, tran, pgm, st);
    } else {
        map.cursor("USRIDIN");
    }
    if !st.err {
        for f in ["FNAME", "LNAME", "PASSWD", "USRTYPE"] {
            map.set_str(f, " ");
        }
        let id = map.get("USRIDIN").to_vec();
        if let Some(u) = read_user(t, map, tran, pgm, st, &id, "Press PF5 key to save your updates ...") {
            if !st.err {
                map.set("FNAME", u.sec_usr_fname());
                map.set("LNAME", u.sec_usr_lname());
                map.set("PASSWD", u.sec_usr_pwd());
                map.set("USRTYPE", u.sec_usr_type());
                send_upd(t, map, tran, pgm, st);
            }
        }
    }
}

fn upd_save(t: &mut Task, map: &mut Map, st: &mut UpdState, tran: &str, pgm: &str) {
    let checks = [
        ("USRIDIN", "User ID can NOT be empty..."),
        ("FNAME", "First Name can NOT be empty..."),
        ("LNAME", "Last Name can NOT be empty..."),
        ("PASSWD", "Password can NOT be empty..."),
        ("USRTYPE", "User Type can NOT be empty..."),
    ];
    for (f, m) in checks {
        if blank(map.get(f)) {
            st.err = true;
            st.message = m.into();
            map.cursor(f);
            send_upd(t, map, tran, pgm, st);
            return;
        }
    }
    map.cursor("FNAME");
    let id = map.get("USRIDIN").to_vec();
    let Some(mut u) = read_user(t, map, tran, pgm, st, &id, "Press PF5 key to save your updates ...") else { return };
    let mut modified = false;
    for (f, cur, set) in [
        ("FNAME", u.sec_usr_fname().to_vec(), 0),
        ("LNAME", u.sec_usr_lname().to_vec(), 1),
        ("PASSWD", u.sec_usr_pwd().to_vec(), 2),
        ("USRTYPE", u.sec_usr_type().to_vec(), 3),
    ] {
        let v = map.get(f).to_vec();
        let mut padded = vec![b' '; cur.len()];
        text::mv(&mut padded, &v);
        if padded != cur {
            modified = true;
            match set {
                0 => u.set_sec_usr_fname(&v),
                1 => u.set_sec_usr_lname(&v),
                2 => u.set_sec_usr_pwd(&v),
                _ => u.set_sec_usr_type(&v),
            }
        }
    }
    if modified {
        match t.rewrite(FILE, &u.buf) {
            Ok(()) => {
                st.message = format!("User {} has been updated ...", text::s(text::until(u.sec_usr_id(), b" ")));
                map.color("ERRMSG", Color::Green);
                send_upd(t, map, tran, pgm, st);
            }
            Err(Resp::NotFnd) => {
                st.err = true;
                st.message = "User ID NOT found...".into();
                map.cursor("USRIDIN");
                send_upd(t, map, tran, pgm, st);
            }
            Err(_) => {
                st.err = true;
                st.message = "Unable to Update User...".into();
                map.cursor("FNAME");
                send_upd(t, map, tran, pgm, st);
            }
        }
    } else {
        st.message = "Please modify to update ...".into();
        map.color("ERRMSG", Color::Red);
        send_upd(t, map, tran, pgm, st);
    }
}

pub fn cousr03c(t: &mut Task) -> Cr {
    const PGM: &str = "COUSR03C";
    const TRAN: &str = "CU03";
    let mut st = UpdState { err: false, message: String::new() };
    let mut ca = entry(t, TRAN, PGM)?;
    let mut map = t.map("COUSR3A");
    if first_entry(&mut ca) {
        map.clear();
        map.cursor("USRIDIN");
        let sel = xget(&ca, X_SEL);
        if !blank(&sel) {
            map.set("USRIDIN", &sel);
            del_enter(t, &mut map, &mut st, TRAN, PGM);
        }
        send_upd(t, &mut map, TRAN, PGM, &st);
    } else {
        map = t.receive_map("COUSR3A");
        match t.eibaid {
            Aid::Enter => del_enter(t, &mut map, &mut st, TRAN, PGM),
            Aid::Pf(3) => {
                let from = cdemo(&ca).cdemo_from_program().to_vec();
                set_to(&mut ca, if blank(&from) { "COADM01C" } else { std::str::from_utf8(&from).unwrap_or("COADM01C") });
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            Aid::Pf(4) => {
                clear_upd(&mut map, false);
                st.message.clear();
                send_upd(t, &mut map, TRAN, PGM, &st);
            }
            Aid::Pf(5) => del_user(t, &mut map, &mut st, TRAN, PGM),
            Aid::Pf(12) => {
                set_to(&mut ca, "COADM01C");
                return Err(return_to_prev(t, &mut ca, TRAN, PGM));
            }
            _ => {
                st.err = true;
                st.message = CCDA_MSG_INVALID_KEY.into();
                send_upd(t, &mut map, TRAN, PGM, &st);
            }
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca)))
}

fn del_enter(t: &mut Task, map: &mut Map, st: &mut UpdState, tran: &str, pgm: &str) {
    if blank(map.get("USRIDIN")) {
        st.err = true;
        st.message = "User ID can NOT be empty...".into();
        map.cursor("USRIDIN");
        send_upd(t, map, tran, pgm, st);
    } else {
        map.cursor("USRIDIN");
    }
    if !st.err {
        for f in ["FNAME", "LNAME", "USRTYPE"] {
            map.set_str(f, " ");
        }
        let id = map.get("USRIDIN").to_vec();
        if let Some(u) = read_user(t, map, tran, pgm, st, &id, "Press PF5 key to delete this user ...") {
            map.set("FNAME", u.sec_usr_fname());
            map.set("LNAME", u.sec_usr_lname());
            map.set("USRTYPE", u.sec_usr_type());
            send_upd(t, map, tran, pgm, st);
        }
    }
}

fn del_user(t: &mut Task, map: &mut Map, st: &mut UpdState, tran: &str, pgm: &str) {
    if blank(map.get("USRIDIN")) {
        st.err = true;
        st.message = "User ID can NOT be empty...".into();
        map.cursor("USRIDIN");
        send_upd(t, map, tran, pgm, st);
        return;
    }
    let id = map.get("USRIDIN").to_vec();
    let Some(u) = read_user(t, map, tran, pgm, st, &id, "Press PF5 key to delete this user ...") else { return };
    match t.delete(FILE, None) {
        Ok(()) => {
            clear_upd(map, false);
            st.message = format!("User {} has been deleted ...", text::s(text::until(u.sec_usr_id(), b" ")));
            map.color("ERRMSG", Color::Green);
            send_upd(t, map, tran, pgm, st);
        }
        Err(Resp::NotFnd) => {
            st.err = true;
            st.message = "User ID NOT found...".into();
            map.cursor("USRIDIN");
            send_upd(t, map, tran, pgm, st);
        }
        Err(_) => {
            st.err = true;
            st.message = "Unable to Update User...".into();
            map.cursor("FNAME");
            send_upd(t, map, tran, pgm, st);
        }
    }
}
