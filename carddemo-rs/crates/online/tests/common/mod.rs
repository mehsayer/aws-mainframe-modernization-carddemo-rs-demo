#![allow(dead_code)]
use cics::{Aid, Region, Session};
use std::path::PathBuf;

pub fn region() -> Region {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    let v = vsam::Vsam::open_in_memory().unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../app/data/EBCDIC");
    batch::repro::reset(&v, &seed).unwrap();
    online::region(v)
}

pub struct Term<'a> {
    pub r: &'a Region,
    pub s: Session,
}

impl<'a> Term<'a> {
    pub fn new(r: &'a Region) -> Self {
        let mut s = Session::default();
        r.run(&mut s, Some("CC00"), Aid::Enter);
        Term { r, s }
    }
    pub fn typ(&mut self, field: &str, v: &str) -> &mut Self {
        self.s.screen.type_into(field, v);
        self
    }
    pub fn key(&mut self, aid: Aid) -> &mut Self {
        self.r.run(&mut self.s, None, aid);
        self
    }
    pub fn enter(&mut self) -> &mut Self {
        self.key(Aid::Enter)
    }
    pub fn field(&self, name: &str) -> String {
        self.s.screen.field(name).map(|f| cobol::text::s(&f.data).trim_end().to_string()).unwrap_or_default()
    }
    pub fn msg(&self) -> String {
        self.field("ERRMSG")
    }
    pub fn map(&self) -> &str {
        &self.s.screen.map
    }
    pub fn dump(&self) -> String {
        self.s.screen.render().join("\n")
    }
    pub fn signon(r: &'a Region, user: &str, pw: &str) -> Self {
        let mut t = Term::new(r);
        t.typ("USERID", user).typ("PASSWD", pw).enter();
        t
    }
}
