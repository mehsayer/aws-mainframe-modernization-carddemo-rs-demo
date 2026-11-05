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
    for job in ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN", "TRANREPT", "CREASTMT"] {
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
}
