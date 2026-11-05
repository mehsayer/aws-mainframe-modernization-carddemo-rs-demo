use batch::seqfile::Format;
use batch::{Dd, StepCtx};
use std::path::PathBuf;

fn seed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../app/data/EBCDIC")
}

#[test]
fn reset_and_post_daily_transactions() {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    let v = vsam::Vsam::open_in_memory().unwrap();
    let loaded = batch::repro::reset(&v, &seed()).unwrap();
    println!("{loaded:?}");
    assert!(loaded.iter().any(|(d, n)| d == "USRSEC" && *n == 10));
    let tmp = std::env::temp_dir().join("carddemo-test-posting");
    let mut ctx = StepCtx::new(&v)
        .dd("DALYTRAN", Dd::File(seed().join("AWS.M2.CARDDEMO.DALYTRAN.PS"), Format::Ebcdic))
        .dd("TRANFILE", Dd::Vsam("TRANSACT".into()))
        .dd("XREFFILE", Dd::Vsam("CARDXREF".into()))
        .dd("DALYREJS", Dd::File(tmp.join("DALYREJS"), Format::Lines))
        .dd("ACCTFILE", Dd::Vsam("ACCTDATA".into()))
        .dd("TCATBALF", Dd::Vsam("TCATBALF".into()));
    ctx.echo = false;
    let rc = batch::cbtrn02c::run(&mut ctx).unwrap();
    let tail: Vec<_> = ctx.sysout.iter().rev().take(3).collect();
    println!("rc={rc} {tail:?}");
    assert!(ctx.sysout.iter().any(|l| l == "TRANSACTIONS PROCESSED :000000300"));
}
