//! COSGN00C (CC00) sign-on, COMEN01C (CM00) main menu, COADM01C (CA00) admin menu.
//! Requirements FR-SGN-01..07, FR-MEN-01..07, FR-ADM-01..04.

use crate::common::*;
use cics::{Aid, Color, Cr, Task};
use cobol::copybooks::{CarddemoCommarea, SecUserData};
use cobol::text;

// ===================================================================== COSGN00C
pub fn cosgn00c(t: &mut Task) -> Cr {
    const PGM: &str = "COSGN00C";
    const TRAN: &str = "CC00";
    let mut ca = CarddemoCommarea::new();
    let mut map = t.map("COSGN0A");
    let mut message = String::new();

    if t.eibcalen == 0 {
        map.clear();
        map.cursor("USERID");
        send_signon(t, &mut map, &message);
    } else {
        match t.eibaid {
            Aid::Enter => {
                map = t.receive_map("COSGN0A");
                let mut err = false;
                if blank(map.get("USERID")) {
                    err = true;
                    message = "Please enter User ID ...".into();
                    map.cursor("USERID");
                    send_signon(t, &mut map, &message);
                } else if blank(map.get("PASSWD")) {
                    err = true;
                    message = "Please enter Password ...".into();
                    map.cursor("PASSWD");
                    send_signon(t, &mut map, &message);
                }
                let user_id = text::upper(map.get("USERID"));
                let user_pwd = text::upper(map.get("PASSWD"));
                ca.set_cdemo_user_id(&user_id);
                if !err {
                    // READ-USER-SEC-FILE
                    match t.read("USRSEC", &user_id, false) {
                        Ok(r) => {
                            let u = SecUserData::from_bytes(&r);
                            if u.sec_usr_pwd() == &user_pwd[..8] {
                                ca.set_cdemo_from_tranid(TRAN.as_bytes());
                                ca.set_cdemo_from_program(PGM.as_bytes());
                                ca.set_cdemo_user_id(&user_id);
                                ca.set_cdemo_user_type(u.sec_usr_type());
                                ca.set_cdemo_pgm_context(b"0");
                                let to = if u.sec_usr_type() == b"A" { "COADM01C" } else { "COMEN01C" };
                                return Err(t.xctl(to, Some(&ca.buf)));
                            }
                            message = "Wrong Password. Try again ...".into();
                            map.cursor("PASSWD");
                            send_signon(t, &mut map, &message);
                        }
                        Err(vsam::Resp::NotFnd) => {
                            message = "User not found. Try again ...".into();
                            map.cursor("USERID");
                            send_signon(t, &mut map, &message);
                        }
                        Err(_) => {
                            message = "Unable to verify the User ...".into();
                            map.cursor("USERID");
                            send_signon(t, &mut map, &message);
                        }
                    }
                }
            }
            Aid::Pf(3) => {
                // SEND-PLAIN-TEXT then RETURN (no transid)
                t.send_text(CCDA_MSG_THANK_YOU.as_bytes());
                return Err(t.ret(None, None));
            }
            _ => {
                message = CCDA_MSG_INVALID_KEY.into();
                send_signon(t, &mut map, &message);
            }
        }
    }
    Err(t.ret(Some(TRAN), Some(&ca.buf)))
}

fn send_signon(t: &mut Task, map: &mut cics::Map, message: &str) {
    header(map, "CC00", "COSGN00C");
    map.set_str("APPLID", &t.applid.clone());
    map.set_str("SYSID", &t.sysid.clone());
    map.set_str("ERRMSG", message);
    t.send_map(map, true);
}

// ===================================================================== menus
struct MenuSpec {
    pgm: &'static str,
    tran: &'static str,
    map: &'static str,
    count: usize,
    admin: bool,
}

fn build_options(map: &mut cics::Map, spec: &MenuSpec) {
    for i in 1..=spec.count {
        let txt = if spec.admin {
            let (n, name, _) = ADMIN_OPTIONS[i - 1];
            format!("{n:02}. {name:<35}")
        } else {
            let (n, name, _, _) = MENU_OPTIONS[i - 1];
            format!("{n:02}. {name:<35}")
        };
        map.set_str(&format!("OPTN{i:03}"), &txt);
    }
}

fn send_menu(t: &mut Task, map: &mut cics::Map, spec: &MenuSpec, message: &str) {
    header(map, spec.tran, spec.pgm);
    build_options(map, spec);
    map.set_str("ERRMSG", message);
    t.send_map(map, true);
}

fn return_to_signon(t: &Task, ca: &mut CarddemoCommarea) -> cics::Transfer {
    if blank(ca.cdemo_to_program()) {
        ca.set_cdemo_to_program(b"COSGN00C");
    }
    t.xctl(&text::s(ca.cdemo_to_program()), None)
}

/// COMEN01C — main menu
pub fn comen01c(t: &mut Task) -> Cr {
    menu(t, &MenuSpec { pgm: "COMEN01C", tran: "CM00", map: "COMEN1A", count: MENU_OPTIONS.len(), admin: false })
}

/// COADM01C — admin menu
pub fn coadm01c(t: &mut Task) -> Cr {
    menu(t, &MenuSpec { pgm: "COADM01C", tran: "CA00", map: "COADM1A", count: ADMIN_OPTIONS.len(), admin: true })
}

fn menu(t: &mut Task, spec: &MenuSpec) -> Cr {
    let mut message = String::new();
    let mut map = t.map(spec.map);
    let mut ca = CarddemoCommarea::new();
    if t.eibcalen == 0 {
        ca.set_cdemo_from_program(b"COSGN00C");
        return Err(return_to_signon(t, &mut ca));
    }
    ca = cdemo(&load_commarea(&t.commarea, CarddemoCommarea::LEN));
    if !reenter(&ca) {
        ca.set_cdemo_pgm_context(b"1");
        map.clear();
        send_menu(t, &mut map, spec, &message);
    } else {
        map = t.receive_map(spec.map);
        match t.eibaid {
            Aid::Enter => {
                // PROCESS-ENTER-KEY: right-justify the option, zero-fill
                let opt_in = map.get("OPTION").to_vec();
                let mut idx = opt_in.len();
                while idx > 1 && opt_in[idx - 1] == b' ' {
                    idx -= 1;
                }
                let mut x = [b' '; 2];
                text::mv_just_right(&mut x, &opt_in[..idx]);
                for c in x.iter_mut() {
                    if *c == b' ' {
                        *c = b'0';
                    }
                }
                map.set("OPTION", &x);
                let numeric = x.iter().all(|c| c.is_ascii_digit());
                let opt: usize = if numeric { text::s(&x).parse().unwrap_or(0) } else { 0 };
                let mut err = false;
                if !numeric || opt > spec.count || opt == 0 {
                    err = true;
                    message = "Please enter a valid option number...".into();
                    send_menu(t, &mut map, spec, &message);
                }
                if !spec.admin && !err && ca.cdemo_user_type() == b"U" && MENU_OPTIONS[opt - 1].3 == "A" {
                    err = true;
                    message = "No access - Admin Only option... ".into();
                    send_menu(t, &mut map, spec, &message);
                }
                if !err {
                    let (name, pgm) = if spec.admin {
                        (ADMIN_OPTIONS[opt - 1].1, ADMIN_OPTIONS[opt - 1].2)
                    } else {
                        (MENU_OPTIONS[opt - 1].1, MENU_OPTIONS[opt - 1].2)
                    };
                    let first_word = name.split(' ').next().unwrap_or("");
                    let go = |t: &Task, ca: &mut CarddemoCommarea| {
                        ca.set_cdemo_from_tranid(spec.tran.as_bytes());
                        ca.set_cdemo_from_program(spec.pgm.as_bytes());
                        ca.set_cdemo_pgm_context(b"0");
                        t.xctl(pgm, Some(&ca.buf))
                    };
                    if spec.admin {
                        if t.program_exists(pgm) {
                            return Err(go(t, &mut ca));
                        }
                        // PGMIDERR-ERR-PARA: send, then RETURN
                        map.color("ERRMSG", Color::Green);
                        message = "This option is not installed ...".into();
                        send_menu(t, &mut map, spec, &message);
                        return Err(t.ret(Some(spec.tran), Some(&ca.buf)));
                    }
                    if pgm == "COPAUS0C" {
                        if t.program_exists(pgm) {
                            return Err(go(t, &mut ca));
                        }
                        map.color("ERRMSG", Color::Red);
                        message = format!("This option {first_word} is not installed...");
                    } else if pgm.starts_with("DUMMY") {
                        map.color("ERRMSG", Color::Green);
                        message = format!("This option {first_word}is coming soon ...");
                    } else {
                        return Err(go(t, &mut ca));
                    }
                    send_menu(t, &mut map, spec, &message);
                }
            }
            Aid::Pf(3) => {
                ca.set_cdemo_to_program(b"COSGN00C");
                return Err(return_to_signon(t, &mut ca));
            }
            _ => {
                message = CCDA_MSG_INVALID_KEY.into();
                send_menu(t, &mut map, spec, &message);
            }
        }
    }
    Err(t.ret(Some(spec.tran), Some(&ca.buf)))
}
