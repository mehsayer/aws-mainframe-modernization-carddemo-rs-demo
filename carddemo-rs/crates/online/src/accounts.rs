//! Accounts: COACTVWC (CAVW view) and COACTUPC (CAUP update).
//! Requirements FR-ACV, FR-ACU (incl. CSUTLDPY date edits and CSLKPCDY lookups).

use crate::cards::{file_error, join, pf3_back, split, store_pfkey, Flg};
use crate::common::*;
use crate::csutldtc::csutldtc;
use crate::lookups::*;
use cics::{Aid, AttrByte, Color, Cr, Map, Task};
use cobol::copybooks::{AccountRecord, CardXrefRecord, CarddemoCommarea, CustomerRecord};
use cobol::edit::edit;
use cobol::numeric::{fit, is_numeric, Pic};
use cobol::{text, Dec};
use vsam::Resp;

const MONEY: Pic = Pic::display(12, 2, true);

/// The fetch chain shared by view and update: XREF by account -> account -> customer.
struct Fetched {
    xref: Option<CardXrefRecord>,
    acct: Option<AccountRecord>,
    cust: Option<CustomerRecord>,
    msg: String,
    acct_not_ok: bool,
}

fn fetch(t: &mut Task, c: &mut CarddemoCommarea, acct_id: &[u8], view_style: bool) -> Fetched {
    let mut f = Fetched { xref: None, acct: None, cust: None, msg: String::new(), acct_not_ok: false };
    let aid = text::s(acct_id);
    match t.read("CXACAIX", acct_id, false) {
        Ok(r) => {
            let x = CardXrefRecord::from_bytes(&r);
            c.set_cdemo_cust_id(x.xref_cust_id());
            c.set_cdemo_card_num(x.xref_card_num());
            f.xref = Some(x);
        }
        Err(e @ Resp::NotFnd) => {
            f.acct_not_ok = true;
            f.msg = format!("Account:{aid} not found in Cross ref file.  Resp:{:<10} Reas:{:<10}", format!("{:09}", e.code()), format!("{:09}", e.code2()));
            if view_style {
                f.msg = format!("Account:{aid} not found in Cross ref file. Resp:{:<10} Reas:{:<10}", format!("{:09}", e.code()), format!("{:09}", e.code2()));
            }
            return f;
        }
        Err(e) => {
            f.acct_not_ok = true;
            f.msg = file_error("READ", "CXACAIX", e, 12);
            return f;
        }
    }
    match t.read("ACCTDAT", acct_id, false) {
        Ok(r) => f.acct = Some(AccountRecord::from_bytes(&r)),
        Err(e @ Resp::NotFnd) => {
            f.acct_not_ok = true;
            f.msg = format!("Account:{aid} not found in Acct Master file.Resp:{:<10} Reas:{:<10}", format!("{:09}", e.code()), format!("{:09}", e.code2()));
            return f;
        }
        Err(e) => {
            f.acct_not_ok = true;
            f.msg = file_error("READ", "ACCTDAT", e, 12);
            return f;
        }
    }
    let cust_id = c.cdemo_cust_id().to_vec();
    match t.read("CUSTDAT", &cust_id, false) {
        Ok(r) => f.cust = Some(CustomerRecord::from_bytes(&r)),
        Err(e @ Resp::NotFnd) => {
            f.msg = format!("CustId:{} not found in customer master.Resp: {:<10} REAS:{:<10}", text::s(&cust_id), format!("{:09}", e.code()), format!("{:09}", e.code2()));
        }
        Err(e) => f.msg = file_error("READ", "CUSTDAT", e, 12),
    }
    f
}

// ===================================================================== COACTVWC
const V_PGM: &str = "COACTVWC";
const V_TRAN: &str = "CAVW";
const V_MAP: &str = "CACTVWA";

pub fn coactvwc(t: &mut Task) -> Cr {
    let aid = store_pfkey(t.eibaid);
    let (c0, prog) = split(&t.commarea, 12);
    let menu_entry = c0.cdemo_from_program() == b"COMEN01C" && !reenter(&c0);
    let mut c = if t.eibcalen == 0 || menu_entry { CarddemoCommarea::new() } else { c0 };
    let aid = if matches!(aid, Aid::Enter | Aid::Pf(3)) { aid } else { Aid::Enter };
    if aid == Aid::Pf(3) {
        return Err(pf3_back(t, &mut c, V_TRAN, V_PGM, "COACTVW", V_MAP, false));
    }
    let mut map = t.map(V_MAP);
    let mut msg = String::new();
    let mut flg = Flg::Valid;
    let mut fetched: Option<Fetched> = None;
    let mut cc_acct = vec![b' '; 11];
    if reenter(&c) {
        map = t.receive_map(V_MAP);
        let a = map.get("ACCTSID").to_vec();
        if a.first() == Some(&b'*') && text::is_spaces(&a[1..]) || text::is_spaces(&a) || text::is_low(&a) {
            cc_acct = vec![0u8; 11];
        } else {
            text::mv(&mut cc_acct, &a);
        }
        // 2210-EDIT-ACCOUNT
        if blank(&cc_acct) {
            flg = Flg::Blank;
            msg = "Account number not provided".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
        } else if !is_numeric(&cc_acct, false) || cc_acct.iter().all(|&b| b == b'0') {
            flg = Flg::NotOk;
            msg = "Account Filter must be a non-zero 11 digit number".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
        } else {
            c.set_cdemo_acct_id(&cc_acct);
        }
        if flg == Flg::Valid {
            let f = fetch(t, &mut c, &cc_acct, true);
            if f.acct_not_ok {
                flg = Flg::NotOk;
            }
            msg = f.msg.clone();
            fetched = Some(f);
        }
    }
    // 1000-SEND-MAP
    map.clear();
    header(&mut map, V_TRAN, V_PGM);
    let mut info = String::new();
    if t.eibcalen == 0 {
        info = "Enter or update id of account to display".into();
    } else {
        if flg == Flg::Blank {
            map.set_low("ACCTSID");
        } else {
            map.set("ACCTSID", &cc_acct);
        }
        if let Some(f) = &fetched {
            if let Some(a) = &f.acct {
                map.set("ACSTTUS", a.acct_active_status());
                map.set_num("ACURBAL", a.acct_curr_bal_n());
                map.set_num("ACRDLIM", a.acct_credit_limit_n());
                map.set_num("ACSHLIM", a.acct_cash_credit_limit_n());
                map.set_num("ACRCYCR", a.acct_curr_cyc_credit_n());
                map.set_num("ACRCYDB", a.acct_curr_cyc_debit_n());
                map.set("ADTOPEN", a.acct_open_date());
                map.set("AEXPDT", a.acct_expiraion_date());
                map.set("AREISDT", a.acct_reissue_date());
                map.set("AADDGRP", a.acct_group_id());
            }
            if let Some(cu) = &f.cust {
                map.set("ACSTNUM", cu.cust_id());
                let ssn = cu.cust_ssn();
                map.set_str("ACSTSSN", &format!("{}-{}-{}", text::s(&ssn[0..3]), text::s(&ssn[3..5]), text::s(&ssn[5..9])));
                map.set("ACSTFCO", cu.cust_fico_credit_score());
                map.set("ACSTDOB", cu.cust_dob_yyyy_mm_dd());
                map.set("ACSFNAM", cu.cust_first_name());
                map.set("ACSMNAM", cu.cust_middle_name());
                map.set("ACSLNAM", cu.cust_last_name());
                map.set("ACSADL1", cu.cust_addr_line_1());
                map.set("ACSADL2", cu.cust_addr_line_2());
                map.set("ACSCITY", cu.cust_addr_line_3());
                map.set("ACSSTTE", cu.cust_addr_state_cd());
                map.set("ACSZIPC", cu.cust_addr_zip());
                map.set("ACSCTRY", cu.cust_addr_country_cd());
                map.set("ACSPHN1", cu.cust_phone_num_1());
                map.set("ACSPHN2", cu.cust_phone_num_2());
                map.set("ACSGOVT", cu.cust_govt_issued_id());
                map.set("ACSEFTC", cu.cust_eft_account_id());
                map.set("ACSPFLG", cu.cust_pri_card_holder_ind());
            }
        }
    }
    if info.is_empty() {
        info = "Enter or update id of account to display".into();
    }
    map.set_str("ERRMSG", &msg);
    map.set_str("INFOMSG", &info);
    map.attr("ACCTSID", AttrByte::UnprotFset);
    map.cursor("ACCTSID");
    map.default_color("ACCTSID");
    if flg == Flg::NotOk {
        map.color("ACCTSID", Color::Red);
    }
    if flg == Flg::Blank && reenter(&c) {
        map.set_str("ACCTSID", "*");
        map.color("ACCTSID", Color::Red);
    }
    map.color("INFOMSG", Color::Neutral);
    c.set_cdemo_pgm_context(b"1");
    t.send_map(&map, true);
    Err(t.ret(Some(V_TRAN), Some(&join(&c, &prog))))
}

// ===================================================================== COACTUPC
const U_PGM: &str = "COACTUPC";
const U_TRAN: &str = "CAUP";
const U_MAP: &str = "CACTUPA";

/// Account+customer snapshot fields (internal commarea layout: action + values).
const FIELDS: &[(&str, usize)] = &[
    ("acct_id", 11), ("status", 1), ("curr_bal", 12), ("credit_limit", 12), ("cash_limit", 12),
    ("open_y", 4), ("open_m", 2), ("open_d", 2), ("exp_y", 4), ("exp_m", 2), ("exp_d", 2),
    ("ris_y", 4), ("ris_m", 2), ("ris_d", 2), ("cyc_credit", 12), ("cyc_debit", 12), ("group", 10),
    ("cust_id", 9), ("fname", 25), ("mname", 25), ("lname", 25), ("addr1", 50), ("addr2", 50),
    ("city", 50), ("state", 2), ("country", 3), ("zip", 10), ("phone1", 15), ("phone2", 15),
    ("ssn", 9), ("govt", 20), ("dob_y", 4), ("dob_m", 2), ("dob_d", 2), ("eft", 10), ("pri", 1), ("fico", 3),
];
fn prog_len() -> usize {
    1 + FIELDS.iter().map(|f| f.1).sum::<usize>()
}

#[derive(Clone, Default)]
struct Snap(std::collections::HashMap<&'static str, Vec<u8>>);

impl Snap {
    fn get(&self, k: &str) -> Vec<u8> {
        self.0.get(k).cloned().unwrap_or_default()
    }
    fn set(&mut self, k: &'static str, v: &[u8]) {
        let len = FIELDS.iter().find(|f| f.0 == k).unwrap().1;
        let mut b = vec![b' '; len];
        text::mv(&mut b, v);
        self.0.insert(k, b);
    }
    fn money(&self, k: &str) -> Dec {
        cobol::numeric::decode(&self.get(k), MONEY)
    }
    fn to_bytes(&self, action: u8) -> Vec<u8> {
        let mut v = vec![action];
        for (k, l) in FIELDS {
            let mut b = vec![b' '; *l];
            if let Some(x) = self.0.get(k) {
                b.copy_from_slice(x);
            }
            v.extend(b);
        }
        v
    }
    fn from_bytes(b: &[u8]) -> (u8, Snap) {
        let mut s = Snap::default();
        let mut o = 1;
        for (k, l) in FIELDS {
            s.0.insert(k, b[o..o + l].to_vec());
            o += l;
        }
        (b[0], s)
    }
    fn from_records(a: &AccountRecord, cu: &CustomerRecord) -> Snap {
        let mut s = Snap::default();
        s.set("acct_id", a.acct_id());
        s.set("status", a.acct_active_status());
        s.set("curr_bal", a.acct_curr_bal());
        s.set("credit_limit", a.acct_credit_limit());
        s.set("cash_limit", a.acct_cash_credit_limit());
        let (o, e, r) = (a.acct_open_date(), a.acct_expiraion_date(), a.acct_reissue_date());
        s.set("open_y", &o[0..4]);
        s.set("open_m", &o[5..7]);
        s.set("open_d", &o[8..10]);
        s.set("exp_y", &e[0..4]);
        s.set("exp_m", &e[5..7]);
        s.set("exp_d", &e[8..10]);
        s.set("ris_y", &r[0..4]);
        s.set("ris_m", &r[5..7]);
        s.set("ris_d", &r[8..10]);
        s.set("cyc_credit", a.acct_curr_cyc_credit());
        s.set("cyc_debit", a.acct_curr_cyc_debit());
        s.set("group", a.acct_group_id());
        s.set("cust_id", cu.cust_id());
        s.set("fname", cu.cust_first_name());
        s.set("mname", cu.cust_middle_name());
        s.set("lname", cu.cust_last_name());
        s.set("addr1", cu.cust_addr_line_1());
        s.set("addr2", cu.cust_addr_line_2());
        s.set("city", cu.cust_addr_line_3());
        s.set("state", cu.cust_addr_state_cd());
        s.set("country", cu.cust_addr_country_cd());
        s.set("zip", cu.cust_addr_zip());
        s.set("phone1", cu.cust_phone_num_1());
        s.set("phone2", cu.cust_phone_num_2());
        s.set("ssn", cu.cust_ssn());
        s.set("govt", cu.cust_govt_issued_id());
        let d = cu.cust_dob_yyyy_mm_dd();
        s.set("dob_y", &d[0..4]);
        s.set("dob_m", &d[5..7]);
        s.set("dob_d", &d[8..10]);
        s.set("eft", cu.cust_eft_account_id());
        s.set("pri", cu.cust_pri_card_holder_ind());
        s.set("fico", cu.cust_fico_credit_score());
        s
    }
}

/// Values received from the screen for the update (ACUP-NEW-*), with the raw
/// money strings and their parsed values.
#[derive(Default)]
struct New {
    v: std::collections::HashMap<&'static str, Vec<u8>>,
    money: std::collections::HashMap<&'static str, (Vec<u8>, Option<Dec>)>,
}

/// Screen field -> snapshot key (non-money fields)
const SCREEN: &[(&str, &str)] = &[
    ("ACSTTUS", "status"), ("OPNYEAR", "open_y"), ("OPNMON", "open_m"), ("OPNDAY", "open_d"),
    ("EXPYEAR", "exp_y"), ("EXPMON", "exp_m"), ("EXPDAY", "exp_d"), ("RISYEAR", "ris_y"), ("RISMON", "ris_m"),
    ("RISDAY", "ris_d"), ("AADDGRP", "group"), ("ACSTNUM", "cust_id"), ("ACTSSN1", "ssn1"), ("ACTSSN2", "ssn2"),
    ("ACTSSN3", "ssn3"), ("DOBYEAR", "dob_y"), ("DOBMON", "dob_m"), ("DOBDAY", "dob_d"), ("ACSTFCO", "fico"),
    ("ACSFNAM", "fname"), ("ACSMNAM", "mname"), ("ACSLNAM", "lname"), ("ACSADL1", "addr1"), ("ACSADL2", "addr2"),
    ("ACSCITY", "city"), ("ACSSTTE", "state"), ("ACSCTRY", "country"), ("ACSZIPC", "zip"), ("ACSPH1A", "ph1a"),
    ("ACSPH1B", "ph1b"), ("ACSPH1C", "ph1c"), ("ACSPH2A", "ph2a"), ("ACSPH2B", "ph2b"), ("ACSPH2C", "ph2c"),
    ("ACSGOVT", "govt"), ("ACSEFTC", "eft"), ("ACSPFLG", "pri"),
];
const MONEY_FIELDS: &[(&str, &str)] = &[
    ("ACRDLIM", "credit_limit"), ("ACSHLIM", "cash_limit"), ("ACURBAL", "curr_bal"), ("ACRCYCR", "cyc_credit"), ("ACRCYDB", "cyc_debit"),
];

struct Acu {
    action: u8,
    old: Snap,
    new: New,
    input_error: bool,
    msg: String,
    acct_flg: Flg,
    /// screen field -> error state ('0' not ok, 'B' blank)
    bad: std::collections::HashMap<&'static str, u8>,
    no_change: bool,
    found: bool,
    cc_acct: Vec<u8>,
}

impl Acu {
    fn err(&mut self, field: &'static str, kind: u8, msg: String) {
        self.input_error = true;
        self.bad.entry(field).or_insert(kind);
        if self.msg.is_empty() {
            self.msg = msg;
        }
    }
    fn nv(&self, k: &str) -> Vec<u8> {
        self.new.v.get(k).cloned().unwrap_or_default()
    }
}

pub fn coactupc(t: &mut Task) -> Cr {
    let aid = store_pfkey(t.eibaid);
    let plen = prog_len();
    let (c0, p0) = split(&t.commarea, plen);
    let (mut c, (action, old)) = if t.eibcalen == 0 || (c0.cdemo_from_program() == b"COMEN01C" && !reenter(&c0)) {
        let mut c = CarddemoCommarea::new();
        c.set_cdemo_pgm_context(b"0");
        (c, (0u8, Snap::default()))
    } else {
        (c0, Snap::from_bytes(&p0))
    };
    let mut s = Acu {
        action,
        old,
        new: New::default(),
        input_error: false,
        msg: String::new(),
        acct_flg: Flg::Blank,
        bad: Default::default(),
        no_change: false,
        found: false,
        cc_acct: vec![b' '; 11],
    };
    let not_fetched = |a: u8| a == 0 || a == b' ';
    let valid = aid == Aid::Enter || aid == Aid::Pf(3) || (aid == Aid::Pf(5) && s.action == b'N') || (aid == Aid::Pf(12) && !not_fetched(s.action));
    let aid = if valid { aid } else { Aid::Enter };
    if aid == Aid::Pf(3) {
        return Err(pf3_back(t, &mut c, U_TRAN, U_PGM, "COACTUP", U_MAP, false));
    }
    let mut map = t.map(U_MAP);
    if (not_fetched(s.action) && !reenter(&c)) || (c.cdemo_from_program() == b"COMEN01C" && !reenter(&c)) {
        s.old = Snap::default();
        s.action = 0;
        acu_send(t, &mut s, &c, &mut map);
        c.set_cdemo_pgm_context(b"1");
    } else if matches!(s.action, b'C' | b'L' | b'F') {
        s.old = Snap::default();
        c.set_cdemo_acct_id_n(Dec::ZERO);
        c.set_cdemo_pgm_context(b"0");
        s.action = 0;
        acu_send(t, &mut s, &c, &mut map);
        c.set_cdemo_pgm_context(b"1");
        s.action = 0;
    } else {
        acu_receive(t, &mut s, &mut c, &mut map);
        acu_decide(t, &mut s, &mut c, aid);
        acu_send(t, &mut s, &c, &mut map);
    }
    let mut prog = s.old.to_bytes(s.action);
    prog.resize(plen, b' ');
    Err(t.ret(Some(U_TRAN), Some(&join(&c, &prog))))
}

fn acu_receive(t: &mut Task, s: &mut Acu, c: &mut CarddemoCommarea, map: &mut Map) {
    *map = t.receive_map(U_MAP);
    let take = |v: &[u8]| -> Vec<u8> {
        if (v.first() == Some(&b'*') && text::is_spaces(&v[1..])) || text::is_spaces(v) || text::is_low(v) { vec![0u8; v.len()] } else { v.to_vec() }
    };
    s.cc_acct = take(map.get("ACCTSID"));
    let not_fetched = s.action == 0 || s.action == b' ';
    if !not_fetched {
        for (f, k) in SCREEN {
            s.new.v.insert(k, take(map.get(f)));
        }
        for (f, k) in MONEY_FIELDS {
            let raw = take(map.get(f));
            let n = if text::test_numval_c(&raw) == 0 && !blank(&raw) { Some(fit(text::numval_c(&raw), MONEY)) } else { None };
            s.new.money.insert(k, (raw, n));
        }
    }
    // 1200-EDIT-MAP-INPUTS
    if not_fetched {
        // 1210-EDIT-ACCOUNT
        if blank(&s.cc_acct) {
            s.input_error = true;
            s.acct_flg = Flg::Blank;
            s.msg = "Account number not provided".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
        } else if !is_numeric(&s.cc_acct, false) || s.cc_acct.iter().all(|&b| b == b'0') {
            s.input_error = true;
            s.acct_flg = Flg::NotOk;
            s.msg = "Account Number if supplied must be a 11 digit Non-Zero Number".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
        } else {
            c.set_cdemo_acct_id(&s.cc_acct);
            s.acct_flg = Flg::Valid;
        }
        return;
    }
    s.found = true;
    s.acct_flg = Flg::Valid;
    s.cc_acct = s.old.get("acct_id");
    // 1205-COMPARE-OLD-NEW
    s.no_change = compare_old_new(s);
    if s.no_change {
        s.msg = "No change detected with respect to values fetched.".into();
    }
    if s.no_change || matches!(s.action, b'N' | b'C') {
        return;
    }
    s.action = b'E';
    acu_edits(s);
    if !s.input_error {
        s.action = b'N';
    }
}

fn up(v: &[u8]) -> Vec<u8> {
    let mut x = v.to_ascii_uppercase();
    for b in x.iter_mut() {
        if *b == 0 {
            *b = b' ';
        }
    }
    x
}

fn compare_old_new(s: &Acu) -> bool {
    let o = &s.old;
    for (_, k) in MONEY_FIELDS {
        let (raw, n) = s.new.money.get(k).cloned().unwrap_or_default();
        if n != Some(o.money(k)) {
            if std::env::var("ACU_DEBUG").is_ok() { eprintln!("DIFF {k}: raw={:?} n={:?} old={}", text::s(&raw), n, o.money(k)); }
            return false;
        }
    }
    let ssn = [s.nv("ssn1"), s.nv("ssn2"), s.nv("ssn3")].concat();
    if ssn != o.get("ssn") {
        if std::env::var("ACU_DEBUG").is_ok() { eprintln!("DIFF ssn {:?} {:?}", text::s(&ssn), text::s(&o.get("ssn"))); }
        return false;
    }
    let ph = |a: &str, b: &str, cc: &str| -> Vec<u8> { [b"(".to_vec(), s.nv(a), b")".to_vec(), s.nv(b), b"-".to_vec(), s.nv(cc)].concat() };
    let mut p1 = vec![b' '; 15];
    text::mv(&mut p1, &ph("ph1a", "ph1b", "ph1c"));
    let mut p2 = vec![b' '; 15];
    text::mv(&mut p2, &ph("ph2a", "ph2b", "ph2c"));
    if up(&p1) != up(&o.get("phone1")) || up(&p2) != up(&o.get("phone2")) {
        if std::env::var("ACU_DEBUG").is_ok() { eprintln!("DIFF phone {:?} {:?} / {:?} {:?}", text::s(&p1), text::s(&o.get("phone1")), text::s(&p2), text::s(&o.get("phone2"))); }
        return false;
    }
    for (_, k) in SCREEN {
        if k.starts_with("ssn") || k.starts_with("ph") {
            continue;
        }
        let mut n = vec![b' '; o.get(k).len()];
        text::mv(&mut n, &s.nv(k));
        if up(&n) != up(&o.get(k)) {
            if std::env::var("ACU_DEBUG").is_ok() { eprintln!("DIFF {k}: new={:?} old={:?}", text::s(&n), text::s(&o.get(k))); }
            return false;
        }
    }
    true
}

/// CSUTLDPY EDIT-DATE-CCYYMMDD (+ EDIT-DATE-OF-BIRTH when `dob`).
#[allow(clippy::too_many_arguments)] // mirrors the copybook's parameters (field names + keys)
fn edit_date(s: &mut Acu, name: &str, fy: &'static str, fm: &'static str, fd: &'static str, ky: &str, km: &str, kd: &str, dob: bool) {
    let (y, m, d) = (s.nv(ky), s.nv(km), s.nv(kd));
    let mut parts_ok = [true, true, true];
    // year
    if blank(&y) {
        s.err(fy, b'B', format!("{name} : Year must be supplied."));
        parts_ok[0] = false;
    } else if !is_numeric(&y, false) {
        s.err(fy, b'0', format!("{name} must be 4 digit number."));
        parts_ok[0] = false;
    } else if !(&y[0..2] == b"19" || &y[0..2] == b"20") {
        s.err(fy, b'0', format!("{name} : Century is not valid."));
        parts_ok[0] = false;
    }
    // month
    let mn: Option<u32> = if is_numeric(&m, false) { text::s(&m).parse().ok() } else { None };
    if blank(&m) {
        s.err(fm, b'B', format!("{name} : Month must be supplied."));
        parts_ok[1] = false;
    } else if !mn.is_some_and(|v| (1..=12).contains(&v)) || text::test_numval(&m) != 0 {
        s.err(fm, b'0', format!("{name}: Month must be a number between 1 and 12."));
        parts_ok[1] = false;
    }
    // day
    let dn: Option<u32> = if text::test_numval(&d) == 0 { Some(cobol::numeric::to_i64(text::numval(&d)) as u32) } else { None };
    if blank(&d) {
        s.err(fd, b'B', format!("{name} : Day must be supplied."));
        parts_ok[2] = false;
    } else if dn.is_none() || !dn.is_some_and(|v| (1..=31).contains(&v)) {
        s.err(fd, b'0', format!("{name}:day must be a number between 1 and 31."));
        parts_ok[2] = false;
    }
    // EDIT-DAY-MONTH-YEAR
    let mv = mn.unwrap_or(0);
    let dv = dn.unwrap_or(0);
    if ![1, 3, 5, 7, 8, 10, 12].contains(&mv) && dv == 31 {
        s.err(fd, b'0', format!("{name}:Cannot have 31 days in this month."));
        s.bad.entry(fm).or_insert(b'0');
        return;
    }
    if mv == 2 && dv == 30 {
        s.err(fd, b'0', format!("{name}:Cannot have 30 days in this month."));
        s.bad.entry(fm).or_insert(b'0');
        return;
    }
    if mv == 2 && dv == 29 {
        let yv: u32 = text::s(&y).parse().unwrap_or(0);
        let div = if yv.is_multiple_of(100) { 400 } else { 4 };
        if !yv.is_multiple_of(div) {
            s.err(fd, b'0', format!("{name}:Not a leap year.Cannot have 29 days in this month."));
            s.bad.entry(fm).or_insert(b'0');
            s.bad.entry(fy).or_insert(b'0');
            return;
        }
    }
    if !parts_ok.iter().all(|&x| x) {
        return;
    }
    // EDIT-DATE-LE (CSUTLDTC with YYYYMMDD); on failure the flags end up valid again
    let ymd = [y.clone(), m.clone(), d.clone()].concat();
    let r = csutldtc(&ymd, b"YYYYMMDD");
    if r.severity != 0 {
        s.input_error = true;
        if s.msg.is_empty() {
            s.msg = format!("{name} validation error Sev code: {} Message code: {}", r.severity_str(), r.msg_no_str());
        }
        return;
    }
    if dob {
        let today = now().date().format("%Y%m%d").to_string();
        if today.as_bytes() <= &ymd[..] {
            s.err(fy, b'0', format!("{name}:cannot be in the future "));
            s.bad.entry(fm).or_insert(b'0');
            s.bad.entry(fd).or_insert(b'0');
        }
    }
}

fn acu_edits(s: &mut Acu) {
    // Account Status (1220-EDIT-YESNO)
    yes_no(s, "ACSTTUS", "status", "Account Status");
    edit_date(s, "Open Date", "OPNYEAR", "OPNMON", "OPNDAY", "open_y", "open_m", "open_d", false);
    money(s, "ACRDLIM", "credit_limit", "Credit Limit");
    edit_date(s, "Expiry Date", "EXPYEAR", "EXPMON", "EXPDAY", "exp_y", "exp_m", "exp_d", false);
    money(s, "ACSHLIM", "cash_limit", "Cash Credit Limit");
    edit_date(s, "Reissue Date", "RISYEAR", "RISMON", "RISDAY", "ris_y", "ris_m", "ris_d", false);
    money(s, "ACURBAL", "curr_bal", "Current Balance");
    money(s, "ACRCYCR", "cyc_credit", "Current Cycle Credit Limit");
    money(s, "ACRCYDB", "cyc_debit", "Current Cycle Debit Limit");
    // 1265-EDIT-US-SSN
    if num_reqd(s, "ACTSSN1", "ssn1", "SSN: First 3 chars", 3) {
        let p1: u32 = text::s(&s.nv("ssn1")).parse().unwrap_or(0);
        if p1 == 0 || p1 == 666 || (900..=999).contains(&p1) {
            s.err("ACTSSN1", b'0', "SSN: First 3 chars: should not be 000, 666, or between 900 and 999".into());
        }
    }
    num_reqd(s, "ACTSSN2", "ssn2", "SSN 4th & 5th chars", 2);
    num_reqd(s, "ACTSSN3", "ssn3", "SSN Last 4 chars", 4);
    edit_date(s, "Date of Birth", "DOBYEAR", "DOBMON", "DOBDAY", "dob_y", "dob_m", "dob_d", true);
    if num_reqd(s, "ACSTFCO", "fico", "FICO Score", 3) {
        let f: u32 = text::s(&s.nv("fico")).parse().unwrap_or(0);
        if !(300..=850).contains(&f) {
            s.err("ACSTFCO", b'0', "FICO Score: should be between 300 and 850".into());
        }
    }
    alpha(s, "ACSFNAM", "fname", "First Name", 25, true);
    alpha(s, "ACSMNAM", "mname", "Middle Name", 25, false);
    alpha(s, "ACSLNAM", "lname", "Last Name", 25, true);
    // Address line 1 mandatory
    if blank(&s.nv("addr1")) || text::trim(&s.nv("addr1")).iter().all(|&b| b == 0) {
        s.err("ACSADL1", b'B', "Address Line 1 must be supplied.".into());
    }
    let state_ok = alpha(s, "ACSSTTE", "state", "State", 2, true);
    if state_ok && !US_STATE_CODES.contains(&text::s(&s.nv("state")).as_str()) {
        s.err("ACSSTTE", b'0', "State: is not a valid state code".into());
    }
    let zip_ok = num_reqd(s, "ACSZIPC", "zip", "Zip", 5);
    alpha(s, "ACSCITY", "city", "City", 50, true);
    alpha(s, "ACSCTRY", "country", "Country", 3, true);
    phone(s, "Phone Number 1", ["ACSPH1A", "ACSPH1B", "ACSPH1C"], ["ph1a", "ph1b", "ph1c"]);
    phone(s, "Phone Number 2", ["ACSPH2A", "ACSPH2B", "ACSPH2C"], ["ph2a", "ph2b", "ph2c"]);
    num_reqd(s, "ACSEFTC", "eft", "EFT Account Id", 10);
    yes_no(s, "ACSPFLG", "pri", "Primary Card Holder");
    if state_ok && zip_ok && !s.bad.contains_key("ACSSTTE") {
        let combo = format!("{}{}", text::s(&s.nv("state")), text::s(&s.nv("zip")[..2]));
        if !US_STATE_ZIP2.contains(&combo.as_str()) {
            s.err("ACSSTTE", b'0', "Invalid zip code for state".into());
            s.bad.entry("ACSZIPC").or_insert(b'0');
        }
    }
}

fn yes_no(s: &mut Acu, f: &'static str, k: &str, name: &str) {
    let v = s.nv(k);
    if blank(&v) || v == b"0" {
        s.err(f, b'B', format!("{name} must be supplied."));
    } else if !(v == b"Y" || v == b"N") {
        s.err(f, b'0', format!("{name} must be Y or N."));
    }
}

fn money(s: &mut Acu, f: &'static str, k: &'static str, name: &str) {
    let (raw, n) = s.new.money.get(k).cloned().unwrap_or_default();
    if blank(&raw) {
        s.err(f, b'B', format!("{name} must be supplied."));
    } else if n.is_none() {
        s.err(f, b'0', format!("{name} is not valid"));
    }
}

/// 1245-EDIT-NUM-REQD; returns true when valid
fn num_reqd(s: &mut Acu, f: &'static str, k: &str, name: &str, len: usize) -> bool {
    let mut v = s.nv(k);
    v.resize(len, b' ');
    let v = &v[..len];
    if blank(v) || text::trim(v).is_empty() {
        s.err(f, b'B', format!("{name} must be supplied."));
        false
    } else if !is_numeric(v, false) {
        s.err(f, b'0', format!("{name} must be all numeric."));
        false
    } else if text::numval(v) == Dec::ZERO {
        s.err(f, b'0', format!("{name} must not be zero."));
        false
    } else {
        true
    }
}

/// 1225-EDIT-ALPHA-REQD / 1235-EDIT-ALPHA-OPT; returns true when valid
fn alpha(s: &mut Acu, f: &'static str, k: &str, name: &str, len: usize, required: bool) -> bool {
    let mut v = s.nv(k);
    v.resize(len, b' ');
    let v = &v[..len];
    if blank(v) || text::trim(v).iter().all(|&b| b == 0) {
        if required {
            s.err(f, b'B', format!("{name} must be supplied."));
            return false;
        }
        return true;
    }
    if v.iter().all(|c| c.is_ascii_alphabetic() || *c == b' ' || *c == 0) {
        true
    } else {
        s.err(f, b'0', format!("{name} can have alphabets only."));
        false
    }
}

fn phone(s: &mut Acu, name: &str, f: [&'static str; 3], k: [&str; 3]) {
    let (a, b, c) = (s.nv(k[0]), s.nv(k[1]), s.nv(k[2]));
    // KD-08: the third test looks at the area code again instead of the line number
    if blank(&a) && blank(&b) && (text::is_spaces(&a) || text::is_low(&c)) {
        return;
    }
    let part = |s: &mut Acu, v: &[u8], fld: &'static str, label: &str, digits: &str, check_area: bool| {
        if blank(v) {
            s.err(fld, b'B', format!("{name}: {label} must be supplied."));
        } else if !is_numeric(v, false) {
            s.err(fld, b'0', format!("{name}: {label} must be A {digits} digit number."));
        } else if text::s(v).parse::<u32>().unwrap_or(0) == 0 {
            s.err(fld, b'0', format!("{name}: {label} cannot be zero"));
        } else if check_area && !GENERAL_PURPOSE_AREA_CODES.contains(&text::s(text::trim(v)).as_str()) {
            s.err(fld, b'0', format!("{name}: Not valid North America general purpose area code"));
        }
    };
    part(s, &a, f[0], "Area code", "3", true);
    part(s, &b, f[1], "Prefix code", "3", false);
    part(s, &c, f[2], "Line number code", "4", false);
}

fn acu_decide(t: &mut Task, s: &mut Acu, c: &mut CarddemoCommarea, aid: Aid) {
    let not_fetched = s.action == 0 || s.action == b' ';
    if not_fetched || aid == Aid::Pf(12) {
        if s.acct_flg == Flg::Valid {
            s.msg.clear();
            let key = if not_fetched { s.cc_acct.clone() } else { s.old.get("acct_id") };
            let f = fetch(t, c, &key, false);
            if f.acct_not_ok {
                s.input_error = true;
                s.acct_flg = Flg::NotOk;
            }
            if !f.msg.is_empty() {
                s.input_error = true;
            }
            s.msg = f.msg;
            if let (Some(a), Some(cu)) = (&f.acct, &f.cust) {
                s.old = Snap::from_records(a, cu);
                s.action = b'S';
                s.found = true;
            }
        }
        return;
    }
    match s.action {
        b'S' => {
            if !(s.input_error || s.no_change) {
                s.action = b'N';
            }
        }
        b'N' if aid == Aid::Pf(5) => s.action = acu_write(t, s, c),
        b'C' => {
            s.action = b'S';
            if blank(c.cdemo_from_tranid()) {
                c.set_cdemo_acct_id_n(Dec::ZERO);
                c.set_cdemo_card_num_n(Dec::ZERO);
                c.set_cdemo_acct_status(&[0]);
            }
        }
        _ => {}
    }
}

fn acu_write(t: &mut Task, s: &mut Acu, c: &CarddemoCommarea) -> u8 {
    let key = s.old.get("acct_id");
    let Ok(ar) = t.read("ACCTDAT", &key, true) else {
        s.input_error = true;
        if s.msg.is_empty() {
            s.msg = "Could not lock account record for update".into();
        }
        return b'L';
    };
    let cust_id = c.cdemo_cust_id().to_vec();
    let Ok(cr) = t.read("CUSTDAT", &cust_id, true) else {
        s.input_error = true;
        if s.msg.is_empty() {
            s.msg = "Could not lock customer record for update".into();
        }
        return b'L';
    };
    let (a, cu) = (AccountRecord::from_bytes(&ar), CustomerRecord::from_bytes(&cr));
    let now_snap = Snap::from_records(&a, &cu);
    let o = &s.old;
    let same_up = |k: &str| up(&now_snap.get(k)) == up(&o.get(k));
    let acct_same = ["status", "curr_bal", "credit_limit", "cash_limit", "cyc_credit", "cyc_debit", "open_y", "open_m", "open_d", "exp_y", "exp_m", "exp_d", "ris_y", "ris_m", "ris_d"]
        .iter()
        .all(|k| now_snap.get(k) == o.get(k))
        && same_up("group");
    let cust_same = ["fname", "mname", "lname", "addr1", "addr2", "city", "state", "country", "govt"].iter().all(|k| same_up(k))
        && ["zip", "phone1", "phone2", "ssn", "dob_y", "dob_m", "dob_d", "eft", "pri", "fico"].iter().all(|k| now_snap.get(k) == o.get(k));
    if !(acct_same && cust_same) {
        s.msg = "Record changed by some one else. Please review".into();
        s.old = now_snap;
        return b'S';
    }
    let nv = |k: &str| s.nv(k);
    let mut na = AccountRecord::new();
    na.set_acct_id(&key);
    na.set_acct_active_status(&nv("status"));
    let m = |k: &str| s.new.money.get(k).and_then(|x| x.1).unwrap_or(Dec::ZERO);
    na.set_acct_curr_bal_n(m("curr_bal"));
    na.set_acct_credit_limit_n(m("credit_limit"));
    na.set_acct_cash_credit_limit_n(m("cash_limit"));
    na.set_acct_curr_cyc_credit_n(m("cyc_credit"));
    na.set_acct_curr_cyc_debit_n(m("cyc_debit"));
    let date = |y: &str, mo: &str, d: &str| [nv(y), b"-".to_vec(), nv(mo), b"-".to_vec(), nv(d)].concat();
    na.set_acct_open_date(&date("open_y", "open_m", "open_d"));
    na.set_acct_expiraion_date(&date("exp_y", "exp_m", "exp_d"));
    na.set_acct_reissue_date(&date("ris_y", "ris_m", "ris_d"));
    na.set_acct_group_id(&nv("group"));
    let mut nc = CustomerRecord::new();
    nc.set_cust_id(&nv("cust_id"));
    nc.set_cust_first_name(&nv("fname"));
    nc.set_cust_middle_name(&nv("mname"));
    nc.set_cust_last_name(&nv("lname"));
    nc.set_cust_addr_line_1(&nv("addr1"));
    nc.set_cust_addr_line_2(&nv("addr2"));
    nc.set_cust_addr_line_3(&nv("city"));
    nc.set_cust_addr_state_cd(&nv("state"));
    nc.set_cust_addr_country_cd(&nv("country"));
    nc.set_cust_addr_zip(&nv("zip"));
    nc.set_cust_phone_num_1(&[b"(".to_vec(), nv("ph1a"), b")".to_vec(), nv("ph1b"), b"-".to_vec(), nv("ph1c")].concat());
    nc.set_cust_phone_num_2(&[b"(".to_vec(), nv("ph2a"), b")".to_vec(), nv("ph2b"), b"-".to_vec(), nv("ph2c")].concat());
    nc.set_cust_ssn(&[nv("ssn1"), nv("ssn2"), nv("ssn3")].concat());
    nc.set_cust_govt_issued_id(&nv("govt"));
    nc.set_cust_dob_yyyy_mm_dd(&date("dob_y", "dob_m", "dob_d"));
    nc.set_cust_eft_account_id(&nv("eft"));
    nc.set_cust_pri_card_holder_ind(&nv("pri"));
    nc.set_cust_fico_credit_score(&nv("fico"));
    if t.rewrite("ACCTDAT", &na.buf).is_err() {
        s.msg = "Update of record failed".into();
        return b'F';
    }
    if t.rewrite("CUSTDAT", &nc.buf).is_err() {
        s.msg = "Update of record failed".into();
        t.syncpoint_rollback();
        return b'F';
    }
    b'C'
}

fn acu_send(t: &mut Task, s: &mut Acu, c: &CarddemoCommarea, map: &mut Map) {
    map.clear();
    header(map, U_TRAN, U_PGM);
    let entering = c.cdemo_pgm_context() == b"0" && !reenter(c);
    if !entering {
        if blank(&s.cc_acct) && s.acct_flg != Flg::Valid {
            map.set_low("ACCTSID");
        } else {
            map.set("ACCTSID", &s.cc_acct);
        }
        match s.action {
            0 | b' ' => {}
            b'E' | b'N' | b'C' | b'L' | b'F' => show_new(s, map),
            _ => show_old(s, map),
        }
    }
    let info = match s.action {
        _ if entering => "Enter or update id of account to update",
        0 | b' ' => "Enter or update id of account to update",
        b'S' | b'E' => "Update account details presented above.",
        b'N' => "Changes validated.Press F5 to save",
        b'C' => "Changes committed to database",
        _ => "Changes unsuccessful. Please try again",
    };
    map.set_str("INFOMSG", info);
    map.set_str("ERRMSG", &s.msg);
    // 3300-SETUP-SCREEN-ATTRS
    let all: Vec<&str> = SCREEN.iter().map(|x| x.0).chain(MONEY_FIELDS.iter().map(|x| x.0)).filter(|f| *f != "ACSTNUM").collect();
    match s.action {
        0 | b' ' => {
            map.attr("ACCTSID", AttrByte::UnprotFset);
            for f in &all {
                map.attr(f, AttrByte::ProtFset);
            }
        }
        b'S' | b'E' => {
            map.attr("ACCTSID", AttrByte::ProtFset);
            for f in &all {
                map.attr(f, AttrByte::UnprotFset);
            }
        }
        _ => {
            map.attr("ACCTSID", AttrByte::ProtFset);
            for f in &all {
                map.attr(f, AttrByte::ProtFset);
            }
        }
    }
    map.attr("ACSTNUM", AttrByte::ProtFset);
    let first_bad = all.iter().find(|f| s.bad.contains_key(**f)).copied();
    let cursor = if s.acct_flg != Flg::Valid {
        "ACCTSID"
    } else if let Some(f) = first_bad {
        f
    } else if s.found {
        "ACSTTUS"
    } else {
        "ACCTSID"
    };
    map.cursor(cursor);
    if s.acct_flg == Flg::NotOk {
        map.color("ACCTSID", Color::Red);
    }
    if s.acct_flg == Flg::Blank && reenter(c) {
        map.set_str("ACCTSID", "*");
        map.color("ACCTSID", Color::Red);
    }
    if s.action == b'E' {
        for (f, kind) in s.bad.clone() {
            map.color(f, Color::Red);
            if kind == b'B' {
                map.set_str(f, "*");
            }
        }
    }
    map.attr("INFOMSG", if info.is_empty() { AttrByte::Dark } else { AttrByte::Bright });
    if s.action == b'N' {
        map.attr("FKEYS", AttrByte::Bright);
    }
    t.send_map(map, true);
}

fn show_old(s: &Acu, map: &mut Map) {
    let o = &s.old;
    for (f, k) in SCREEN {
        match *k {
            "ssn1" => map.set(f, &o.get("ssn")[0..3]),
            "ssn2" => map.set(f, &o.get("ssn")[3..5]),
            "ssn3" => map.set(f, &o.get("ssn")[5..9]),
            "ph1a" => map.set(f, &o.get("phone1")[1..4]),
            "ph1b" => map.set(f, &o.get("phone1")[5..8]),
            "ph1c" => map.set(f, &o.get("phone1")[9..13]),
            "ph2a" => map.set(f, &o.get("phone2")[1..4]),
            "ph2b" => map.set(f, &o.get("phone2")[5..8]),
            "ph2c" => map.set(f, &o.get("phone2")[9..13]),
            k => map.set(f, &o.get(k)),
        }
    }
    for (f, k) in MONEY_FIELDS {
        map.set_str(f, &edit(o.money(k), "+ZZZ,ZZZ,ZZZ.99"));
    }
}

fn show_new(s: &Acu, map: &mut Map) {
    for (f, k) in SCREEN {
        let v = s.nv(k);
        if v.iter().all(|&b| b == 0) {
            map.set_low(f);
        } else {
            map.set(f, &v);
        }
    }
    for (f, k) in MONEY_FIELDS {
        let (raw, n) = s.new.money.get(k).cloned().unwrap_or_default();
        match n {
            Some(v) if !s.bad.contains_key(*f) => map.set_str(f, &edit(v, "+ZZZ,ZZZ,ZZZ.99")),
            // only the cash credit limit redisplays an invalid raw value (3203)
            _ if *k == "cash_limit" => map.set(f, &raw),
            _ => map.set_low(f),
        }
    }
}
