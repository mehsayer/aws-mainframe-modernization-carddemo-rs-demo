//! NFR-06: several terminals on one data store.
mod common;
use cics::{Aid, Region};
use common::*;
use std::path::{Path, PathBuf};

/// A region on a file-backed store (each region = one process/terminal server with its own connection).
fn file_region(db: &Path) -> Region {
    std::env::set_var("CARDDEMO_NOW", "2022-07-18T10:11:12.34");
    online::region(vsam::Vsam::open(db).unwrap())
}

fn fresh_store(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("carddemo-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("carddemo.db");
    let v = vsam::Vsam::open(&db).unwrap();
    let seed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cobol/app/data/EBCDIC");
    batch::repro::reset(&v, &seed).unwrap();
    db
}

fn open_card_update(t: &mut Term) -> String {
    t.typ("OPTION", "3").enter();
    let card = t.field("CRDNUM1");
    t.typ("CRDSEL1", "U").enter();
    assert_eq!(t.map(), "CCRDUPA");
    card
}

/// Two terminals fetch the same card for update. The second one saves first; when the first one
/// presses PF5, COCRDUPC's re-read FOR UPDATE finds the record changed and refuses to overwrite it.
#[test]
fn concurrent_update_of_the_same_card_is_detected() {
    let db = fresh_store("lock");
    let (ra, rb) = (file_region(&db), file_region(&db));
    let mut a = Term::signon(&ra, "USER0001", "PASSWORD");
    let mut b = Term::signon(&rb, "USER0001", "PASSWORD");
    let card = open_card_update(&mut a);
    assert_eq!(open_card_update(&mut b), card);

    a.typ("CRDSTCD", "N").enter();
    assert_eq!(a.field("INFOMSG"), "Changes validated.Press F5 to save");
    b.typ("CRDNAME", "SOMEONE ELSE").enter();
    assert_eq!(b.field("INFOMSG"), "Changes validated.Press F5 to save");
    b.key(Aid::Pf(5));
    assert_eq!(b.field("INFOMSG"), "Changes committed to database");

    a.key(Aid::Pf(5));
    assert_eq!(a.msg(), "Record changed by some one else. Please review");
    // A's change was not written; B's is in the store
    let rec = vsam::Vsam::open(&db).unwrap().read("CARDDAT", card.as_bytes()).unwrap();
    let c = cobol::copybooks::CardRecord::from_bytes(&rec);
    assert_eq!(c.card_active_status(), b"Y");
    assert!(c.card_embossed_name().starts_with(b"SOMEONE ELSE"));
}

/// Two terminal servers in parallel threads pay bills on one store. Each payment allocates the next
/// transaction id (browse to the highest key, +1) and writes it; tasks are serialised, so no id is
/// handed out twice and every successful payment is in TRANSACT.
#[test]
fn parallel_bill_payments_allocate_unique_transaction_ids() {
    let db = fresh_store("pay");
    let before = vsam::Vsam::open(&db).unwrap().count("TRANSACT").unwrap();
    let pay = |db: PathBuf, accts: std::ops::RangeInclusive<u32>| {
        std::thread::spawn(move || {
            let r = file_region(&db);
            let mut t = Term::signon(&r, "USER0001", "PASSWORD");
            t.typ("OPTION", "10").enter();
            assert_eq!(t.map(), "COBIL0A");
            let mut paid = 0;
            for n in accts {
                t.typ("ACTIDIN", &format!("{n:011}")).enter();
                match t.msg().as_str() {
                    "Confirm to make a bill payment..." => {
                        t.typ("CONFIRM", "Y").enter();
                        let m = t.msg();
                        assert!(m.starts_with("Payment successful.  Your Transaction ID is "), "account {n}: {m}");
                        paid += 1;
                    }
                    "You have nothing to pay..." => {}
                    m => panic!("account {n}: {m}"),
                }
            }
            paid
        })
    };
    let ta = pay(db.clone(), 1..=25);
    let tb = pay(db.clone(), 26..=50);
    let paid = ta.join().unwrap() + tb.join().unwrap();
    assert!(paid > 20, "only {paid} payments");
    let after = vsam::Vsam::open(&db).unwrap().count("TRANSACT").unwrap();
    assert_eq!(after - before, paid);
}
