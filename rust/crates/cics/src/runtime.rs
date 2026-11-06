//! Task execution: EIB, COMMAREA, XCTL / RETURN, file control, BMS, TD queue.
//!
//! A program is `fn(&mut Task) -> Result<(), Transfer>`. XCTL and RETURN end the
//! program immediately, which is modelled as returning `Err(Transfer)` (propagated
//! with `?` from any paragraph). SEND MAP does not end the program; the last screen
//! sent is what the terminal shows.

use crate::bms::MapCatalog;
use crate::screen::{Map, Screen};
use std::collections::HashMap;
use vsam::{Browse, Resp, Vsam};

/// Attention identifier (EIBAID).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aid {
    Enter,
    Clear,
    Pa1,
    Pa2,
    Pf(u8),
}

#[derive(Debug, Clone)]
pub enum Transfer {
    Xctl { program: String, commarea: Option<Vec<u8>> },
    Return { transid: Option<String>, commarea: Option<Vec<u8>> },
    Abend(String),
}

pub type Cr<T = ()> = Result<T, Transfer>;
pub type Program = fn(&mut Task) -> Cr;

/// Messages sent to the terminal with SEND TEXT end up here too.
pub struct Task<'a> {
    pub vsam: &'a Vsam,
    maps: &'a MapCatalog,
    programs: &'a HashMap<&'static str, Program>,
    pub program: String,
    pub eibcalen: usize,
    pub eibaid: Aid,
    pub eibresp: i32,
    pub commarea: Vec<u8>,
    terminal: Screen,
    pub sent: Option<Screen>,
    held: HashMap<String, Vec<u8>>,
    pub applid: String,
    pub sysid: String,
    pub tdq: Vec<(String, Vec<u8>)>,
    pub log: Vec<String>,
}

/// Consumer of the `JOBS` TD queue: receives the card images of one submitted job.
pub type JobReader = Box<dyn Fn(&[Vec<u8>]) + Send + Sync>;

impl<'a> Task<'a> {
    // ---- program control -------------------------------------------------
    pub fn xctl(&self, program: &str, commarea: Option<&[u8]>) -> Transfer {
        Transfer::Xctl { program: program.trim().to_string(), commarea: commarea.map(|c| c.to_vec()) }
    }
    pub fn ret(&self, transid: Option<&str>, commarea: Option<&[u8]>) -> Transfer {
        Transfer::Return { transid: transid.map(|t| t.to_string()), commarea: commarea.map(|c| c.to_vec()) }
    }
    pub fn abend(&self, code: &str) -> Transfer {
        Transfer::Abend(code.to_string())
    }
    /// INQUIRE PROGRAM: is the program installed?
    pub fn program_exists(&self, name: &str) -> bool {
        self.programs.contains_key(name.trim())
    }
    /// DISPLAY in a CICS program goes to the job log
    pub fn display(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
    }

    // ---- BMS -------------------------------------------------------------
    /// A fresh symbolic map (all LOW-VALUES).
    pub fn map(&self, name: &str) -> Map {
        Map::new(self.maps.get(name).unwrap_or_else(|| panic!("map {name} not defined")))
    }
    /// RECEIVE MAP
    pub fn receive_map(&mut self, name: &str) -> Map {
        let def = self.maps.get(name).unwrap_or_else(|| panic!("map {name} not defined"));
        Map::from_screen(def, &self.terminal)
    }
    /// SEND MAP [ERASE] [CURSOR]
    pub fn send_map(&mut self, map: &Map, erase: bool) {
        let prev = self.sent.as_ref().or(Some(&self.terminal)).cloned();
        let s = map.to_screen(prev.as_ref(), erase);
        self.sent = Some(s);
    }
    /// SEND TEXT ERASE
    pub fn send_text(&mut self, text: &[u8]) {
        self.sent = Some(Screen { text: Some(cobol::text::s(text)), ..Default::default() });
    }

    // ---- file control ----------------------------------------------------
    /// READ [UPDATE]
    pub fn read(&mut self, file: &str, key: &[u8], update: bool) -> Result<Vec<u8>, Resp> {
        let r = self.vsam.read(file.trim(), key);
        self.eibresp = r.as_ref().map_or_else(|e| e.code(), |_| 0);
        if update {
            if let Ok(rec) = &r {
                let p = self.vsam.path(file.trim())?;
                self.held.insert(file.trim().to_string(), rec[p.ds.key_off..p.ds.key_off + p.ds.key_len].to_vec());
            }
        }
        r
    }
    pub fn write(&mut self, file: &str, rec: &[u8]) -> Result<(), Resp> {
        self.vsam.write(file.trim(), rec)
    }
    /// REWRITE (requires a prior READ UPDATE on the file)
    pub fn rewrite(&mut self, file: &str, rec: &[u8]) -> Result<(), Resp> {
        if self.held.remove(file.trim()).is_none() {
            return Err(Resp::InvReq);
        }
        self.vsam.rewrite(file.trim(), rec)
    }
    /// DELETE with RIDFLD, or of the record held by READ UPDATE when `key` is None
    pub fn delete(&mut self, file: &str, key: Option<&[u8]>) -> Result<(), Resp> {
        let k = match key {
            Some(k) => k.to_vec(),
            None => self.held.remove(file.trim()).ok_or(Resp::InvReq)?,
        };
        self.vsam.delete(file.trim(), &k)
    }
    pub fn startbr(&mut self, file: &str, key: &[u8], gteq: bool) -> Result<Browse, Resp> {
        self.vsam.start_browse(file.trim(), key, gteq)
    }
    pub fn readnext(&mut self, br: &mut Browse) -> Result<Vec<u8>, Resp> {
        self.vsam.read_next(br)
    }
    pub fn readprev(&mut self, br: &mut Browse) -> Result<Vec<u8>, Resp> {
        self.vsam.read_prev(br)
    }
    pub fn syncpoint(&mut self) {
        self.vsam.syncpoint();
        self.held.clear();
    }
    pub fn syncpoint_rollback(&mut self) {
        self.vsam.syncpoint_rollback();
        self.held.clear();
    }
    /// WRITEQ TD
    pub fn writeq_td(&mut self, queue: &str, data: &[u8]) {
        self.tdq.push((queue.to_string(), data.to_vec()));
    }
}

/// The CICS region: programs, transactions and resources.
pub struct Region {
    pub vsam: Vsam,
    pub maps: MapCatalog,
    pub programs: HashMap<&'static str, Program>,
    pub transactions: HashMap<&'static str, &'static str>,
    pub applid: String,
    pub sysid: String,
    /// TD queue `JOBS` consumer (internal reader); receives each JCL deck submitted
    pub on_job: Option<JobReader>,
}

/// A terminal session (pseudo-conversation state).
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub screen: Screen,
    pub next_transid: Option<String>,
    pub commarea: Vec<u8>,
    pub log: Vec<String>,
}

impl Region {
    /// Run a task for `transid` (or the pending RETURN TRANSID) on `aid`.
    pub fn run(&self, sess: &mut Session, transid: Option<&str>, aid: Aid) {
        let (tran, commarea) = match transid {
            Some(t) => (t.to_string(), Vec::new()),
            None => match &sess.next_transid {
                Some(t) => (t.clone(), sess.commarea.clone()),
                None => ("CC00".to_string(), Vec::new()),
            },
        };
        let Some(&pgm) = self.transactions.get(tran.as_str()) else {
            sess.screen = Screen { text: Some(format!("DFHAC2001 Transaction '{tran}' is not recognized.")), ..Default::default() };
            sess.next_transid = None;
            return;
        };
        self.vsam.begin();
        let mut task = Task {
            vsam: &self.vsam,
            maps: &self.maps,
            programs: &self.programs,
            program: pgm.to_string(),
            eibcalen: commarea.len(),
            eibaid: aid,
            eibresp: 0,
            commarea,
            terminal: sess.screen.clone(),
            sent: None,
            held: HashMap::new(),
            applid: self.applid.clone(),
            sysid: self.sysid.clone(),
            tdq: Vec::new(),
            log: Vec::new(),
        };
        let mut hops = 0;
        let outcome = loop {
            let f = match self.programs.get(task.program.as_str()) {
                Some(f) => *f,
                None => break Transfer::Abend(format!("APCT {}", task.program)),
            };
            let r = f(&mut task);
            match r {
                Ok(()) => break Transfer::Return { transid: None, commarea: None },
                Err(Transfer::Xctl { program, commarea }) => {
                    hops += 1;
                    if hops > 50 {
                        break Transfer::Abend("LOOP".into());
                    }
                    task.program = program;
                    task.commarea = commarea.unwrap_or_default();
                    task.eibcalen = task.commarea.len();
                    task.held.clear();
                }
                Err(t) => break t,
            }
        };
        match outcome {
            Transfer::Return { transid, commarea } => {
                self.vsam.commit();
                if let Some(s) = task.sent.take() {
                    sess.screen = s;
                }
                sess.next_transid = transid;
                sess.commarea = commarea.unwrap_or_default();
                // spool TD queue JOBS to the internal reader
                let deck: Vec<Vec<u8>> = task.tdq.iter().filter(|(q, _)| q == "JOBS").map(|(_, d)| d.clone()).collect();
                if !deck.is_empty() {
                    if let Some(cb) = &self.on_job {
                        cb(&deck);
                    }
                }
            }
            Transfer::Abend(code) => {
                self.vsam.rollback();
                sess.screen = Screen {
                    text: Some(format!("DFHAC2206 Transaction {tran} failed with abend {code}. Updates to local recoverable resources backed out.")),
                    ..Default::default()
                };
                sess.next_transid = None;
                sess.commarea.clear();
            }
            Transfer::Xctl { .. } => unreachable!(),
        }
        sess.log.extend(task.log);
    }
}
