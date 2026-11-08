//! Differential fuzzing of the online programs: random keystroke sessions. Each session signs on (as a user
//! or an administrator) and then, step after step, types values from a vocabulary of interesting inputs into
//! random input fields of whatever screen is shown and presses a random key. The same seed gives the same
//! sessions, so `difftest/online.py --only fuzz_sessions` (driven by `difftest/fuzz.py online`) can run them on
//! the Rust programs and on the original COBOL programs and compare every screen.
//!
//! `CARDDEMO_FUZZ_SEED` (default 1), `CARDDEMO_FUZZ_SESSIONS` (default 4), `CARDDEMO_FUZZ_STEPS` (default 25).
//! Numbers typed into numeric fields are valid digits or obviously invalid text: invalid data moved into a
//! numeric item is compiler-dependent, so such differences are expected (and known to online.py).
mod common;
use cics::Aid;
use common::*;

/// xorshift64*: small, deterministic, the same on every platform.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Values worth typing: valid keys of the sample data, codes the screens accept, and edge cases. No lone "0":
/// in a search field it becomes 0 plus spaces, and the programs test that numeric item against ZEROS before
/// any IS NUMERIC check, which is compiler-dependent (GnuCOBOL reads the spaces as zeros).
const VALUES: &[&str] = &[
    "", " ", "*", "S", "U", "D", "Y", "N", "X", "1", "2", "3", "5", "6", "7", "8", "9", "10", "11", "12",
    "00000000001", "00000000050", "00000000017", "99999999999", "0500024453765740", "9999999999999999",
    "USER0001", "ADMIN001", "USER0099", "PASSWORD", "U", "A", "JOHN DOE", "Ada", "abc", "12a", "-5", "2022",
    "13", "31", "01", "100.00", "-25.50", "999999999.99",
];

/// The menus' OPTION field goes into a numeric item without a numeric check first (COMEN01C, COADM01C):
/// anything but digits and blanks there is compiler-dependent (GnuCOBOL turns "-5" into 5, IBM COBOL rejects
/// it), so only these values are typed into it.
const OPTION_VALUES: &[&str] = &["", " ", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "99"];

/// COBIL00C moves its account field (X(11)) into a PIC 9(11) key with no numeric check first: anything but a
/// full-length number there is compiler-dependent (GnuCOBOL reads "6          " as account 6), so only these.
const PAYMENT_ACCOUNT_VALUES: &[&str] = &["", " ", "00000000001", "00000000050", "00000000017", "99999999999"];

/// Keys, Enter more often than the others.
const KEYS: &[Aid] = &[
    Aid::Enter, Aid::Enter, Aid::Enter, Aid::Enter, Aid::Enter, Aid::Pf(3), Aid::Pf(4), Aid::Pf(5), Aid::Pf(7),
    Aid::Pf(8), Aid::Pf(12), Aid::Clear,
];

#[test]
fn fuzz() {
    let seed = env("CARDDEMO_FUZZ_SEED", 1);
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    for _ in 0..env("CARDDEMO_FUZZ_SESSIONS", 4) {
        let r = region();
        let (user, _) = *rng.pick(&[("USER0001", ()), ("ADMIN001", ())]);
        let mut t = Term::signon(&r, user, "PASSWORD");
        for _ in 0..env("CARDDEMO_FUZZ_STEPS", 25) {
            if t.s.screen.text.is_some() {
                // a text screen (signed off, an abend): Enter starts the sign-on again
                t.enter();
                t = Term::signon(&r, user, "PASSWORD");
                continue;
            }
            let inputs: Vec<String> = t
                .s
                .screen
                .fields
                .iter()
                .filter(|f| !f.attr.protected && f.len > 0)
                .filter_map(|f| f.name.clone())
                .collect();
            for _ in 0..rng.below(4) {
                if inputs.is_empty() {
                    break;
                }
                let field = rng.pick(&inputs).clone();
                let value = match (field.as_str(), t.map()) {
                    ("OPTION", _) => *rng.pick(OPTION_VALUES),
                    ("ACTIDIN", "COBIL0A") => *rng.pick(PAYMENT_ACCOUNT_VALUES),
                    _ => *rng.pick(VALUES),
                };
                log_note(&format!("type {field} = {value:?}"));
                t.typ(&field, value);
            }
            let key = *rng.pick(KEYS);
            log_note(&format!("key {key:?}"));
            t.key(key);
        }
    }
}
