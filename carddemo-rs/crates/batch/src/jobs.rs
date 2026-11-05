//! The CardDemo JCL job streams, re-expressed as step lists.
//!
//! Data set naming follows the JCL (`AWS.M2.CARDDEMO.*`). VSAM clusters live in the
//! SQLite store; sequential data sets and GDG generations are files under
//! `<root>/datasets/`. A GDG base `X` keeps its generations as `X.G0001V00`, ...

use crate::ctx::{Abend, Dd, StepCtx};
use crate::repro;
use crate::seqfile::{read_all, Format, SeqOut};
use std::path::{Path, PathBuf};
use vsam::Vsam;

pub const HLQ: &str = "AWS.M2.CARDDEMO";

pub struct Env<'a> {
    pub vsam: &'a Vsam,
    pub root: PathBuf,
    pub seed: PathBuf,
    pub echo: bool,
}

/// Outcome of one job step.
#[derive(Debug, Clone)]
pub struct StepLog {
    pub step: String,
    pub program: String,
    pub rc: Result<i32, Abend>,
    pub sysout: Vec<String>,
}

impl Env<'_> {
    pub fn ds(&self, name: &str) -> PathBuf {
        self.root.join("datasets").join(format!("{HLQ}.{name}"))
    }

    /// Path of a GDG generation relative to the current one (0 = current, 1 = new).
    pub fn gdg(&self, base: &str, rel: i32) -> PathBuf {
        let dir = self.root.join("datasets");
        let prefix = format!("{HLQ}.{base}.G");
        let mut cur = 0;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(rest) = n.strip_prefix(&prefix) {
                    if let Ok(g) = rest[..4].parse::<i32>() {
                        cur = cur.max(g);
                    }
                }
            }
        }
        dir.join(format!("{prefix}{:04}V00", (cur + rel).max(0)))
    }

    fn step(&self, logs: &mut Vec<StepLog>, step: &str, program: &str, f: impl FnOnce(&mut StepCtx) -> Result<i32, Abend>, dds: Vec<(&str, Dd)>, parm: &str) -> Result<i32, Abend> {
        let mut ctx = StepCtx::new(self.vsam).with_parm(parm);
        ctx.echo = self.echo;
        for (n, d) in dds {
            ctx = ctx.dd(n, d);
        }
        self.vsam.begin();
        let rc = f(&mut ctx);
        match &rc {
            Ok(_) => self.vsam.commit(),
            // an abending batch step leaves VSAM updates made so far in place
            // (no rollback on the mainframe either), so commit them too
            Err(_) => self.vsam.commit(),
        }
        logs.push(StepLog { step: step.into(), program: program.into(), rc: rc.clone(), sysout: ctx.sysout });
        rc
    }

    fn util(&self, logs: &mut Vec<StepLog>, step: &str, program: &str, f: impl FnOnce() -> Result<String, String>) -> Result<i32, Abend> {
        self.vsam.begin();
        let r = f();
        self.vsam.commit();
        let (rc, msg) = match r {
            Ok(m) => (Ok(0), m),
            Err(m) => (Ok(12), m),
        };
        if self.echo {
            println!("{step} {program}: {msg}");
        }
        logs.push(StepLog { step: step.into(), program: program.into(), rc: rc.clone(), sysout: vec![msg] });
        match rc {
            Ok(12) => Err(Abend(12)),
            r => r,
        }
    }
}

/// SORT FIELDS=(...) on byte ranges (CH, ascending, EBCDIC collating sequence).
pub fn sort_records(recs: &mut [Vec<u8>], keys: &[(usize, usize)]) {
    let ek = |r: &Vec<u8>| -> Vec<u8> {
        let mut k = Vec::new();
        for &(off, len) in keys {
            let mut part = r[off..off + len].to_vec();
            cobol::ebcdic::to_ebcdic(&mut part);
            k.extend(part);
        }
        k
    };
    recs.sort_by_cached_key(ek);
}

fn write_file(path: &Path, recs: &[Vec<u8>], fmt: Format) -> Result<(), String> {
    let mut o = SeqOut::create(path, fmt).map_err(|e| e.to_string())?;
    for r in recs {
        o.write(r).map_err(|e| e.to_string())?;
    }
    o.close().map_err(|e| e.to_string())
}

/// Names of all jobs the runner knows, with their purpose.
pub const JOBS: &[(&str, &str)] = &[
    ("CLOSEFIL", "Close VSAM files in CICS (no-op)"),
    ("ACCTFILE", "Refresh account master from sample data"),
    ("CARDFILE", "Refresh card master from sample data"),
    ("CUSTFILE", "Refresh customer master from sample data"),
    ("XREFFILE", "Refresh card/account/customer cross reference"),
    ("TRANFILE", "Load initial transaction master"),
    ("DISCGRP", "Load disclosure groups"),
    ("TCATBALF", "Load transaction category balances"),
    ("TRANCATG", "Load transaction categories"),
    ("TRANTYPE", "Load transaction types"),
    ("DUSRSECJ", "Load user security file"),
    ("TRANBKP", "Back up and re-define transaction master"),
    ("POSTTRAN", "Post daily transactions (CBTRN02C)"),
    ("INTCALC", "Interest calculation (CBACT04C)"),
    ("COMBTRAN", "Combine system and posted transactions"),
    ("TRANREPT", "Transaction report (CBTRN03C)"),
    ("CREASTMT", "Customer statements, text and HTML (CBSTM03A/B)"),
    ("TRANIDX", "Define AIX on transaction master (automatic)"),
    ("OPENFIL", "Open VSAM files in CICS (no-op)"),
];

/// Run a job. `opts` carries job parameters (INTCALC run date, TRANREPT dates).
pub fn run_job(env: &Env, job: &str, opts: &JobOpts) -> Result<Vec<StepLog>, (Vec<StepLog>, Abend)> {
    let mut logs = Vec::new();
    let r = run_job_inner(env, job, opts, &mut logs);
    match r {
        Ok(()) => Ok(logs),
        Err(a) => Err((logs, a)),
    }
}

#[derive(Clone, Debug)]
pub struct JobOpts {
    pub intcalc_parm: String,
    pub report_start: String,
    pub report_end: String,
    pub dalytran: Option<PathBuf>,
}

impl Default for JobOpts {
    fn default() -> Self {
        JobOpts {
            intcalc_parm: "2022071800".into(),
            report_start: "2022-01-01".into(),
            report_end: "2022-07-06".into(),
            dalytran: None,
        }
    }
}

fn reload(env: &Env, logs: &mut Vec<StepLog>, dataset: &str, seed_file: &str) -> Result<i32, Abend> {
    env.util(logs, "STEP05", "IDCAMS", || {
        env.vsam.clear(dataset).map_err(|e| e.to_string())?;
        let n = repro::repro_in(env.vsam, dataset, &env.seed.join(seed_file), Format::Ebcdic)?;
        Ok(format!("DELETE/DEFINE {dataset}; REPRO {n} RECORDS"))
    })
}

fn run_job_inner(env: &Env, job: &str, o: &JobOpts, logs: &mut Vec<StepLog>) -> Result<(), Abend> {
    let v = |n: &str| Dd::Vsam(n.to_string());
    match job {
        "CLOSEFIL" | "OPENFIL" | "TRANIDX" => {
            env.util(logs, "STEP01", "IEFBR14", || Ok(format!("{job}: nothing to do in the Rust runtime")))?;
        }
        "ACCTFILE" => drop(reload(env, logs, "ACCTDATA", "AWS.M2.CARDDEMO.ACCTDATA.PS")?),
        "CARDFILE" => drop(reload(env, logs, "CARDDATA", "AWS.M2.CARDDEMO.CARDDATA.PS")?),
        "CUSTFILE" => drop(reload(env, logs, "CUSTDATA", "AWS.M2.CARDDEMO.CUSTDATA.PS")?),
        "XREFFILE" => drop(reload(env, logs, "CARDXREF", "AWS.M2.CARDDEMO.CARDXREF.PS")?),
        "TRANFILE" => drop(reload(env, logs, "TRANSACT", "AWS.M2.CARDDEMO.DALYTRAN.PS.INIT")?),
        "DISCGRP" => drop(reload(env, logs, "DISCGRP", "AWS.M2.CARDDEMO.DISCGRP.PS")?),
        "TCATBALF" => drop(reload(env, logs, "TCATBALF", "AWS.M2.CARDDEMO.TCATBALF.PS")?),
        "TRANCATG" => drop(reload(env, logs, "TRANCATG", "AWS.M2.CARDDEMO.TRANCATG.PS")?),
        "TRANTYPE" => drop(reload(env, logs, "TRANTYPE", "AWS.M2.CARDDEMO.TRANTYPE.PS")?),
        "DUSRSECJ" => drop(reload(env, logs, "USRSEC", "AWS.M2.CARDDEMO.USRSEC.PS")?),
        "TRANBKP" => {
            let bkup = env.gdg("TRANSACT.BKUP", 1);
            env.util(logs, "STEP05R", "IDCAMS", || {
                let n = repro::repro_out(env.vsam, "TRANSACT", &bkup, Format::Fixed)?;
                Ok(format!("REPRO TRANSACT -> {} ({n} RECORDS)", bkup.display()))
            })?;
            env.util(logs, "STEP05", "IDCAMS", || {
                env.vsam.clear("TRANSACT").map_err(|e| e.to_string())?;
                Ok("DELETE/DEFINE TRANSACT CLUSTER AND AIX".into())
            })?;
        }
        "POSTTRAN" => {
            let dalytran = o.dalytran.clone().unwrap_or_else(|| env.seed.join("AWS.M2.CARDDEMO.DALYTRAN.PS"));
            let fmt = Format::guess(&dalytran);
            let rejs = env.gdg("DALYREJS", 1);
            let rc = env.step(
                logs,
                "STEP15",
                "CBTRN02C",
                crate::cbtrn02c::run,
                vec![
                    ("TRANFILE", v("TRANSACT")),
                    ("DALYTRAN", Dd::File(dalytran, fmt)),
                    ("XREFFILE", v("CARDXREF")),
                    ("DALYREJS", Dd::File(rejs, Format::Fixed)),
                    ("ACCTFILE", v("ACCTDATA")),
                    ("TCATBALF", v("TCATBALF")),
                ],
                "",
            )?;
            let _ = rc;
        }
        "INTCALC" => {
            let systran = env.gdg("SYSTRAN", 1);
            env.step(
                logs,
                "STEP15",
                "CBACT04C",
                crate::cbact04c::run,
                vec![
                    ("TCATBALF", v("TCATBALF")),
                    ("XREFFILE", v("CARDXREF")),
                    ("ACCTFILE", v("ACCTDATA")),
                    ("DISCGRP", v("DISCGRP")),
                    ("TRANSACT", Dd::File(systran, Format::Fixed)),
                ],
                &o.intcalc_parm,
            )?;
        }
        "COMBTRAN" => {
            let bk = env.gdg("TRANSACT.BKUP", 0);
            let sys = env.gdg("SYSTRAN", 0);
            let comb = env.gdg("TRANSACT.COMBINED", 1);
            env.util(logs, "STEP05R", "SORT", || {
                let mut recs = read_all(&bk, 350, Format::Fixed).map_err(|e| format!("{}: {e}", bk.display()))?;
                recs.extend(read_all(&sys, 350, Format::Fixed).map_err(|e| format!("{}: {e}", sys.display()))?);
                sort_records(&mut recs, &[(0, 16)]);
                write_file(&comb, &recs, Format::Fixed)?;
                Ok(format!("SORT FIELDS=(TRAN-ID,A): {} RECORDS", recs.len()))
            })?;
            env.util(logs, "STEP10", "IDCAMS", || {
                let n = repro::repro_in(env.vsam, "TRANSACT", &comb, Format::Fixed)?;
                Ok(format!("REPRO INFILE(TRANSACT) OUTFILE(TRANVSAM): {n} RECORDS"))
            })?;
        }
        "TRANREPT" => {
            let bkup = env.gdg("TRANSACT.BKUP", 1);
            env.util(logs, "STEP05R", "IDCAMS", || {
                let n = repro::repro_out(env.vsam, "TRANSACT", &bkup, Format::Fixed)?;
                Ok(format!("REPRO TRANSACT -> BKUP ({n} RECORDS)"))
            })?;
            let daly = env.gdg("TRANSACT.DALY", 1);
            let (s, e) = (o.report_start.clone(), o.report_end.clone());
            env.util(logs, "STEP05R", "SORT", || {
                let recs = read_all(&bkup, 350, Format::Fixed).map_err(|e| e.to_string())?;
                // INCLUDE COND=(TRAN-PROC-DT,GE,PARM-START-DATE,AND,TRAN-PROC-DT,LE,PARM-END-DATE)
                let mut recs: Vec<_> = recs
                    .into_iter()
                    .filter(|r| &r[304..314] >= s.as_bytes() && &r[304..314] <= e.as_bytes())
                    .collect();
                // TRAN-CARD-NUM,263,16,ZD
                sort_records(&mut recs, &[(262, 16)]);
                write_file(&daly, &recs, Format::Fixed)?;
                Ok(format!("SORT FIELDS=(TRAN-CARD-NUM,A) INCLUDE {s}..{e}: {} RECORDS", recs.len()))
            })?;
            let mut dateparm = format!("{} {}", o.report_start, o.report_end).into_bytes();
            dateparm.resize(80, b' ');
            let rept = env.gdg("TRANREPT", 1);
            env.step(
                logs,
                "STEP10R",
                "CBTRN03C",
                crate::cbtrn03c::run,
                vec![
                    ("TRANFILE", Dd::File(daly, Format::Fixed)),
                    ("CARDXREF", v("CARDXREF")),
                    ("TRANTYPE", v("TRANTYPE")),
                    ("TRANCATG", v("TRANCATG")),
                    ("DATEPARM", Dd::Instream(vec![dateparm])),
                    ("TRANREPT", Dd::File(rept, Format::Lines)),
                ],
                "",
            )?;
        }
        "CREASTMT" => {
            let seq = env.ds("TRXFL.SEQ");
            env.util(logs, "STEP010", "SORT", || {
                let recs = env.vsam.unload("TRANSACT").map_err(|e| e.to_string())?;
                let mut recs = recs;
                // SORT FIELDS=(263,16,CH,A,1,16,CH,A)
                sort_records(&mut recs, &[(262, 16), (0, 16)]);
                // OUTREC FIELDS=(1:263,16,17:1,262,279:279,50)
                let out: Vec<Vec<u8>> = recs
                    .iter()
                    .map(|r| {
                        let mut o = vec![b' '; 350];
                        o[0..16].copy_from_slice(&r[262..278]);
                        o[16..278].copy_from_slice(&r[0..262]);
                        o[278..328].copy_from_slice(&r[278..328]);
                        o
                    })
                    .collect();
                write_file(&seq, &out, Format::Fixed)?;
                Ok(format!("SORT/OUTREC TRANSACT -> TRXFL.SEQ: {} RECORDS", out.len()))
            })?;
            env.util(logs, "STEP020", "IDCAMS", || {
                env.vsam.clear("TRXFL").map_err(|e| e.to_string())?;
                let n = repro::repro_in(env.vsam, "TRXFL", &seq, Format::Fixed)?;
                Ok(format!("REPRO TRXFL.SEQ -> TRXFL.VSAM.KSDS: {n} RECORDS"))
            })?;
            env.step(
                logs,
                "STEP040",
                "CBSTM03A",
                crate::cbstm03a::run,
                vec![
                    ("TRNXFILE", v("TRXFL")),
                    ("XREFFILE", v("CARDXREF")),
                    ("ACCTFILE", v("ACCTDATA")),
                    ("CUSTFILE", v("CUSTDATA")),
                    ("STMTFILE", Dd::File(env.ds("STATEMNT.PS"), Format::Lines)),
                    ("HTMLFILE", Dd::File(env.ds("STATEMNT.HTML"), Format::Lines)),
                ],
                "",
            )?;
        }
        other => {
            eprintln!("JCL ERROR: unknown job {other}");
            return Err(Abend(9));
        }
    }
    Ok(())
}
