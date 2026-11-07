//! IDCAMS REPRO / DELETE / DEFINE replacements and the initial data load
//! (jobs ACCTFILE, CARDFILE, CUSTFILE, XREFFILE, TRANFILE, DISCGRP, TCATBALF,
//! TRANCATG, TRANTYPE, DUSRSECJ).

use crate::seqfile::{read_all, Format, SeqOut};
use std::path::{Path, PathBuf};
use vsam::{Resp, Vsam};

/// The shipped EBCDIC sample data in the repository (`cobol/app/data/EBCDIC`), for tests and tools run
/// from the source tree.
pub fn sample_seed_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cobol/app/data/EBCDIC")
}

/// An in-memory store loaded with the shipped sample data ([`sample_seed_dir`]).
pub fn sample_store() -> Result<Vsam, String> {
    let v = Vsam::open_in_memory().map_err(|e| format!("in-memory store: {e:?}"))?;
    reset(&v, &sample_seed_dir())?;
    Ok(v)
}

/// REPRO a sequential file into a KSDS (records sorted by key as VSAM requires;
/// duplicates rejected like REPRO does). Returns records loaded.
pub fn repro_in(vsam: &Vsam, dataset: &str, file: &Path, fmt: Format) -> Result<usize, String> {
    let ds = vsam::catalog_dataset(dataset).ok_or_else(|| format!("unknown dataset {dataset}"))?;
    let recs = read_all(file, ds.rec_len, fmt).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut n = 0;
    for r in recs {
        if r.iter().all(|&c| c == b' ' || c == 0) {
            continue;
        }
        match vsam.write(dataset, &r) {
            Ok(()) => n += 1,
            Err(Resp::DupRec) => eprintln!("IDC3302I DUPLICATE RECORD IN {dataset}: {}", String::from_utf8_lossy(&r[..ds.key_len])),
            Err(e) => return Err(format!("{dataset}: {e}")),
        }
    }
    Ok(n)
}

/// REPRO a KSDS out to a sequential file (key order).
pub fn repro_out(vsam: &Vsam, dataset: &str, file: &Path, fmt: Format) -> Result<usize, String> {
    let recs = vsam.unload(dataset).map_err(|e| e.to_string())?;
    let mut out = SeqOut::create(file, fmt).map_err(|e| e.to_string())?;
    for r in &recs {
        out.write(r).map_err(|e| e.to_string())?;
    }
    out.close().map_err(|e| e.to_string())?;
    Ok(recs.len())
}

/// Seed files shipped with the application (`cobol/app/data/EBCDIC`).
pub const SEED: &[(&str, &str)] = &[
    ("USRSEC", "AWS.M2.CARDDEMO.USRSEC.PS"),
    ("ACCTDATA", "AWS.M2.CARDDEMO.ACCTDATA.PS"),
    ("CARDDATA", "AWS.M2.CARDDEMO.CARDDATA.PS"),
    ("CUSTDATA", "AWS.M2.CARDDEMO.CUSTDATA.PS"),
    ("CARDXREF", "AWS.M2.CARDDEMO.CARDXREF.PS"),
    ("TRANSACT", "AWS.M2.CARDDEMO.DALYTRAN.PS.INIT"),
    ("DISCGRP", "AWS.M2.CARDDEMO.DISCGRP.PS"),
    ("TCATBALF", "AWS.M2.CARDDEMO.TCATBALF.PS"),
    ("TRANCATG", "AWS.M2.CARDDEMO.TRANCATG.PS"),
    ("TRANTYPE", "AWS.M2.CARDDEMO.TRANTYPE.PS"),
];

/// Reset the store to the shipped sample data (DELETE/DEFINE + REPRO of every cluster).
pub fn reset(vsam: &Vsam, seed_dir: &Path) -> Result<Vec<(String, usize)>, String> {
    let mut out = Vec::new();
    vsam.begin().map_err(|_| "the data store is busy (another job or terminal holds it)".to_string())?;
    let load = |out: &mut Vec<(String, usize)>| -> Result<(), String> {
        for ds in vsam::CATALOG {
            vsam.clear(ds.name).map_err(|e| e.to_string())?;
        }
        for (ds, f) in SEED {
            let n = repro_in(vsam, ds, &seed_dir.join(f), Format::Ebcdic)?;
            out.push((ds.to_string(), n));
        }
        Ok(())
    };
    if let Err(e) = load(&mut out) {
        let _ = vsam.rollback();
        return Err(e);
    }
    vsam.commit().map_err(|_| "reset could not be committed".to_string())?;
    Ok(out)
}
