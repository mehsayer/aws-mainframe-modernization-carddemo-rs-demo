//! The CardDemo online (CICS) programs, translated paragraph by paragraph.
pub mod common;
pub mod csutldtc;
pub mod signon;
pub mod transactions;
pub mod users;

use cics::{MapCatalog, Program, Region};
use std::collections::HashMap;

/// Program table (PPT) of the base application.
pub fn programs() -> HashMap<&'static str, Program> {
    let mut p: HashMap<&'static str, Program> = HashMap::new();
    p.insert("COSGN00C", signon::cosgn00c);
    p.insert("COMEN01C", signon::comen01c);
    p.insert("COADM01C", signon::coadm01c);
    p.insert("COUSR00C", users::cousr00c);
    p.insert("COTRN00C", transactions::cotrn00c);
    p.insert("COTRN01C", transactions::cotrn01c);
    p.insert("COTRN02C", transactions::cotrn02c);
    p.insert("COBIL00C", transactions::cobil00c);
    p.insert("CORPT00C", transactions::corpt00c);
    p.insert("COUSR01C", users::cousr01c);
    p.insert("COUSR02C", users::cousr02c);
    p.insert("COUSR03C", users::cousr03c);
    p
}

/// Transaction table (PCT).
pub fn transactions() -> HashMap<&'static str, &'static str> {
    [
        ("CC00", "COSGN00C"),
        ("CM00", "COMEN01C"),
        ("CA00", "COADM01C"),
        ("CAVW", "COACTVWC"),
        ("CAUP", "COACTUPC"),
        ("CCLI", "COCRDLIC"),
        ("CCDL", "COCRDSLC"),
        ("CCUP", "COCRDUPC"),
        ("CT00", "COTRN00C"),
        ("CT01", "COTRN01C"),
        ("CT02", "COTRN02C"),
        ("CR00", "CORPT00C"),
        ("CB00", "COBIL00C"),
        ("CU00", "COUSR00C"),
        ("CU01", "COUSR01C"),
        ("CU02", "COUSR02C"),
        ("CU03", "COUSR03C"),
    ]
    .into_iter()
    .collect()
}

/// A region with all base programs and the BMS maps compiled into the binary.
pub fn region(vsam: vsam::Vsam) -> Region {
    Region {
        vsam,
        maps: MapCatalog::builtin(),
        programs: programs(),
        transactions: transactions(),
        applid: "CARDDEMO".into(),
        sysid: "RUST".into(),
        on_job: None,
    }
}
