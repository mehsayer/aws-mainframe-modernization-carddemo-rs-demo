//! VSAM KSDS emulation on SQLite.
//!
//! * Every KSDS is a table `(k BLOB PRIMARY KEY, r BLOB)`. Keys are stored translated
//!   to EBCDIC so that SQLite's memcmp ordering equals mainframe key ordering.
//!   Records are stored in ASCII (as the programs see them).
//! * Alternate indexes are tables `(ak, k)`, maintained on every write.
//! * Browses (STARTBR / READNEXT / READPREV) follow CICS rules: the first READPREV
//!   after STARTBR returns the record the browse was positioned on, and a STARTBR
//!   key of HIGH-VALUES positions after the last record.
//! * Each pseudo-conversational task or batch step runs in one SQLite transaction.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

mod catalog;
pub use catalog::{dataset as catalog_dataset, Aix, Dataset, CATALOG, CICS_FILES};

/// CICS-style response codes (EIBRESP values).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Resp {
    #[error("NOTFND")]
    NotFnd,
    #[error("DUPREC")]
    DupRec,
    #[error("DUPKEY")]
    DupKey,
    #[error("INVREQ")]
    InvReq,
    #[error("IOERR")]
    IoErr,
    #[error("NOTOPEN")]
    NotOpen,
    #[error("ENDFILE")]
    EndFile,
    #[error("FILENOTFOUND")]
    FileNotFound,
    #[error("LENGERR")]
    LengErr,
}

impl Resp {
    /// Numeric EIBRESP value.
    pub fn code(self) -> i32 {
        match self {
            Resp::NotFnd => 13,
            Resp::DupRec => 14,
            Resp::DupKey => 15,
            Resp::InvReq => 16,
            Resp::IoErr => 17,
            Resp::NotOpen => 19,
            Resp::EndFile => 20,
            Resp::LengErr => 22,
            Resp::FileNotFound => 12,
        }
    }
    /// EIBRESP2 value for the common conditions.
    pub fn code2(self) -> i32 {
        match self {
            Resp::NotFnd => 80,
            Resp::DupRec => 150,
            Resp::EndFile => 90,
            _ => 0,
        }
    }
    /// Equivalent batch FILE STATUS.
    pub fn file_status(self) -> &'static str {
        match self {
            Resp::NotFnd => "23",
            Resp::DupRec => "22",
            Resp::DupKey => "02",
            Resp::EndFile => "10",
            Resp::NotOpen => "47",
            _ => "30",
        }
    }
}

pub type VResult<T> = Result<T, Resp>;

fn ekey(k: &[u8]) -> Vec<u8> {
    k.iter().map(|&c| LATIN1_TO_EBCDIC[c as usize]).collect()
}

/// Access path resolved from a CICS file name or dataset name.
#[derive(Clone, Copy, Debug)]
pub struct PathRef {
    pub ds: &'static Dataset,
    pub aix: Option<&'static Aix>,
}

/// Position of an active browse.
#[derive(Clone, Debug)]
pub struct Browse {
    path: PathRef,
    /// (alternate key, primary key) of the current position (EBCDIC); for a base
    /// path the alternate key equals the primary key.
    pos: Option<(Vec<u8>, Vec<u8>)>,
    /// true until the first READNEXT/READPREV after STARTBR
    fresh: bool,
    /// positioned after end (STARTBR HIGH-VALUES / key beyond last)
    at_end: bool,
}

pub struct Vsam {
    conn: Connection,
}

impl Vsam {
    /// Open (and create if needed) the data store.
    pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let v = Vsam { conn };
        v.define_all()?;
        Ok(v)
    }

    /// How long a unit of work waits for another connection's write lock (default 30 s).
    pub fn set_busy_timeout(&self, d: std::time::Duration) -> rusqlite::Result<()> {
        self.conn.busy_timeout(d)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let v = Vsam { conn: Connection::open_in_memory()? };
        v.define_all()?;
        Ok(v)
    }

    fn define_all(&self) -> rusqlite::Result<()> {
        for ds in CATALOG {
            self.conn.execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS \"k_{n}\" (k BLOB PRIMARY KEY, r BLOB NOT NULL) WITHOUT ROWID;",
                n = ds.name
            ))?;
            for a in ds.aix {
                self.conn.execute_batch(&format!(
                    "CREATE TABLE IF NOT EXISTS \"a_{n}_{a}\" (ak BLOB, k BLOB, PRIMARY KEY(ak, k)) WITHOUT ROWID;",
                    n = ds.name,
                    a = a.name
                ))?;
            }
        }
        Ok(())
    }

    // ---- unit of work -------------------------------------------------------
    /// Start a unit of work (takes the store's write lock, waiting up to the busy timeout).
    /// Fails if the lock cannot be obtained or a unit of work is already open: the caller must not
    /// run updates then, because they would autocommit one by one and could not be backed out.
    pub fn begin(&self) -> VResult<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(|_| Resp::IoErr)
    }
    pub fn commit(&self) -> VResult<()> {
        self.conn.execute_batch("COMMIT").map_err(|_| Resp::IoErr)
    }
    pub fn rollback(&self) -> VResult<()> {
        self.conn.execute_batch("ROLLBACK").map_err(|_| Resp::IoErr)
    }
    /// True while a unit of work is open.
    pub fn in_unit_of_work(&self) -> bool {
        !self.conn.is_autocommit()
    }
    /// SYNCPOINT: commit and start a new unit of work.
    pub fn syncpoint(&self) -> VResult<()> {
        self.commit()?;
        self.begin()
    }
    pub fn syncpoint_rollback(&self) -> VResult<()> {
        self.rollback()?;
        self.begin()
    }

    // ---- path resolution ----------------------------------------------------
    /// Resolve a CICS file name (ACCTDAT, CXACAIX ...) or a dataset name.
    pub fn path(&self, name: &str) -> VResult<PathRef> {
        let name = name.trim();
        if let Some((_, ds, aix)) = CICS_FILES.iter().find(|(f, _, _)| *f == name) {
            let ds = catalog::dataset(ds).ok_or(Resp::FileNotFound)?;
            let aix = match aix {
                Some(a) => Some(ds.aix.iter().find(|x| x.name == *a).ok_or(Resp::FileNotFound)?),
                None => None,
            };
            return Ok(PathRef { ds, aix });
        }
        catalog::dataset(name).map(|ds| PathRef { ds, aix: None }).ok_or(Resp::FileNotFound)
    }

    fn fetch(&self, ds: &Dataset, ek: &[u8]) -> VResult<Option<Vec<u8>>> {
        self.conn
            .query_row(&format!("SELECT r FROM \"k_{}\" WHERE k = ?1", ds.name), params![ek], |r| r.get(0))
            .optional()
            .map_err(|_| Resp::IoErr)
    }

    // ---- keyed access -------------------------------------------------------
    /// READ by full key (EQUAL). Via a non-unique alternate path, returns the first
    /// record together with `dupkey = true` when more records share the key.
    pub fn read_ex(&self, file: &str, key: &[u8]) -> VResult<(Vec<u8>, bool)> {
        let p = self.path(file)?;
        match p.aix {
            None => {
                let ek = ekey(&key[..p.ds.key_len.min(key.len())]);
                self.fetch(p.ds, &ek)?.map(|r| (r, false)).ok_or(Resp::NotFnd)
            }
            Some(a) => {
                let eak = ekey(&key[..a.len.min(key.len())]);
                let mut st = self
                    .conn
                    .prepare(&format!("SELECT k FROM \"a_{}_{}\" WHERE ak = ?1 ORDER BY k LIMIT 2", p.ds.name, a.name))
                    .map_err(|_| Resp::IoErr)?;
                let ks: Vec<Vec<u8>> = st
                    .query_map(params![eak], |r| r.get(0))
                    .map_err(|_| Resp::IoErr)?
                    .filter_map(Result::ok)
                    .collect();
                let first = ks.first().ok_or(Resp::NotFnd)?;
                let rec = self.fetch(p.ds, first)?.ok_or(Resp::IoErr)?;
                Ok((rec, ks.len() > 1))
            }
        }
    }

    /// READ by key; DUPKEY on a non-unique path is reported as `Err(DupKey)` the way
    /// a CICS program that only tests RESP=NORMAL experiences it.
    pub fn read(&self, file: &str, key: &[u8]) -> VResult<Vec<u8>> {
        match self.read_ex(file, key)? {
            (_, true) => Err(Resp::DupKey),
            (r, false) => Ok(r),
        }
    }

    fn primary_key(ds: &Dataset, rec: &[u8]) -> Vec<u8> {
        rec[ds.key_off..ds.key_off + ds.key_len].to_vec()
    }

    fn index_record(&self, ds: &Dataset, ek: &[u8], rec: &[u8]) -> VResult<()> {
        for a in ds.aix {
            let eak = ekey(&rec[a.off..a.off + a.len]);
            if a.unique {
                let n: i64 = self
                    .conn
                    .query_row(&format!("SELECT count(*) FROM \"a_{}_{}\" WHERE ak = ?1", ds.name, a.name), params![eak], |r| r.get(0))
                    .map_err(|_| Resp::IoErr)?;
                if n > 0 {
                    return Err(Resp::DupRec);
                }
            }
            self.conn
                .execute(&format!("INSERT OR IGNORE INTO \"a_{}_{}\" (ak, k) VALUES (?1, ?2)", ds.name, a.name), params![eak, ek])
                .map_err(|_| Resp::IoErr)?;
        }
        Ok(())
    }

    fn unindex_record(&self, ds: &Dataset, ek: &[u8], rec: &[u8]) -> VResult<()> {
        for a in ds.aix {
            let eak = ekey(&rec[a.off..a.off + a.len]);
            self.conn
                .execute(&format!("DELETE FROM \"a_{}_{}\" WHERE ak = ?1 AND k = ?2", ds.name, a.name), params![eak, ek])
                .map_err(|_| Resp::IoErr)?;
        }
        Ok(())
    }

    /// Run one file operation all-or-nothing: base record and alternate-index rows change together.
    /// A savepoint works both inside a unit of work and in autocommit mode.
    fn atomic<T>(&self, op: impl FnOnce() -> VResult<T>) -> VResult<T> {
        self.conn.execute_batch("SAVEPOINT vsam_op").map_err(|_| Resp::IoErr)?;
        match op() {
            Ok(v) => {
                self.conn.execute_batch("RELEASE vsam_op").map_err(|_| Resp::IoErr)?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK TO vsam_op; RELEASE vsam_op");
                Err(e)
            }
        }
    }

    /// WRITE a new record (key taken from the record).
    pub fn write(&self, file: &str, rec: &[u8]) -> VResult<()> {
        self.atomic(|| self.write_raw(file, rec))
    }
    fn write_raw(&self, file: &str, rec: &[u8]) -> VResult<()> {
        let p = self.path(file)?;
        let ds = p.ds;
        let rec = fix_len(rec, ds.rec_len);
        let ek = ekey(&Self::primary_key(ds, &rec));
        if self.fetch(ds, &ek)?.is_some() {
            return Err(Resp::DupRec);
        }
        self.index_record(ds, &ek, &rec)?;
        self.conn
            .execute(&format!("INSERT INTO \"k_{}\" (k, r) VALUES (?1, ?2)", ds.name), params![ek, rec])
            .map_err(|_| Resp::IoErr)?;
        Ok(())
    }

    /// REWRITE an existing record (key taken from the record).
    pub fn rewrite(&self, file: &str, rec: &[u8]) -> VResult<()> {
        self.atomic(|| self.rewrite_raw(file, rec))
    }
    fn rewrite_raw(&self, file: &str, rec: &[u8]) -> VResult<()> {
        let p = self.path(file)?;
        let ds = p.ds;
        let rec = fix_len(rec, ds.rec_len);
        let ek = ekey(&Self::primary_key(ds, &rec));
        let old = self.fetch(ds, &ek)?.ok_or(Resp::NotFnd)?;
        self.unindex_record(ds, &ek, &old)?;
        self.index_record(ds, &ek, &rec)?;
        self.conn
            .execute(&format!("UPDATE \"k_{}\" SET r = ?2 WHERE k = ?1", ds.name), params![ek, rec])
            .map_err(|_| Resp::IoErr)?;
        Ok(())
    }

    /// DELETE by primary key.
    pub fn delete(&self, file: &str, key: &[u8]) -> VResult<()> {
        self.atomic(|| self.delete_raw(file, key))
    }
    fn delete_raw(&self, file: &str, key: &[u8]) -> VResult<()> {
        let p = self.path(file)?;
        let ds = p.ds;
        let ek = ekey(&key[..ds.key_len.min(key.len())]);
        let old = self.fetch(ds, &ek)?.ok_or(Resp::NotFnd)?;
        self.unindex_record(ds, &ek, &old)?;
        self.conn
            .execute(&format!("DELETE FROM \"k_{}\" WHERE k = ?1", ds.name), params![ek])
            .map_err(|_| Resp::IoErr)?;
        Ok(())
    }

    // ---- browsing -----------------------------------------------------------
    /// STARTBR (GTEQ when `gteq`, else EQUAL).
    pub fn start_browse(&self, file: &str, key: &[u8], gteq: bool) -> VResult<Browse> {
        let p = self.path(file)?;
        let klen = p.aix.map_or(p.ds.key_len, |a| a.len);
        let key = &key[..klen.min(key.len())];
        let mut br = Browse { path: p, pos: None, fresh: true, at_end: false };
        if !key.is_empty() && key.iter().all(|&c| c == 0xFF) {
            br.at_end = true;
            return Ok(br);
        }
        let ek = ekey(key);
        match self.seek(&p, &ek, None, true)? {
            Some((ak, k)) => {
                if !gteq && ak[..ek.len().min(ak.len())] != ek[..] {
                    return Err(Resp::NotFnd);
                }
                br.pos = Some((ak, k));
                Ok(br)
            }
            None => Err(Resp::NotFnd),
        }
    }

    /// First entry with (ak,k) >= (or >) the given position, or <= / < when !forward.
    fn seek(&self, p: &PathRef, ak: &[u8], k: Option<&[u8]>, inclusive_fwd: bool) -> VResult<Option<(Vec<u8>, Vec<u8>)>> {
        let (sql, has_k) = match (p.aix, k) {
            (None, _) => (
                format!("SELECT k, k FROM \"k_{}\" WHERE k {} ?1 ORDER BY k LIMIT 1", p.ds.name, if inclusive_fwd { ">=" } else { ">" }),
                false,
            ),
            (Some(a), None) => (
                format!("SELECT ak, k FROM \"a_{}_{}\" WHERE ak >= ?1 ORDER BY ak, k LIMIT 1", p.ds.name, a.name),
                false,
            ),
            (Some(a), Some(_)) => (
                format!(
                    "SELECT ak, k FROM \"a_{}_{}\" WHERE (ak > ?1) OR (ak = ?1 AND k {} ?2) ORDER BY ak, k LIMIT 1",
                    p.ds.name,
                    a.name,
                    if inclusive_fwd { ">=" } else { ">" }
                ),
                true,
            ),
        };
        let mut st = self.conn.prepare_cached(&sql).map_err(|_| Resp::IoErr)?;
        let row = if has_k {
            st.query_row(params![ak, k.unwrap()], |r| Ok((r.get(0)?, r.get(1)?))).optional()
        } else {
            st.query_row(params![ak], |r| Ok((r.get(0)?, r.get(1)?))).optional()
        };
        row.map_err(|_| Resp::IoErr)
    }

    fn seek_back(&self, p: &PathRef, pos: Option<(&[u8], &[u8])>, inclusive: bool) -> VResult<Option<(Vec<u8>, Vec<u8>)>> {
        let op = if inclusive { "<=" } else { "<" };
        let (sql, n) = match (p.aix, pos) {
            (None, None) => (format!("SELECT k, k FROM \"k_{}\" ORDER BY k DESC LIMIT 1", p.ds.name), 0),
            (None, Some(_)) => (format!("SELECT k, k FROM \"k_{}\" WHERE k {op} ?1 ORDER BY k DESC LIMIT 1", p.ds.name), 1),
            (Some(a), None) => (format!("SELECT ak, k FROM \"a_{}_{}\" ORDER BY ak DESC, k DESC LIMIT 1", p.ds.name, a.name), 0),
            (Some(a), Some(_)) => (
                format!(
                    "SELECT ak, k FROM \"a_{}_{}\" WHERE (ak < ?1) OR (ak = ?1 AND k {op} ?2) ORDER BY ak DESC, k DESC LIMIT 1",
                    p.ds.name, a.name
                ),
                2,
            ),
        };
        let mut st = self.conn.prepare_cached(&sql).map_err(|_| Resp::IoErr)?;
        let row = match (n, pos) {
            (0, _) => st.query_row([], |r| Ok((r.get(0)?, r.get(1)?))).optional(),
            (1, Some((ak, _))) => st.query_row(params![ak], |r| Ok((r.get(0)?, r.get(1)?))).optional(),
            (_, Some((ak, k))) => st.query_row(params![ak, k], |r| Ok((r.get(0)?, r.get(1)?))).optional(),
            _ => unreachable!(),
        };
        row.map_err(|_| Resp::IoErr)
    }

    /// READNEXT
    pub fn read_next(&self, br: &mut Browse) -> VResult<Vec<u8>> {
        if br.at_end {
            return Err(Resp::EndFile);
        }
        let next = match (&br.pos, br.fresh) {
            (Some(pos), true) => Some(pos.clone()),
            (Some((ak, k)), false) => self.seek(&br.path, ak, Some(k), false)?,
            (None, _) => None,
        };
        br.fresh = false;
        match next {
            Some(pos) => {
                let rec = self.fetch(br.path.ds, &pos.1)?.ok_or(Resp::IoErr)?;
                br.pos = Some(pos);
                Ok(rec)
            }
            None => {
                br.at_end = true;
                Err(Resp::EndFile)
            }
        }
    }

    /// READPREV
    pub fn read_prev(&self, br: &mut Browse) -> VResult<Vec<u8>> {
        let prev = if br.at_end {
            br.at_end = false;
            self.seek_back(&br.path, None, true)?
        } else {
            match &br.pos {
                Some((ak, k)) => self.seek_back(&br.path, Some((ak, k)), br.fresh)?,
                None => None,
            }
        };
        br.fresh = false;
        match prev {
            Some(pos) => {
                let rec = self.fetch(br.path.ds, &pos.1)?.ok_or(Resp::IoErr)?;
                br.pos = Some(pos);
                Ok(rec)
            }
            None => {
                br.pos = None;
                Err(Resp::EndFile)
            }
        }
    }

    // ---- bulk operations (IDCAMS REPRO / DELETE-DEFINE) -----------------------
    pub fn clear(&self, dataset: &str) -> VResult<()> {
        let ds = catalog::dataset(dataset).ok_or(Resp::FileNotFound)?;
        self.conn.execute(&format!("DELETE FROM \"k_{}\"", ds.name), []).map_err(|_| Resp::IoErr)?;
        for a in ds.aix {
            self.conn.execute(&format!("DELETE FROM \"a_{}_{}\"", ds.name, a.name), []).map_err(|_| Resp::IoErr)?;
        }
        Ok(())
    }

    /// All records in key order.
    pub fn unload(&self, dataset: &str) -> VResult<Vec<Vec<u8>>> {
        let ds = catalog::dataset(dataset).ok_or(Resp::FileNotFound)?;
        let mut st = self.conn.prepare(&format!("SELECT r FROM \"k_{}\" ORDER BY k", ds.name)).map_err(|_| Resp::IoErr)?;
        let rows = st.query_map([], |r| r.get(0)).map_err(|_| Resp::IoErr)?.filter_map(Result::ok).collect();
        Ok(rows)
    }

    pub fn count(&self, dataset: &str) -> VResult<usize> {
        let ds = catalog::dataset(dataset).ok_or(Resp::FileNotFound)?;
        self.conn
            .query_row(&format!("SELECT count(*) FROM \"k_{}\"", ds.name), [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(|_| Resp::IoErr)
    }
}

fn fix_len(rec: &[u8], len: usize) -> Vec<u8> {
    let mut v = rec.to_vec();
    v.resize(len, b' ');
    v
}

const LATIN1_TO_EBCDIC: [u8; 256] = {
    // Same table as cobol::ebcdic (duplicated to keep this crate dependency-free).
    let a2e: [u8; 256] = include!("a2e.in");
    a2e
};

#[cfg(test)]
mod tests;
