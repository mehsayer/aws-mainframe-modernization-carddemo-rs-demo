//! Credit cards: COCRDLIC (CCLI list), COCRDSLC (CCDL detail), COCRDUPC (CCUP update).
//! Requirements FR-CDL, FR-CDV, FR-CDU and the shared rules BR-AC-01..07.

use crate::common::*;
use cics::{Aid, AttrByte, Color, Cr, Map, Task};
use cobol::copybooks::{CardRecord, CarddemoCommarea};
use cobol::numeric::is_numeric;
use cobol::{text, Dec};
use vsam::Resp;

pub const CA_TOTAL: usize = 2000;

/// YYYY-STORE-PFKEY: PF13-PF24 behave as PF1-PF12.
pub fn store_pfkey(aid: Aid) -> Aid {
    match aid {
        Aid::Pf(n) if n > 12 => Aid::Pf(n - 12),
        a => a,
    }
}

/// BR-AC-05 file error message.
pub fn file_error(op: &str, file: &str, e: Resp, prefix_len: usize) -> String {
    let mut s = format!("{:<w$}", "File Error:", w = prefix_len);
    s.push_str(&format!("{op:<8} on {file:<9} returned RESP {:<10},RESP2 {:<10}     ", format!("{:09}", e.code()), format!("{:09}", e.code2())));
    s
}

/// Split the 2000-byte commarea into CARDDEMO-COMMAREA and the program area.
pub fn split(ca: &[u8], prog_len: usize) -> (CarddemoCommarea, Vec<u8>) {
    let full = load_commarea(ca, CarddemoCommarea::LEN + prog_len);
    (cdemo(&full), full[CarddemoCommarea::LEN..].to_vec())
}
pub fn join(c: &CarddemoCommarea, prog: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; CA_TOTAL];
    v[..CarddemoCommarea::LEN].copy_from_slice(&c.buf);
    v[CarddemoCommarea::LEN..CarddemoCommarea::LEN + prog.len()].copy_from_slice(prog);
    v
}

fn set_n(c: &mut CarddemoCommarea, acct: Option<Dec>, card: Option<Dec>) {
    if let Some(a) = acct {
        c.set_cdemo_acct_id_n(a);
    }
    if let Some(n) = card {
        c.set_cdemo_card_num_n(n);
    }
}

/// Header of the newer-style screens (1100-SCREEN-INIT).
fn screen_init(map: &mut Map, tran: &str, pgm: &str) {
    map.clear();
    header(map, tran, pgm);
}

/// Filter flags (FLG-xxxFILTER-NOT-OK '0' / ISVALID '1' / BLANK ' ')
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flg {
    Blank,
    NotOk,
    Valid,
}

// ===================================================================== COCRDLIC
const L_PGM: &str = "COCRDLIC";
const L_TRAN: &str = "CCLI";
const L_MAP: &str = "CCRDLIA";
const L_MAPSET: &str = "COCRDLI";
/// WS-THIS-PROGCOMMAREA: LAST-CARD-NUM(16) LAST-ACCT 9(11) FIRST-CARD-NUM(16)
/// FIRST-ACCT 9(11) SCREEN-NUM 9 LAST-PAGE-DISPLAYED 9 NEXT-PAGE-IND X RETURN-FLAG X
/// + WS-SCREEN-DATA 7 x (acct 11, card 16, status 1)
const L_PROG: usize = 58 + 196;

struct Lic {
    prog: Vec<u8>,
    input_error: bool,
    acct_flg: Flg,
    card_flg: Flg,
    cc_acct: Vec<u8>,
    cc_card: Vec<u8>,
    select: [u8; 7],
    row_err: [bool; 7],
    protect_rows: bool,
    i_selected: usize,
    error_msg: String,
    info: bool,
    no_records: bool,
}

impl Lic {
    fn get(&self, off: usize, len: usize) -> &[u8] {
        &self.prog[off..off + len]
    }
    fn put(&mut self, off: usize, v: &[u8]) {
        self.prog[off..off + v.len()].copy_from_slice(v);
    }
    fn last_card(&self) -> Vec<u8> {
        self.get(0, 16).to_vec()
    }
    fn first_card(&self) -> Vec<u8> {
        self.get(27, 16).to_vec()
    }
    fn screen_num(&self) -> u8 {
        self.prog[54].wrapping_sub(b'0')
    }
    fn set_screen_num(&mut self, n: u8) {
        self.prog[54] = b'0' + (n % 10);
    }
    fn last_page_shown(&self) -> bool {
        self.prog[55] == b'0'
    }
    fn set_last_page(&mut self, shown: bool) {
        self.prog[55] = if shown { b'0' } else { b'9' };
    }
    fn next_exists(&self) -> bool {
        self.prog[56] == b'Y'
    }
    fn set_next(&mut self, y: bool) {
        self.prog[56] = if y { b'Y' } else { 0 };
    }
    fn row(&self, i: usize) -> &[u8] {
        self.get(58 + (i - 1) * 28, 28)
    }
    fn set_row(&mut self, i: usize, card: &CardRecord) {
        let o = 58 + (i - 1) * 28;
        self.prog[o..o + 11].copy_from_slice(card.card_acct_id());
        self.prog[o + 11..o + 27].copy_from_slice(card.card_num());
        self.prog[o + 27] = card.card_active_status()[0];
    }
    fn clear_rows(&mut self) {
        self.prog[58..58 + 196].fill(0);
    }
    fn msg_off(&self) -> bool {
        self.error_msg.is_empty()
    }
}

fn lic_new_prog() -> Vec<u8> {
    // INITIALIZE WS-THIS-PROGCOMMAREA
    let mut p = vec![b' '; L_PROG];
    p[16..27].fill(b'0');
    p[43..54].fill(b'0');
    p[54] = b'0';
    p[55] = b'0';
    p
}

pub fn cocrdlic(t: &mut Task) -> Cr {
    let aid = store_pfkey(t.eibaid);
    let (mut c, prog) = if t.eibcalen == 0 {
        let mut c = CarddemoCommarea::new();
        c.set_cdemo_from_tranid(L_TRAN.as_bytes());
        c.set_cdemo_from_program(L_PGM.as_bytes());
        c.set_cdemo_user_type(b"U");
        c.set_cdemo_pgm_context(b"0");
        c.set_cdemo_last_map(L_MAP.as_bytes());
        c.set_cdemo_last_mapset(L_MAPSET.as_bytes());
        let mut p = lic_new_prog();
        p[54] = b'1';
        p[55] = b'9';
        (c, p)
    } else {
        split(&t.commarea, L_PROG)
    };
    let mut s = Lic {
        prog,
        input_error: false,
        acct_flg: Flg::Blank,
        card_flg: Flg::Blank,
        cc_acct: vec![b' '; 11],
        cc_card: vec![b' '; 16],
        select: [b' '; 7],
        row_err: [false; 7],
        protect_rows: false,
        i_selected: 0,
        error_msg: String::new(),
        info: false,
        no_records: false,
    };
    let from_this = c.cdemo_from_program() == L_PGM.as_bytes();
    if !reenter(&c) && !from_this {
        s.prog = lic_new_prog();
        c.set_cdemo_pgm_context(b"0");
        c.set_cdemo_last_map(L_MAP.as_bytes());
        s.set_screen_num(1);
        s.set_last_page(false);
    }
    let mut map = t.map(L_MAP);
    if t.eibcalen > 0 && from_this {
        lic_receive(t, &mut s, &mut c, &mut map);
    }
    let valid = matches!(aid, Aid::Enter | Aid::Pf(3) | Aid::Pf(7) | Aid::Pf(8));
    let aid = if valid { aid } else { Aid::Enter };
    if aid == Aid::Pf(3) && from_this {
        c.set_cdemo_from_tranid(L_TRAN.as_bytes());
        c.set_cdemo_from_program(L_PGM.as_bytes());
        c.set_cdemo_user_type(b"U");
        c.set_cdemo_pgm_context(b"0");
        c.set_cdemo_last_mapset(L_MAPSET.as_bytes());
        c.set_cdemo_last_map(L_MAP.as_bytes());
        c.set_cdemo_to_program(b"COMEN01C");
        return Err(t.xctl("COMEN01C", Some(&c.buf)));
    }
    if aid != Aid::Pf(8) {
        s.set_last_page(false);
    }
    let first_page = s.screen_num() == 1;
    let selected = |s: &Lic, ch: u8| s.i_selected >= 1 && s.select[s.i_selected - 1] == ch;
    if s.input_error {
        if s.acct_flg != Flg::NotOk && s.card_flg != Flg::NotOk {
            lic_forward(t, &mut s, &[b' '; 16]);
        }
        lic_send(t, &mut s, &c, &mut map, aid);
    } else if aid == Aid::Pf(7) && first_page {
        let k = s.first_card();
        lic_forward(t, &mut s, &k);
        lic_send(t, &mut s, &c, &mut map, aid);
    } else if aid == Aid::Pf(3) || (reenter(&c) && !from_this) {
        c = CarddemoCommarea::new();
        s.prog = lic_new_prog();
        c.set_cdemo_from_tranid(L_TRAN.as_bytes());
        c.set_cdemo_from_program(L_PGM.as_bytes());
        c.set_cdemo_user_type(b"U");
        c.set_cdemo_pgm_context(b"0");
        c.set_cdemo_last_map(L_MAP.as_bytes());
        c.set_cdemo_last_mapset(L_MAPSET.as_bytes());
        s.set_screen_num(1);
        s.set_last_page(false);
        let k = s.first_card();
        lic_forward(t, &mut s, &k);
        lic_send(t, &mut s, &c, &mut map, aid);
    } else if aid == Aid::Pf(8) && s.next_exists() {
        let k = s.last_card();
        let n = s.screen_num() + 1;
        s.set_screen_num(n);
        lic_forward(t, &mut s, &k);
        lic_send(t, &mut s, &c, &mut map, aid);
    } else if aid == Aid::Pf(7) && !first_page {
        let k = s.first_card();
        let n = s.screen_num().saturating_sub(1);
        s.set_screen_num(n);
        lic_backward(t, &mut s, &k);
        lic_send(t, &mut s, &c, &mut map, aid);
    } else if aid == Aid::Enter && from_this && (selected(&s, b'S') || selected(&s, b'U')) {
        let to = if selected(&s, b'S') { "COCRDSLC" } else { "COCRDUPC" };
        c.set_cdemo_from_tranid(L_TRAN.as_bytes());
        c.set_cdemo_from_program(L_PGM.as_bytes());
        c.set_cdemo_user_type(b"U");
        c.set_cdemo_pgm_context(b"0");
        c.set_cdemo_last_mapset(L_MAPSET.as_bytes());
        c.set_cdemo_last_map(L_MAP.as_bytes());
        let row = s.row(s.i_selected).to_vec();
        c.set_cdemo_acct_id(&row[..11]);
        c.set_cdemo_card_num(&row[11..27]);
        return Err(t.xctl(to, Some(&c.buf)));
    } else {
        let k = s.first_card();
        lic_forward(t, &mut s, &k);
        lic_send(t, &mut s, &c, &mut map, aid);
    }
    // COMMON-RETURN
    c.set_cdemo_from_tranid(L_TRAN.as_bytes());
    c.set_cdemo_from_program(L_PGM.as_bytes());
    c.set_cdemo_last_mapset(L_MAPSET.as_bytes());
    c.set_cdemo_last_map(L_MAP.as_bytes());
    Err(t.ret(Some(L_TRAN), Some(&join(&c, &s.prog))))
}

fn lic_receive(t: &mut Task, s: &mut Lic, c: &mut CarddemoCommarea, map: &mut Map) {
    *map = t.receive_map(L_MAP);
    text::mv(&mut s.cc_acct, map.get("ACCTSID"));
    text::mv(&mut s.cc_card, map.get("CARDSID"));
    for i in 1..=7 {
        s.select[i - 1] = map.get(&format!("CRDSEL{i}"))[0];
    }
    // 2210-EDIT-ACCOUNT
    s.acct_flg = Flg::Blank;
    let zero = |b: &[u8]| is_numeric(b, false) && b.iter().all(|&x| x == b'0');
    if blank(&s.cc_acct) || zero(&s.cc_acct) {
        set_n(c, Some(Dec::ZERO), None);
    } else if !is_numeric(&s.cc_acct, false) {
        s.input_error = true;
        s.acct_flg = Flg::NotOk;
        s.protect_rows = true;
        s.error_msg = "ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER".into();
        set_n(c, Some(Dec::ZERO), None);
    } else {
        c.set_cdemo_acct_id(&s.cc_acct);
        s.acct_flg = Flg::Valid;
    }
    // 2220-EDIT-CARD
    s.card_flg = Flg::Blank;
    if blank(&s.cc_card) || zero(&s.cc_card) {
        set_n(c, None, Some(Dec::ZERO));
    } else if !is_numeric(&s.cc_card, false) {
        s.input_error = true;
        s.card_flg = Flg::NotOk;
        s.protect_rows = true;
        if s.msg_off() {
            s.error_msg = "CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER".into();
        }
        set_n(c, None, Some(Dec::ZERO));
    } else {
        c.set_cdemo_card_num(&s.cc_card);
        s.card_flg = Flg::Valid;
    }
    // 2250-EDIT-ARRAY
    if s.input_error {
        return;
    }
    let n = s.select.iter().filter(|&&x| x == b'S' || x == b'U').count();
    let more = n > 1;
    if more {
        s.input_error = true;
        s.error_msg = "PLEASE SELECT ONLY ONE RECORD TO VIEW OR UPDATE".into();
        for i in 0..7 {
            s.row_err[i] = s.select[i] == b'S' || s.select[i] == b'U';
        }
    }
    s.i_selected = 0;
    for i in 1..=7 {
        match s.select[i - 1] {
            b'S' | b'U' => {
                s.i_selected = i;
                if more {
                    s.row_err[i - 1] = true;
                }
            }
            b' ' | 0 => {}
            _ => {
                s.input_error = true;
                s.row_err[i - 1] = true;
                if s.msg_off() {
                    s.error_msg = "INVALID ACTION CODE".into();
                }
            }
        }
    }
}

/// 9500-FILTER-RECORDS
fn lic_keep(s: &Lic, card: &CardRecord) -> bool {
    if s.acct_flg == Flg::Valid && card.card_acct_id() != &s.cc_acct[..] {
        return false;
    }
    if s.card_flg == Flg::Valid && card.card_num() != &s.cc_card[..] {
        return false;
    }
    true
}

fn lic_forward(t: &mut Task, s: &mut Lic, key: &[u8]) {
    s.clear_rows();
    let mut br = t.startbr("CARDDAT", key, true).ok();
    let mut n = 0;
    s.set_next(true);
    loop {
        let r = match br.as_mut() {
            Some(b) => t.readnext(b),
            None => Err(Resp::InvReq),
        };
        match r {
            Ok(rec) => {
                let card = CardRecord::from_bytes(&rec);
                if lic_keep(s, &card) {
                    n += 1;
                    s.set_row(n, &card);
                    if n == 1 {
                        s.put(43, card.card_acct_id());
                        s.put(27, card.card_num());
                        if s.screen_num() == 0 {
                            s.set_screen_num(1);
                        }
                    }
                }
                if n == 7 {
                    s.put(16, card.card_acct_id());
                    s.put(0, card.card_num());
                    let r2 = match br.as_mut() {
                        Some(b) => t.readnext(b),
                        None => Err(Resp::InvReq),
                    };
                    match r2 {
                        Ok(rec2) => {
                            let c2 = CardRecord::from_bytes(&rec2);
                            s.set_next(true);
                            s.put(16, c2.card_acct_id());
                            s.put(0, c2.card_num());
                        }
                        Err(Resp::EndFile) => {
                            s.set_next(false);
                            if s.msg_off() {
                                s.error_msg = "NO MORE RECORDS TO SHOW".into();
                            }
                        }
                        Err(e) => s.error_msg = file_error("READ", "CARDDAT", e, 12),
                    }
                    break;
                }
            }
            Err(Resp::EndFile) => {
                s.set_next(false);
                if s.msg_off() {
                    s.error_msg = "NO MORE RECORDS TO SHOW".into();
                }
                if s.screen_num() == 1 && n == 0 {
                    s.error_msg = "NO RECORDS FOUND FOR THIS SEARCH CONDITION.".into();
                    s.no_records = true;
                }
                break;
            }
            Err(e) => {
                s.error_msg = file_error("READ", "CARDDAT", e, 12);
                break;
            }
        }
    }
}

fn lic_backward(t: &mut Task, s: &mut Lic, key: &[u8]) {
    s.clear_rows();
    let first = s.get(27, 27).to_vec();
    s.put(0, &first[..16]);
    s.put(16, &first[16..]);
    let mut br = t.startbr("CARDDAT", key, true).ok();
    let mut n: usize = 8;
    s.set_next(true);
    let mut read = |t: &mut Task| match br.as_mut() {
        Some(b) => t.readprev(b),
        None => Err(Resp::InvReq),
    };
    match read(t) {
        Ok(_) => n -= 1,
        Err(e) => {
            s.error_msg = file_error("READ", "CARDDAT", e, 12);
            return;
        }
    }
    loop {
        match read(t) {
            Ok(rec) => {
                let card = CardRecord::from_bytes(&rec);
                if lic_keep(s, &card) {
                    s.set_row(n, &card);
                    n -= 1;
                    if n == 0 {
                        s.put(43, card.card_acct_id());
                        s.put(27, card.card_num());
                        break;
                    }
                }
            }
            Err(e) => {
                s.error_msg = file_error("READ", "CARDDAT", e, 12);
                break;
            }
        }
    }
}

fn lic_send(t: &mut Task, s: &mut Lic, c: &CarddemoCommarea, map: &mut Map, aid: Aid) {
    // 1100-SCREEN-INIT
    screen_init(map, L_TRAN, L_PGM);
    map.set_str("PAGENO", &s.screen_num().to_string());
    s.info = false;
    // 1200-SCREEN-ARRAY-INIT
    for i in 1..=7 {
        let row = s.row(i).to_vec();
        if row.iter().all(|&b| b == 0) {
            continue;
        }
        map.set(&format!("CRDSEL{i}"), &[s.select[i - 1]]);
        map.set(&format!("ACCTNO{i}"), &row[..11]);
        map.set(&format!("CRDNUM{i}"), &row[11..27]);
        map.set(&format!("CRDSTS{i}"), &row[27..28]);
    }
    // 1250-SETUP-ARRAY-ATTRIBS
    for i in 1..=7 {
        let f = format!("CRDSEL{i}");
        if s.row(i).iter().all(|&b| b == 0) || s.protect_rows {
            map.attr(&f, if i == 1 { AttrByte::ProtFset } else { AttrByte::Prot });
        } else {
            if s.row_err[i - 1] {
                map.color(&f, Color::Red);
                if i == 1 {
                    if s.select[0] == b' ' || s.select[0] == 0 {
                        map.set(&f, b"*");
                    }
                } else {
                    map.cursor(&f);
                }
            }
            map.attr(&f, AttrByte::UnprotFset);
        }
    }
    // 1300-SETUP-SCREEN-ATTRS
    let menu_entry = !reenter(c) && c.cdemo_from_program() == b"COMEN01C";
    if !(t.eibcalen == 0 || menu_entry) {
        match s.acct_flg {
            Flg::Valid | Flg::NotOk => {
                map.set("ACCTSID", &s.cc_acct);
                map.attr("ACCTSID", AttrByte::UnprotFset);
            }
            _ if c.cdemo_acct_id_n() == Dec::ZERO => map.set_low("ACCTSID"),
            _ => {
                map.set("ACCTSID", c.cdemo_acct_id());
                map.attr("ACCTSID", AttrByte::UnprotFset);
            }
        }
        match s.card_flg {
            Flg::Valid | Flg::NotOk => {
                map.set("CARDSID", &s.cc_card);
                map.attr("CARDSID", AttrByte::UnprotFset);
            }
            _ if c.cdemo_card_num_n() == Dec::ZERO => map.set_low("CARDSID"),
            _ => {
                map.set("CARDSID", c.cdemo_card_num());
                map.attr("CARDSID", AttrByte::UnprotFset);
            }
        }
    }
    if s.acct_flg == Flg::NotOk {
        map.color("ACCTSID", Color::Red);
        map.cursor("ACCTSID");
    }
    if s.card_flg == Flg::NotOk {
        map.color("CARDSID", Color::Red);
        map.cursor("CARDSID");
    }
    if !s.input_error {
        map.cursor("ACCTSID");
    }
    // 1400-SETUP-MESSAGE
    let first_page = s.screen_num() == 1;
    if s.acct_flg == Flg::NotOk || s.card_flg == Flg::NotOk {
    } else if aid == Aid::Pf(7) && first_page {
        s.error_msg = "NO PREVIOUS PAGES TO DISPLAY".into();
    } else if aid == Aid::Pf(8) && !s.next_exists() && s.last_page_shown() {
        s.error_msg = "NO MORE PAGES TO DISPLAY".into();
    } else if aid == Aid::Pf(8) && !s.next_exists() {
        s.info = true;
        if !s.last_page_shown() {
            s.set_last_page(true);
        }
    } else {
        // WS-NO-INFO-MESSAGE is always true here (set by 1100-SCREEN-INIT)
        s.info = true;
    }
    map.set_str("ERRMSG", &s.error_msg);
    if s.info && !s.no_records {
        map.set_str("INFOMSG", "TYPE S FOR DETAIL, U TO UPDATE ANY RECORD");
        map.color("INFOMSG", Color::Neutral);
    }
    t.send_map(map, true);
}

// ===================================================================== COCRDSLC
const S_PGM: &str = "COCRDSLC";
const S_TRAN: &str = "CCDL";
const S_MAP: &str = "CCRDSLA";

pub fn cocrdslc(t: &mut Task) -> Cr {
    let aid = store_pfkey(t.eibaid);
    let (mut c, prog) = split(&t.commarea, 12);
    let menu_entry = c.cdemo_from_program() == b"COMEN01C" && !reenter(&c);
    let (mut c, prog) = if t.eibcalen == 0 || menu_entry { (CarddemoCommarea::new(), vec![b' '; 12]) } else { (std::mem::take(&mut c), prog) };
    let aid = if matches!(aid, Aid::Enter | Aid::Pf(3)) { aid } else { Aid::Enter };
    let mut st = Sl { input_error: false, acct: Flg::Valid, card: Flg::Valid, msg: String::new(), found: None, cc_acct: vec![b' '; 11], cc_card: vec![b' '; 16] };
    if aid == Aid::Pf(3) {
        return Err(pf3_back(t, &mut c, S_TRAN, S_PGM, "COCRDSL", S_MAP, false));
    }
    let mut map = t.map(S_MAP);
    if !reenter(&c) && c.cdemo_from_program() == b"COCRDLIC" {
        st.cc_acct = format!("{:011}", c.cdemo_acct_id_n()).into_bytes();
        st.cc_card = format!("{:016}", c.cdemo_card_num_n()).into_bytes();
        sl_read(t, &mut st);
    } else if reenter(&c) {
        map = t.receive_map(S_MAP);
        sl_edit(&mut st, &mut c, &map);
        if !st.input_error {
            sl_read(t, &mut st);
        }
    } else if !reenter(&c) {
        // plain first entry
    }
    sl_send(t, &mut st, &mut c, &mut map);
    let mut ca = join(&c, &prog);
    ca.truncate(CA_TOTAL);
    Err(t.ret(Some(S_TRAN), Some(&ca)))
}

struct Sl {
    input_error: bool,
    acct: Flg,
    card: Flg,
    msg: String,
    found: Option<CardRecord>,
    cc_acct: Vec<u8>,
    cc_card: Vec<u8>,
}

/// PF3 handling shared by the detail / update / account programs (BR-AC-04).
pub fn pf3_back(t: &Task, c: &mut CarddemoCommarea, tran: &str, pgm: &str, mapset: &str, map: &str, clear_keys_if_list: bool) -> cics::Transfer {
    let to_tran = if blank(c.cdemo_from_tranid()) { "CM00".to_string() } else { text::s(c.cdemo_from_tranid()) };
    let to_pgm = if blank(c.cdemo_from_program()) { "COMEN01C".to_string() } else { text::s(c.cdemo_from_program()) };
    c.set_cdemo_to_tranid(to_tran.as_bytes());
    c.set_cdemo_to_program(to_pgm.as_bytes());
    c.set_cdemo_from_tranid(tran.as_bytes());
    c.set_cdemo_from_program(pgm.as_bytes());
    if clear_keys_if_list && c.cdemo_last_mapset().starts_with(b"COCRDLI") {
        c.set_cdemo_acct_id_n(Dec::ZERO);
        c.set_cdemo_card_num_n(Dec::ZERO);
    }
    c.set_cdemo_user_type(b"U");
    c.set_cdemo_pgm_context(b"0");
    c.set_cdemo_last_mapset(mapset.as_bytes());
    c.set_cdemo_last_map(map.as_bytes());
    t.vsam.syncpoint();
    t.xctl(&to_pgm, Some(&c.buf))
}

fn sl_edit(st: &mut Sl, c: &mut CarddemoCommarea, map: &Map) {
    let a = map.get("ACCTSID");
    st.cc_acct = if a == b"*" || cobol::text::is_spaces(a) || a.first() == Some(&b'*') && a.len() == 1 { vec![0u8; 11] } else { let mut v = vec![b' '; 11]; text::mv(&mut v, a); v };
    let k = map.get("CARDSID");
    st.cc_card = if k.first() == Some(&b'*') && cobol::text::is_spaces(&k[1..]) || cobol::text::is_spaces(k) { vec![0u8; 16] } else { let mut v = vec![b' '; 16]; text::mv(&mut v, k); v };
    let zero = |b: &[u8]| is_numeric(b, false) && b.iter().all(|&x| x == b'0');
    // 2210-EDIT-ACCOUNT
    if blank(&st.cc_acct) || zero(&st.cc_acct) {
        st.input_error = true;
        st.acct = Flg::Blank;
        st.msg = "Account number not provided".into();
        c.set_cdemo_acct_id_n(Dec::ZERO);
    } else if !is_numeric(&st.cc_acct, false) {
        st.input_error = true;
        st.acct = Flg::NotOk;
        st.msg = "ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER".into();
        c.set_cdemo_acct_id_n(Dec::ZERO);
    } else {
        c.set_cdemo_acct_id(&st.cc_acct);
    }
    // 2220-EDIT-CARD
    if blank(&st.cc_card) || zero(&st.cc_card) {
        st.input_error = true;
        st.card = Flg::Blank;
        if st.msg.is_empty() {
            st.msg = "Card number not provided".into();
        }
        c.set_cdemo_card_num_n(Dec::ZERO);
    } else if !is_numeric(&st.cc_card, false) {
        st.input_error = true;
        st.card = Flg::NotOk;
        if st.msg.is_empty() {
            st.msg = "CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER".into();
        }
        c.set_cdemo_card_num_n(Dec::ZERO);
    } else {
        c.set_cdemo_card_num(&st.cc_card);
    }
}

fn sl_read(t: &mut Task, st: &mut Sl) {
    match t.read("CARDDAT", &st.cc_card, false) {
        Ok(r) => st.found = Some(CardRecord::from_bytes(&r)),
        Err(Resp::NotFnd) => {
            st.input_error = true;
            st.acct = Flg::NotOk;
            st.card = Flg::NotOk;
            if st.msg.is_empty() {
                st.msg = "Did not find cards for this search condition".into();
            }
        }
        Err(e) => {
            st.input_error = true;
            if st.msg.is_empty() {
                st.acct = Flg::NotOk;
            }
            st.msg = file_error("READ", "CARDDAT", e, 12);
        }
    }
}

fn sl_send(t: &mut Task, st: &mut Sl, c: &mut CarddemoCommarea, map: &mut Map) {
    screen_init(map, S_TRAN, S_PGM);
    let mut info = String::new();
    if t.eibcalen == 0 {
        info = "Please enter Account and Card Number".into();
    } else {
        if c.cdemo_acct_id_n() == Dec::ZERO {
            map.set_low("ACCTSID");
        } else {
            map.set("ACCTSID", &st.cc_acct);
        }
        if c.cdemo_card_num_n() == Dec::ZERO {
            map.set_low("CARDSID");
        } else {
            map.set("CARDSID", &st.cc_card);
        }
        if let Some(card) = &st.found {
            info = " Displaying requested details".into();
            map.set("CRDNAME", card.card_embossed_name());
            let exp = card.card_expiraion_date();
            map.set("EXPMON", &exp[5..7]);
            map.set("EXPYEAR", &exp[0..4]);
            map.set("CRDSTCD", card.card_active_status());
        }
    }
    if info.is_empty() {
        info = "Please enter Account and Card Number".into();
    }
    map.set_str("ERRMSG", &st.msg);
    map.set_str("INFOMSG", &info);
    // 1300-SETUP-SCREEN-ATTRS
    let from_list = c.cdemo_last_mapset().starts_with(b"COCRDLI") && c.cdemo_from_program() == b"COCRDLIC";
    let a = if from_list { AttrByte::ProtFset } else { AttrByte::UnprotFset };
    map.attr("ACCTSID", a);
    map.attr("CARDSID", a);
    match (st.acct, st.card) {
        (Flg::NotOk | Flg::Blank, _) => map.cursor("ACCTSID"),
        (_, Flg::NotOk | Flg::Blank) => map.cursor("CARDSID"),
        _ => map.cursor("ACCTSID"),
    }
    if from_list {
        map.default_color("ACCTSID");
        map.default_color("CARDSID");
    }
    if st.acct == Flg::NotOk {
        map.color("ACCTSID", Color::Red);
    }
    if st.card == Flg::NotOk {
        map.color("CARDSID", Color::Red);
    }
    if st.acct == Flg::Blank && reenter(c) {
        map.set_str("ACCTSID", "*");
        map.color("ACCTSID", Color::Red);
    }
    if st.card == Flg::Blank && reenter(c) {
        map.set_str("CARDSID", "*");
        map.color("CARDSID", Color::Red);
    }
    map.color("INFOMSG", Color::Neutral);
    c.set_cdemo_pgm_context(b"1");
    t.send_map(map, true);
}

// ===================================================================== COCRDUPC
const U_PGM: &str = "COCRDUPC";
const U_TRAN: &str = "CCUP";
const U_MAP: &str = "CCRDUPA";
/// WS-THIS-PROGCOMMAREA: action(1) + OLD(11+16+3+50+8+1) + NEW(same) + CARD-UPDATE-RECORD(150)
const U_PROG: usize = 1 + 89 + 89 + 150;
const O_OLD: usize = 1;
const O_NEW: usize = 90;

/// Offsets inside an OLD/NEW details block.
const D_ACCT: (usize, usize) = (0, 11);
const D_CARD: (usize, usize) = (11, 16);
const D_CVV: (usize, usize) = (27, 3);
const D_NAME: (usize, usize) = (30, 50);
const D_YEAR: (usize, usize) = (80, 4);
const D_MON: (usize, usize) = (84, 2);
const D_DAY: (usize, usize) = (86, 2);
const D_STS: (usize, usize) = (88, 1);

struct Upd {
    prog: Vec<u8>,
    input_error: bool,
    acct: Flg,
    card: Flg,
    name: Flg,
    sts: Flg,
    mon: Flg,
    year: Flg,
    msg: String,
    info: &'static str,
    cc_acct: Vec<u8>,
    cc_card: Vec<u8>,
    found: bool,
    no_change: bool,
}

impl Upd {
    fn action(&self) -> u8 {
        self.prog[0]
    }
    fn set_action(&mut self, a: u8) {
        self.prog[0] = a;
    }
    fn not_fetched(&self) -> bool {
        matches!(self.action(), 0 | b' ')
    }
    fn d(&self, base: usize, f: (usize, usize)) -> Vec<u8> {
        self.prog[base + f.0..base + f.0 + f.1].to_vec()
    }
    fn set_d(&mut self, base: usize, f: (usize, usize), v: &[u8]) {
        text::mv(&mut self.prog[base + f.0..base + f.0 + f.1], v);
    }
    fn fill_d(&mut self, base: usize, f: (usize, usize), b: u8) {
        self.prog[base + f.0..base + f.0 + f.1].fill(b);
    }
}

fn new_upd_prog() -> Vec<u8> {
    let mut p = vec![b' '; U_PROG];
    p[0] = 0;
    // CARD-UPDATE-RECORD numeric fields
    let cu = O_NEW + 89;
    p[cu + 16..cu + 27].fill(b'0');
    p[cu + 27..cu + 30].fill(b'0');
    p
}

pub fn cocrdupc(t: &mut Task) -> Cr {
    let aid = store_pfkey(t.eibaid);
    let (c0, p0) = split(&t.commarea, U_PROG);
    let mut c;
    let mut s = Upd {
        prog: p0,
        input_error: false,
        acct: Flg::Blank,
        card: Flg::Blank,
        name: Flg::Blank,
        sts: Flg::Blank,
        mon: Flg::Blank,
        year: Flg::Blank,
        msg: String::new(),
        info: "",
        cc_acct: vec![b' '; 11],
        cc_card: vec![b' '; 16],
        found: false,
        no_change: false,
    };
    if t.eibcalen == 0 || (c0.cdemo_from_program() == b"COMEN01C" && !reenter(&c0)) {
        c = CarddemoCommarea::new();
        c.set_cdemo_pgm_context(b"0");
        s.prog = new_upd_prog();
    } else {
        c = c0;
    }
    let valid = aid == Aid::Enter
        || aid == Aid::Pf(3)
        || (aid == Aid::Pf(5) && s.action() == b'N')
        || (aid == Aid::Pf(12) && !s.not_fetched());
    let aid = if valid { aid } else { Aid::Enter };
    let from_list_mapset = c.cdemo_last_mapset().starts_with(b"COCRDLI");
    let done_or_failed = matches!(s.action(), b'C' | b'L' | b'F');
    if aid == Aid::Pf(3) || (done_or_failed && from_list_mapset) {
        return Err(pf3_back(t, &mut c, U_TRAN, U_PGM, "COCRDUP", U_MAP, true));
    }
    let mut map = t.map(U_MAP);
    let from_list = c.cdemo_from_program() == b"COCRDLIC";
    if (!reenter(&c) && from_list) || (aid == Aid::Pf(12) && from_list) {
        c.set_cdemo_pgm_context(b"1");
        s.acct = Flg::Valid;
        s.card = Flg::Valid;
        s.cc_acct = format!("{:011}", c.cdemo_acct_id_n()).into_bytes();
        s.cc_card = format!("{:016}", c.cdemo_card_num_n()).into_bytes();
        upd_read(t, &mut s);
        s.set_action(b'S');
        upd_send(t, &mut s, &c, &mut map);
    } else if (s.not_fetched() && !reenter(&c)) || (c.cdemo_from_program() == b"COMEN01C" && !reenter(&c)) {
        s.prog = new_upd_prog();
        upd_send(t, &mut s, &c, &mut map);
        c.set_cdemo_pgm_context(b"1");
        s.set_action(0);
    } else if done_or_failed {
        s.prog = new_upd_prog();
        c.set_cdemo_acct_id_n(Dec::ZERO);
        c.set_cdemo_card_num_n(Dec::ZERO);
        c.set_cdemo_pgm_context(b"0");
        upd_send(t, &mut s, &c, &mut map);
        c.set_cdemo_pgm_context(b"1");
        s.set_action(0);
    } else {
        upd_receive(t, &mut s, &mut c, &mut map);
        upd_decide(t, &mut s, &mut c, aid);
        upd_send(t, &mut s, &c, &mut map);
    }
    Err(t.ret(Some(U_TRAN), Some(&join(&c, &s.prog))))
}

fn upd_receive(t: &mut Task, s: &mut Upd, c: &mut CarddemoCommarea, map: &mut Map) {
    *map = t.receive_map(U_MAP);
    // INITIALIZE CCUP-NEW-DETAILS
    for f in [D_ACCT, D_CARD, D_CVV, D_NAME, D_YEAR, D_MON, D_DAY, D_STS] {
        s.fill_d(O_NEW, f, b' ');
    }
    let star_or_blank = |v: &[u8]| (v.first() == Some(&b'*') && text::is_spaces(&v[1..])) || text::is_spaces(v);
    let take = |v: &[u8], len: usize| -> Vec<u8> {
        if star_or_blank(v) { vec![0u8; len] } else { let mut x = vec![b' '; len]; text::mv(&mut x, v); x }
    };
    s.cc_acct = take(map.get("ACCTSID"), 11);
    s.set_d(O_NEW, D_ACCT, &s.cc_acct.clone());
    s.cc_card = take(map.get("CARDSID"), 16);
    s.set_d(O_NEW, D_CARD, &s.cc_card.clone());
    let v = take(map.get("CRDNAME"), 50);
    s.set_d(O_NEW, D_NAME, &v);
    let v = take(map.get("CRDSTCD"), 1);
    s.set_d(O_NEW, D_STS, &v);
    let v = map.get("EXPDAY").to_vec();
    s.set_d(O_NEW, D_DAY, &v);
    let v = take(map.get("EXPMON"), 2);
    s.set_d(O_NEW, D_MON, &v);
    let v = take(map.get("EXPYEAR"), 4);
    s.set_d(O_NEW, D_YEAR, &v);
    // 1200-EDIT-MAP-INPUTS
    let zero = |b: &[u8]| is_numeric(b, false) && b.iter().all(|&x| x == b'0');
    if s.not_fetched() {
        // 1210 / 1220
        if blank(&s.cc_acct) || zero(&s.cc_acct) {
            s.input_error = true;
            s.acct = Flg::Blank;
            s.msg = "Account number not provided".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
            s.fill_d(O_NEW, D_ACCT, 0);
        } else if !is_numeric(&s.cc_acct, false) {
            s.input_error = true;
            s.acct = Flg::NotOk;
            s.msg = "ACCOUNT FILTER,IF SUPPLIED MUST BE A 11 DIGIT NUMBER".into();
            c.set_cdemo_acct_id_n(Dec::ZERO);
            s.fill_d(O_NEW, D_ACCT, 0);
        } else {
            c.set_cdemo_acct_id(&s.cc_acct);
            s.acct = Flg::Valid;
        }
        if blank(&s.cc_card) || zero(&s.cc_card) {
            s.input_error = true;
            s.card = Flg::Blank;
            if s.msg.is_empty() {
                s.msg = "Card number not provided".into();
            }
            c.set_cdemo_card_num_n(Dec::ZERO);
        } else if !is_numeric(&s.cc_card, false) {
            s.input_error = true;
            s.card = Flg::NotOk;
            if s.msg.is_empty() {
                s.msg = "CARD ID FILTER,IF SUPPLIED MUST BE A 16 DIGIT NUMBER".into();
            }
            c.set_cdemo_card_num_n(Dec::ZERO);
            s.fill_d(O_NEW, D_CARD, 0);
        } else {
            c.set_cdemo_card_num(&s.cc_card);
            s.card = Flg::Valid;
        }
        for f in [D_NAME, D_YEAR, D_MON, D_DAY, D_STS] {
            s.fill_d(O_NEW, f, 0);
        }
        return;
    }
    s.found = true;
    s.acct = Flg::Valid;
    s.card = Flg::Valid;
    c.set_cdemo_acct_id(&s.d(O_OLD, D_ACCT));
    c.set_cdemo_card_num(&s.d(O_OLD, D_CARD));
    let new_data: Vec<u8> = s.prog[O_NEW + 30..O_NEW + 89].to_ascii_uppercase();
    let old_data: Vec<u8> = s.prog[O_OLD + 30..O_OLD + 89].to_ascii_uppercase();
    if new_data == old_data {
        s.no_change = true;
        s.msg = "No change detected with respect to values fetched.".into();
    }
    if s.no_change || matches!(s.action(), b'N' | b'C') {
        s.name = Flg::Valid;
        s.sts = Flg::Valid;
        s.mon = Flg::Valid;
        s.year = Flg::Valid;
        return;
    }
    s.set_action(b'E');
    // 1230-EDIT-NAME
    let name = s.d(O_NEW, D_NAME);
    if blank(&name) || text::is_zeros(&name) {
        s.input_error = true;
        s.name = Flg::Blank;
        if s.msg.is_empty() {
            s.msg = "Card name not provided".into();
        }
    } else if name.iter().all(|c| c.is_ascii_alphabetic() || *c == b' ') {
        s.name = Flg::Valid;
    } else {
        s.input_error = true;
        s.name = Flg::NotOk;
        if s.msg.is_empty() {
            s.msg = "Card name can only contain alphabets and spaces".into();
        }
    }
    // 1240-EDIT-CARDSTATUS
    let st = s.d(O_NEW, D_STS);
    if blank(&st) || st == b"0" {
        s.input_error = true;
        s.sts = Flg::Blank;
        if s.msg.is_empty() {
            s.msg = "Card Active Status must be Y or N".into();
        }
    } else if st == b"Y" || st == b"N" {
        s.sts = Flg::Valid;
    } else {
        s.input_error = true;
        s.sts = Flg::NotOk;
        if s.msg.is_empty() {
            s.msg = "Card Active Status must be Y or N".into();
        }
    }
    // 1250-EDIT-EXPIRY-MON
    let m = s.d(O_NEW, D_MON);
    let mn: Option<u32> = if is_numeric(&m, false) { text::s(&m).parse().ok() } else { None };
    if blank(&m) || text::is_zeros(&m) {
        s.input_error = true;
        s.mon = Flg::Blank;
        if s.msg.is_empty() {
            s.msg = "Card expiry month must be between 1 and 12".into();
        }
    } else if mn.is_some_and(|v| (1..=12).contains(&v)) {
        s.mon = Flg::Valid;
    } else {
        s.input_error = true;
        s.mon = Flg::NotOk;
        if s.msg.is_empty() {
            s.msg = "Card expiry month must be between 1 and 12".into();
        }
    }
    // 1260-EDIT-EXPIRY-YEAR
    let y = s.d(O_NEW, D_YEAR);
    let yn: Option<u32> = if is_numeric(&y, false) { text::s(&y).parse().ok() } else { None };
    if blank(&y) || text::is_zeros(&y) {
        s.input_error = true;
        s.year = Flg::Blank;
        if s.msg.is_empty() {
            s.msg = "Invalid card expiry year".into();
        }
    } else if yn.is_some_and(|v| (1950..=2099).contains(&v)) {
        s.year = Flg::Valid;
    } else {
        s.input_error = true;
        s.year = Flg::NotOk;
        if s.msg.is_empty() {
            s.msg = "Invalid card expiry year".into();
        }
    }
    if !s.input_error {
        s.set_action(b'N');
    }
}

fn upd_read(t: &mut Task, s: &mut Upd) {
    for f in [D_ACCT, D_CARD, D_CVV, D_NAME, D_YEAR, D_MON, D_DAY, D_STS] {
        s.fill_d(O_OLD, f, b' ');
    }
    s.set_d(O_OLD, D_ACCT, &s.cc_acct.clone());
    s.set_d(O_OLD, D_CARD, &s.cc_card.clone());
    match t.read("CARDDAT", &s.cc_card, false) {
        Ok(r) => {
            let card = CardRecord::from_bytes(&r);
            s.found = true;
            s.set_d(O_OLD, D_CVV, card.card_cvv_cd());
            s.set_d(O_OLD, D_NAME, &card.card_embossed_name().to_ascii_uppercase());
            let e = card.card_expiraion_date();
            s.set_d(O_OLD, D_YEAR, &e[0..4]);
            s.set_d(O_OLD, D_MON, &e[5..7]);
            s.set_d(O_OLD, D_DAY, &e[8..10]);
            s.set_d(O_OLD, D_STS, card.card_active_status());
        }
        Err(Resp::NotFnd) => {
            s.input_error = true;
            s.acct = Flg::NotOk;
            s.card = Flg::NotOk;
            if s.msg.is_empty() {
                s.msg = "Did not find cards for this search condition".into();
            }
        }
        Err(e) => {
            s.input_error = true;
            if s.msg.is_empty() {
                s.acct = Flg::NotOk;
            }
            s.msg = file_error("READ", "CARDDAT", e, 12);
        }
    }
}

fn upd_decide(t: &mut Task, s: &mut Upd, c: &mut CarddemoCommarea, aid: Aid) {
    match s.action() {
        _ if s.not_fetched() || aid == Aid::Pf(12) => {
            if s.acct == Flg::Valid && s.card == Flg::Valid {
                upd_read(t, s);
                if s.found {
                    s.set_action(b'S');
                }
            }
        }
        b'S' => {
            if !(s.input_error || s.no_change) {
                s.set_action(b'N');
            }
        }
        b'E' => {}
        b'N' if aid == Aid::Pf(5) => match upd_write(t, s) {
            UpdResult::LockErr => s.set_action(b'L'),
            UpdResult::Failed => s.set_action(b'F'),
            UpdResult::Changed => s.set_action(b'S'),
            UpdResult::Ok => s.set_action(b'C'),
        },
        b'N' => {}
        b'C' => {
            s.set_action(b'S');
            if blank(c.cdemo_from_tranid()) {
                c.set_cdemo_acct_id_n(Dec::ZERO);
                c.set_cdemo_card_num_n(Dec::ZERO);
                c.set_cdemo_acct_status(&[0]);
            }
        }
        _ => {}
    }
}

enum UpdResult {
    Ok,
    LockErr,
    Failed,
    Changed,
}

fn upd_write(t: &mut Task, s: &mut Upd) -> UpdResult {
    let key = s.cc_card.clone();
    let rec = match t.read("CARDDAT", &key, true) {
        Ok(r) => r,
        Err(_) => {
            s.input_error = true;
            if s.msg.is_empty() {
                s.msg = "Could not lock record for update".into();
            }
            return UpdResult::LockErr;
        }
    };
    let card = CardRecord::from_bytes(&rec);
    let name = card.card_embossed_name().to_ascii_uppercase();
    let e = card.card_expiraion_date();
    if !(card.card_cvv_cd() == &s.d(O_OLD, D_CVV)[..]
        && name == s.d(O_OLD, D_NAME)
        && e[0..4] == s.d(O_OLD, D_YEAR)[..]
        && e[5..7] == s.d(O_OLD, D_MON)[..]
        && e[8..10] == s.d(O_OLD, D_DAY)[..]
        && card.card_active_status() == &s.d(O_OLD, D_STS)[..])
    {
        s.msg = "Record changed by some one else. Please review".into();
        s.set_d(O_OLD, D_CVV, card.card_cvv_cd());
        s.set_d(O_OLD, D_NAME, &name);
        s.set_d(O_OLD, D_YEAR, &e[0..4]);
        s.set_d(O_OLD, D_MON, &e[5..7]);
        s.set_d(O_OLD, D_DAY, &e[8..10]);
        s.set_d(O_OLD, D_STS, card.card_active_status());
        return UpdResult::Changed;
    }
    let mut u = CardRecord::new();
    u.set_card_num(&s.d(O_NEW, D_CARD));
    u.set_card_acct_id(&s.cc_acct);
    // KD-07: CCUP-NEW-CVV-CD is never filled; the CVV is written as zero
    u.set_card_cvv_cd_n(Dec::ZERO);
    u.set_card_embossed_name(&s.d(O_NEW, D_NAME));
    let mut exp = s.d(O_NEW, D_YEAR);
    exp.push(b'-');
    exp.extend(s.d(O_NEW, D_MON));
    exp.push(b'-');
    exp.extend(s.d(O_NEW, D_DAY));
    u.set_card_expiraion_date(&exp);
    u.set_card_active_status(&s.d(O_NEW, D_STS));
    match t.rewrite("CARDDAT", &u.buf) {
        Ok(()) => UpdResult::Ok,
        Err(_) => {
            s.msg = "Update of record failed".into();
            UpdResult::Failed
        }
    }
}

fn upd_send(t: &mut Task, s: &mut Upd, c: &CarddemoCommarea, map: &mut Map) {
    screen_init(map, U_TRAN, U_PGM);
    // 3200-SETUP-SCREEN-VARS
    if reenter(c) || c.cdemo_pgm_context() != b"0" {
        if s.cc_acct.iter().all(|&b| b == b'0' || b == 0 || b == b' ') {
            map.set_low("ACCTSID");
        } else {
            map.set("ACCTSID", &s.cc_acct);
        }
        if s.cc_card.iter().all(|&b| b == b'0' || b == 0 || b == b' ') {
            map.set_low("CARDSID");
        } else {
            map.set("CARDSID", &s.cc_card);
        }
        let (base, show_day_old) = match s.action() {
            0 | b' ' => (None, false),
            b'S' => (Some(O_OLD), true),
            b'E' | b'N' | b'C' | b'L' | b'F' => (Some(O_NEW), true),
            _ => (Some(O_OLD), true),
        };
        if let Some(b) = base {
            map.set("CRDNAME", &s.d(b, D_NAME));
            map.set("CRDSTCD", &s.d(b, D_STS));
            map.set("EXPMON", &s.d(b, D_MON));
            map.set("EXPYEAR", &s.d(b, D_YEAR));
            if show_day_old {
                map.set("EXPDAY", &s.d(O_OLD, D_DAY));
            }
        }
    }
    // 3250-SETUP-INFOMSG
    s.info = match s.action() {
        _ if c.cdemo_pgm_context() == b"0" => "Please enter Account and Card Number",
        0 | b' ' => "Please enter Account and Card Number",
        b'S' => "Details of selected card shown above",
        b'E' => "Update card details presented above.",
        b'N' => "Changes validated.Press F5 to save",
        b'C' => "Changes committed to database",
        b'L' | b'F' => "Changes unsuccessful. Please try again",
        _ => "Please enter Account and Card Number",
    };
    map.set_str("INFOMSG", s.info);
    map.set_str("ERRMSG", &s.msg);
    // 3300-SETUP-SCREEN-ATTRS
    let edit_fields = ["CRDNAME", "CRDSTCD", "EXPMON", "EXPYEAR"];
    match s.action() {
        0 | b' ' => {
            map.attr("ACCTSID", AttrByte::UnprotFset);
            map.attr("CARDSID", AttrByte::UnprotFset);
            for f in edit_fields {
                map.attr(f, AttrByte::ProtFset);
            }
        }
        b'S' | b'E' => {
            map.attr("ACCTSID", AttrByte::ProtFset);
            map.attr("CARDSID", AttrByte::ProtFset);
            for f in edit_fields {
                map.attr(f, AttrByte::UnprotFset);
            }
        }
        b'N' | b'C' => {
            for f in ["ACCTSID", "CARDSID"].iter().chain(edit_fields.iter()) {
                map.attr(f, AttrByte::ProtFset);
            }
        }
        _ => {
            map.attr("ACCTSID", AttrByte::UnprotFset);
            map.attr("CARDSID", AttrByte::UnprotFset);
            for f in edit_fields {
                map.attr(f, AttrByte::ProtFset);
            }
        }
    }
    let cursor = if s.found || s.no_change {
        "CRDNAME"
    } else if matches!(s.acct, Flg::NotOk | Flg::Blank) {
        "ACCTSID"
    } else if matches!(s.card, Flg::NotOk | Flg::Blank) {
        "CARDSID"
    } else if matches!(s.name, Flg::NotOk) {
        "CRDNAME"
    } else if matches!(s.sts, Flg::NotOk) {
        "CRDSTCD"
    } else if matches!(s.mon, Flg::NotOk) {
        "EXPMON"
    } else if matches!(s.year, Flg::NotOk) {
        "EXPYEAR"
    } else {
        "ACCTSID"
    };
    map.cursor(cursor);
    if c.cdemo_last_mapset().starts_with(b"COCRDLI") {
        map.default_color("ACCTSID");
        map.default_color("CARDSID");
    }
    let reent = reenter(c);
    for (fld, flg, field_star) in [("ACCTSID", s.acct, reent), ("CARDSID", s.card, reent)] {
        if flg == Flg::NotOk {
            map.color(fld, Color::Red);
        }
        if flg == Flg::Blank && field_star && s.not_fetched() {
            map.set_str(fld, "*");
            map.color(fld, Color::Red);
        }
    }
    if s.action() == b'E' {
        for (fld, flg) in [("CRDNAME", s.name), ("CRDSTCD", s.sts), ("EXPMON", s.mon), ("EXPYEAR", s.year)] {
            if flg == Flg::NotOk {
                map.color(fld, Color::Red);
            }
            if flg == Flg::Blank {
                map.set_str(fld, "*");
                map.color(fld, Color::Red);
            }
        }
    }
    map.attr("EXPDAY", AttrByte::Dark);
    map.attr("INFOMSG", AttrByte::Bright);
    if s.action() == b'N' {
        map.attr("FKEYSC", AttrByte::Bright);
    }
    t.send_map(map, true);
}
