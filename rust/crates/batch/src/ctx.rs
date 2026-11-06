//! Batch step runtime: data-set resolution (VSAM clusters in the SQLite store,
//! sequential data sets and GDG generations as files), SYSOUT, PARM and clock.

use crate::seqfile::Format;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use vsam::Vsam;

/// Abnormal end of a batch program (CEE3ABD / abend code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abend(pub i32);

impl std::fmt::Display for Abend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ABEND U{:04}", self.0)
    }
}
impl std::error::Error for Abend {}

pub type StepResult = Result<i32, Abend>;

/// What a DD statement points at.
#[derive(Clone, Debug)]
pub enum Dd {
    /// VSAM cluster or path in the store (dataset or CICS-style path name)
    Vsam(String),
    /// sequential file with its format
    File(PathBuf, Format),
    /// in-stream data (`DD *`)
    Instream(Vec<Vec<u8>>),
}

/// Clock used for FUNCTION CURRENT-DATE etc. Overridable for reproducible runs
/// (env `CARDDEMO_NOW=YYYY-MM-DDTHH:MM:SS[.ff]`).
pub fn now() -> NaiveDateTime {
    if let Ok(s) = std::env::var("CARDDEMO_NOW") {
        for f in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
            if let Ok(t) = NaiveDateTime::parse_from_str(&s, f) {
                return t;
            }
        }
    }
    chrono::Local::now().naive_local()
}

/// FUNCTION CURRENT-DATE (first 16 chars used by CardDemo: YYYYMMDDHHMMSScc).
pub fn current_date() -> String {
    let t = now();
    format!("{}{:02}+0000", t.format("%Y%m%d%H%M%S"), t.and_utc().timestamp_subsec_millis() / 10)
}

/// The DB2-format timestamp built by Z-GET-DB2-FORMAT-TIMESTAMP (BR-BAT-002).
pub fn db2_timestamp() -> String {
    let c = current_date();
    format!("{}-{}-{}-{}.{}.{}.{}0000", &c[0..4], &c[4..6], &c[6..8], &c[8..10], &c[10..12], &c[12..14], &c[14..16])
}

pub struct StepCtx<'a> {
    pub vsam: &'a Vsam,
    pub dds: HashMap<String, Dd>,
    pub parm: String,
    pub sysout: Vec<String>,
    pub echo: bool,
}

impl<'a> StepCtx<'a> {
    pub fn new(vsam: &'a Vsam) -> Self {
        StepCtx { vsam, dds: HashMap::new(), parm: String::new(), sysout: Vec::new(), echo: true }
    }
    pub fn dd(mut self, name: &str, dd: Dd) -> Self {
        self.dds.insert(name.to_string(), dd);
        self
    }
    pub fn with_parm(mut self, p: &str) -> Self {
        self.parm = p.to_string();
        self
    }
    /// DISPLAY
    pub fn display(&mut self, line: impl Into<String>) {
        let l = line.into();
        if self.echo {
            let _ = writeln!(std::io::stdout(), "{l}");
        }
        self.sysout.push(l);
    }
    pub fn vsam_name(&self, dd: &str) -> Result<String, Abend> {
        match self.dds.get(dd) {
            Some(Dd::Vsam(n)) => Ok(n.clone()),
            _ => Err(self.missing(dd)),
        }
    }
    pub fn file(&self, dd: &str) -> Result<(PathBuf, Format), Abend> {
        match self.dds.get(dd) {
            Some(Dd::File(p, f)) => Ok((p.clone(), *f)),
            _ => Err(self.missing(dd)),
        }
    }
    pub fn input(&self, dd: &str, rec_len: usize) -> Result<crate::seqfile::SeqIn, Abend> {
        match self.dds.get(dd) {
            Some(Dd::File(p, f)) => crate::seqfile::SeqIn::open(p, rec_len, *f).map_err(|_| self.open_error(dd)),
            Some(Dd::Instream(recs)) => Ok(crate::seqfile::SeqIn::from_records(
                recs.iter().map(|r| {
                    let mut r = r.clone();
                    r.resize(rec_len, b' ');
                    r
                }).collect(),
            )),
            Some(Dd::Vsam(ds)) => {
                // sequential read of a KSDS
                let recs = self.vsam.unload(ds).map_err(|_| self.open_error(dd))?;
                Ok(crate::seqfile::SeqIn::from_records(recs))
            }
            None => Err(self.missing(dd)),
        }
    }
    pub fn output(&self, dd: &str) -> Result<crate::seqfile::SeqOut, Abend> {
        let (p, f) = self.file(dd)?;
        crate::seqfile::SeqOut::create(&p, f).map_err(|_| self.open_error(dd))
    }
    fn missing(&self, dd: &str) -> Abend {
        eprintln!("IEC130I {dd} DD STATEMENT MISSING");
        Abend(999)
    }
    fn open_error(&self, dd: &str) -> Abend {
        eprintln!("IEC141I OPEN ERROR ON {dd}");
        Abend(999)
    }
}

/// `9910-DISPLAY-IO-STATUS` / `Z-DISPLAY-IO-STATUS` common paragraph.
pub fn io_status_line(status: &str) -> String {
    let b = status.as_bytes();
    let numeric = b.len() == 2 && b.iter().all(|c| c.is_ascii_digit());
    if !numeric || b[0] == b'9' {
        format!("FILE STATUS IS: NNNN{}{:03}", b[0] as char, b.get(1).copied().unwrap_or(0))
    } else {
        format!("FILE STATUS IS: NNNN00{status}")
    }
}

/// Standard abend paragraph (9999-ABEND-PROGRAM): DISPLAY then CEE3ABD 999.
pub fn abend(ctx: &mut StepCtx, msg: &str, status: &str) -> Abend {
    ctx.display(msg);
    ctx.display(io_status_line(status));
    ctx.display("ABENDING PROGRAM");
    Abend(999)
}

/// Resolve the data directory for sequential data sets and GDGs.
pub fn dataset_path(root: &Path, dsn: &str) -> PathBuf {
    root.join(dsn)
}
