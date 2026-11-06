use batch::jobs::{run_job, Env, JobOpts};
use std::path::PathBuf;

#[test]
fn full_batch_cycle() {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    let root = std::env::temp_dir().join("carddemo-test-jobs");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let v = vsam::Vsam::open(root.join("carddemo.db")).unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../app/data/EBCDIC");
    batch::repro::reset(&v, &seed).unwrap();
    let env = Env { vsam: &v, root: root.clone(), seed, echo: false };
    let o = JobOpts { report_end: "2022-07-31".into(), ..JobOpts::default() };
    for job in ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN", "TRANREPT", "CREASTMT", "READACCT", "READCARD", "READXREF", "READCUST", "DALYVRFY", "CBEXPORT", "CBIMPORT"] {
        let logs = run_job(&env, job, &o).unwrap_or_else(|(l, a)| panic!("{job} {a}: {:?}", l.last().map(|x| &x.sysout)));
        for l in &logs {
            println!("{job:9} {:8} {:8} {:?} {}", l.step, l.program, l.rc, l.sysout.last().cloned().unwrap_or_default());
        }
    }
    let rept = env.gdg("TRANREPT", 0);
    let text = std::fs::read_to_string(&rept).unwrap();
    println!("{}", text.lines().take(8).collect::<Vec<_>>().join("\n"));
    assert!(text.contains("Grand Total"));
    let stmt = std::fs::read_to_string(env.ds("STATEMNT.PS")).unwrap();
    println!("{}", stmt.lines().take(24).collect::<Vec<_>>().join("\n"));
    assert!(stmt.contains("END OF STATEMENT"));
    // export -> import round trip reproduces every named field of the master files; FILLER bytes
    // come back as X'00' (CBIMPORT INITIALIZEs its FD records, which leaves FILLER alone)
    use cobol::copybooks::*;
    fn filler_mask(init: impl Fn(&mut Vec<u8>)) -> Vec<bool> {
        let mut r = Vec::new();
        init(&mut r);
        r.iter().map(|&c| c == 0xFF).collect()
    }
    macro_rules! mask {
        ($t:ty) => {
            filler_mask(|r| {
                let mut x = <$t>::from_bytes(&[0xFFu8; <$t>::LEN]);
                x.initialize();
                *r = x.buf.to_vec();
            })
        };
    }
    for (imp, ds, mask) in [
        ("CUSTDATA.IMPORT", "CUSTDATA", mask!(CustomerRecord)),
        ("ACCTDATA.IMPORT", "ACCTDATA", mask!(AccountRecord)),
        ("CARDXREF.IMPORT", "CARDXREF", mask!(CardXrefRecord)),
        ("TRANSACT.IMPORT", "TRANSACT", mask!(TranRecord)),
        ("CARDDATA.IMPORT", "CARDDATA", mask!(CardRecord)),
    ] {
        let a = std::fs::read(env.ds(imp)).unwrap();
        let b = v.unload(ds).unwrap();
        assert_eq!(a.len(), b.len() * mask.len(), "record count for {ds}");
        for (i, (ra, rb)) in a.chunks(mask.len()).zip(&b).enumerate() {
            for (j, m) in mask.iter().enumerate() {
                if *m {
                    assert_eq!(ra[j], 0, "{ds} record {i}: FILLER byte {j} should be X'00'");
                } else {
                    assert_eq!(ra[j], rb[j], "round trip mismatch for {ds} record {i} byte {j}");
                }
            }
        }
    }
}
