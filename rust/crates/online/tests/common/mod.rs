#![allow(dead_code)]
use cics::{Aid, Region, Session};
use std::path::PathBuf;

/// The online region under test: the Rust programs, or with `CARDDEMO_REGION=cobol` the original
/// COBOL programs on the CICS bridge (run those tests with `--test-threads=1`: libcob is single-threaded).
pub fn region() -> Region {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    let v = vsam::Vsam::open_in_memory().unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cobol/app/data/EBCDIC");
    batch::repro::reset(&v, &seed).unwrap();
    if cobol_region() {
        // the COBOL programs' DISPLAY output goes to a file, not into the test harness output
        let dir = std::env::var_os("CARDDEMO_SCREEN_LOG").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        cobridge::set_sysout(&dir.join("cobol-sysout.log")).expect("cobol sysout file");
        cobridge::region(v)
    } else {
        online::region(v)
    }
}

pub fn cobol_region() -> bool {
    std::env::var("CARDDEMO_REGION").is_ok_and(|v| v == "cobol")
}

/// With `CARDDEMO_SCREEN_LOG=<dir>`, every screen a test sees is appended to `<dir>/<test name>.log`
/// (text, map, cursor, and each field's attributes), so a Rust run and a COBOL run can be compared.
pub fn log_screen(s: &Session) {
    let Some(dir) = std::env::var_os("CARDDEMO_SCREEN_LOG") else { return };
    use std::io::Write;
    let name = std::thread::current().name().unwrap_or("main").replace("::", "-");
    let mut out = String::new();
    match &s.screen.text {
        Some(t) => out.push_str(&format!("TEXT {t}\n")),
        None => {
            out.push_str(&format!("MAP {} CURSOR {:?} NEXT {:?}\n", s.screen.map, s.screen.cursor, s.next_transid));
            for l in s.screen.render() {
                out.push_str(&l);
                out.push('\n');
            }
            for f in &s.screen.fields {
                if let Some(n) = &f.name {
                    out.push_str(&format!("  {n} {:?} {:?} prot={} dark={} bright={}\n", cobol::text::s(&f.data), f.color, f.attr.protected, f.attr.dark, f.attr.bright));
                }
            }
        }
    }
    out.push_str("----\n");
    let path = std::path::Path::new(&dir).join(format!("{name}.log"));
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    f.write_all(out.as_bytes()).unwrap();
}

pub struct Term<'a> {
    pub r: &'a Region,
    pub s: Session,
}

impl<'a> Term<'a> {
    pub fn new(r: &'a Region) -> Self {
        let mut s = Session::default();
        r.run(&mut s, Some("CC00"), Aid::Enter);
        log_screen(&s);
        Term { r, s }
    }
    pub fn typ(&mut self, field: &str, v: &str) -> &mut Self {
        self.s.screen.type_into(field, v);
        self
    }
    pub fn key(&mut self, aid: Aid) -> &mut Self {
        self.r.run(&mut self.s, None, aid);
        log_screen(&self.s);
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
