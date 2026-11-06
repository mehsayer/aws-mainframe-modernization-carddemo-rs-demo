//! Dataset catalog: the VSAM clusters defined by the CardDemo JCL
//! (ACCTFILE, CARDFILE, CUSTFILE, XREFFILE, TRANFILE/TRANIDX, TCATBALF, DISCGRP,
//! TRANTYPE, TRANCATG, DUSRSECJ, CREASTMT) and the CICS file definitions (CSD).

/// Alternate index (AIX + PATH).
#[derive(Debug)]
pub struct Aix {
    pub name: &'static str,
    pub off: usize,
    pub len: usize,
    pub unique: bool,
}

/// A KSDS cluster.
#[derive(Debug)]
pub struct Dataset {
    pub name: &'static str,
    pub rec_len: usize,
    pub key_off: usize,
    pub key_len: usize,
    pub aix: &'static [Aix],
}

pub const CATALOG: &[Dataset] = &[
    Dataset { name: "USRSEC", rec_len: 80, key_off: 0, key_len: 8, aix: &[] },
    Dataset { name: "ACCTDATA", rec_len: 300, key_off: 0, key_len: 11, aix: &[] },
    Dataset {
        name: "CARDDATA",
        rec_len: 150,
        key_off: 0,
        key_len: 16,
        aix: &[Aix { name: "ACCTID", off: 16, len: 11, unique: false }],
    },
    Dataset { name: "CUSTDATA", rec_len: 500, key_off: 0, key_len: 9, aix: &[] },
    Dataset {
        name: "CARDXREF",
        rec_len: 50,
        key_off: 0,
        key_len: 16,
        aix: &[Aix { name: "ACCTID", off: 25, len: 11, unique: false }],
    },
    Dataset {
        name: "TRANSACT",
        rec_len: 350,
        key_off: 0,
        key_len: 16,
        aix: &[Aix { name: "PROCTS", off: 304, len: 26, unique: false }],
    },
    Dataset { name: "TCATBALF", rec_len: 50, key_off: 0, key_len: 17, aix: &[] },
    Dataset { name: "DISCGRP", rec_len: 50, key_off: 0, key_len: 16, aix: &[] },
    Dataset { name: "TRANTYPE", rec_len: 60, key_off: 0, key_len: 2, aix: &[] },
    Dataset { name: "TRANCATG", rec_len: 60, key_off: 0, key_len: 6, aix: &[] },
    // CREASTMT work file: transactions re-keyed by card number + transaction id
    Dataset { name: "TRXFL", rec_len: 350, key_off: 0, key_len: 32, aix: &[] },
];

/// CICS FILE name -> (dataset, alternate index path)
pub const CICS_FILES: &[(&str, &str, Option<&str>)] = &[
    ("USRSEC", "USRSEC", None),
    ("ACCTDAT", "ACCTDATA", None),
    ("CARDDAT", "CARDDATA", None),
    ("CARDAIX", "CARDDATA", Some("ACCTID")),
    ("CUSTDAT", "CUSTDATA", None),
    ("CCXREF", "CARDXREF", None),
    ("CXACAIX", "CARDXREF", Some("ACCTID")),
    ("TRANSACT", "TRANSACT", None),
];

pub fn dataset(name: &str) -> Option<&'static Dataset> {
    CATALOG.iter().find(|d| d.name == name)
}
