//! COBOL numeric data items: DISPLAY (zoned), COMP-3 (packed) and BINARY.
//!
//! Semantics follow IBM Enterprise COBOL with TRUNC(STD):
//! * a MOVE/COMPUTE into a numeric item truncates high-order integer digits and
//!   low-order fraction digits (no rounding);
//! * an unsigned receiver stores the absolute value;
//! * signed DISPLAY items carry the sign as an overpunch in the last byte, in the
//!   ASCII rendering of the mainframe convention: `{ABCDEFGHI` = +0..+9 and
//!   `}JKLMNOPQR` = -0..-9 (EBCDIC X'C0'-X'C9' / X'D0'-X'D9' translated with cp037).

pub use rust_decimal::Decimal as Dec;
use rust_decimal::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Usage {
    Display,
    Comp3,
    Binary,
}

/// Picture of a numeric item: total digits, digits after the implied decimal point, sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pic {
    pub digits: u32,
    pub scale: u32,
    pub signed: bool,
    pub usage: Usage,
}

impl Pic {
    pub const fn display(digits: u32, scale: u32, signed: bool) -> Self {
        Pic { digits, scale, signed, usage: Usage::Display }
    }
    pub const fn comp3(digits: u32, scale: u32, signed: bool) -> Self {
        Pic { digits, scale, signed, usage: Usage::Comp3 }
    }
    pub const fn binary(digits: u32, scale: u32, signed: bool) -> Self {
        Pic { digits, scale, signed, usage: Usage::Binary }
    }
    /// Storage length in bytes.
    #[allow(clippy::len_without_is_empty)]
    pub const fn len(&self) -> usize {
        match self.usage {
            Usage::Display => self.digits as usize,
            Usage::Comp3 => self.digits as usize / 2 + 1,
            Usage::Binary => {
                if self.digits <= 4 {
                    2
                } else if self.digits <= 9 {
                    4
                } else {
                    8
                }
            }
        }
    }
}

fn pow10(n: u32) -> Dec {
    let mut d = Dec::ONE;
    for _ in 0..n {
        d *= Dec::TEN;
    }
    d
}

/// Apply receiving-item truncation rules to a value (see module docs).
pub fn fit(v: Dec, pic: Pic) -> Dec {
    let mut v = v.trunc_with_scale(pic.scale);
    let int_digits = pic.digits - pic.scale;
    let limit = pow10(int_digits);
    let int = v.trunc();
    if int.abs() >= limit {
        let frac = v - int;
        v = (int % limit) + frac;
    }
    if !pic.signed {
        v = v.abs();
    }
    // normalise -0
    if v.is_zero() {
        v = Dec::ZERO;
    }
    v.rescale(pic.scale);
    v
}

/// The unscaled integer of `fit(v)` (e.g. 12.34 with scale 2 -> 1234).
fn unscaled(v: Dec, pic: Pic) -> i128 {
    let v = fit(v, pic);
    v.mantissa()
}

const POS_OVERPUNCH: &[u8; 10] = b"{ABCDEFGHI";
const NEG_OVERPUNCH: &[u8; 10] = b"}JKLMNOPQR";

/// Encode a value into `out` (which must be `pic.len()` bytes).
pub fn encode(v: Dec, pic: Pic, out: &mut [u8]) {
    debug_assert_eq!(out.len(), pic.len());
    let n = unscaled(v, pic);
    let neg = n < 0;
    let mag = n.unsigned_abs();
    match pic.usage {
        Usage::Display => {
            let s = format!("{:0width$}", mag, width = pic.digits as usize);
            let s = &s.as_bytes()[s.len() - pic.digits as usize..];
            out.copy_from_slice(s);
            if pic.signed {
                let last = out.len() - 1;
                let d = (out[last] - b'0') as usize;
                out[last] = if neg { NEG_OVERPUNCH[d] } else { POS_OVERPUNCH[d] };
            }
        }
        Usage::Comp3 => {
            let nibbles = out.len() * 2;
            let s = format!("{:0width$}", mag, width = nibbles - 1);
            let digits: Vec<u8> = s.bytes().map(|b| b - b'0').collect();
            let sign = if !pic.signed { 0xF } else if neg { 0xD } else { 0xC };
            let mut all = digits;
            all.push(sign);
            for (i, b) in out.iter_mut().enumerate() {
                *b = (all[2 * i] << 4) | all[2 * i + 1];
            }
        }
        Usage::Binary => {
            let len = out.len();
            let bytes = n.to_be_bytes();
            out.copy_from_slice(&bytes[16 - len..]);
        }
    }
}

/// Decode a numeric item. Invalid DISPLAY bytes are interpreted leniently
/// (low nibble of each byte, spaces = 0), like the mainframe does for non-strict
/// arithmetic on dirty data.
pub fn decode(b: &[u8], pic: Pic) -> Dec {
    let (mag, neg): (i128, bool) = match pic.usage {
        Usage::Display => {
            let mut mag: i128 = 0;
            let mut neg = false;
            for (i, &c) in b.iter().enumerate() {
                let last = i + 1 == b.len();
                let d = if c.is_ascii_digit() {
                    c - b'0'
                } else if let Some(p) = POS_OVERPUNCH.iter().position(|&x| x == c) {
                    p as u8
                } else if let Some(p) = NEG_OVERPUNCH.iter().position(|&x| x == c) {
                    if last && pic.signed {
                        neg = true;
                    }
                    p as u8
                } else if (b'p'..=b'y').contains(&c) {
                    // GnuCOBOL default ASCII negative sign nibble
                    if last && pic.signed {
                        neg = true;
                    }
                    c - b'p'
                } else {
                    (c & 0x0F) % 10
                };
                mag = mag * 10 + d as i128;
            }
            (mag, neg)
        }
        Usage::Comp3 => {
            let mut mag: i128 = 0;
            let mut neg = false;
            for (i, &c) in b.iter().enumerate() {
                let hi = (c >> 4) as i128;
                let lo = (c & 0x0F) as i128;
                mag = mag * 10 + hi.min(9);
                if i + 1 == b.len() {
                    neg = lo == 0xD || lo == 0xB;
                } else {
                    mag = mag * 10 + lo.min(9);
                }
            }
            (mag, neg && pic.signed)
        }
        Usage::Binary => {
            let mut buf = [0u8; 16];
            let len = b.len();
            buf[16 - len..].copy_from_slice(b);
            if pic.signed && b.first().is_some_and(|x| x & 0x80 != 0) {
                for x in buf[..16 - len].iter_mut() {
                    *x = 0xFF;
                }
            }
            let n = i128::from_be_bytes(buf);
            (n.abs(), n < 0)
        }
    };
    let mut d = Dec::from_i128_with_scale(mag, pic.scale);
    if neg {
        d = -d;
    }
    d
}

/// COBOL class test `IS NUMERIC` for a DISPLAY numeric or alphanumeric item.
pub fn is_numeric(b: &[u8], signed: bool) -> bool {
    if b.is_empty() {
        return false;
    }
    let (body, last) = b.split_at(b.len() - 1);
    body.iter().all(|c| c.is_ascii_digit())
        && (last[0].is_ascii_digit()
            || (signed && (POS_OVERPUNCH.contains(&last[0]) || NEG_OVERPUNCH.contains(&last[0]))))
}

/// Convert a decimal to i64 (integer part).
pub fn to_i64(d: Dec) -> i64 {
    d.trunc().to_i64().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Dec {
        Dec::from_str(s).unwrap()
    }

    #[test]
    fn zoned_roundtrip() {
        let p = Pic::display(12, 2, true);
        let mut b = [0u8; 12];
        encode(d("1940.00"), p, &mut b);
        assert_eq!(&b, b"00000019400{");
        assert_eq!(decode(&b, p), d("1940.00"));
        encode(d("-12.34"), p, &mut b);
        assert_eq!(&b, b"00000000123M");
        assert_eq!(decode(&b, p), d("-12.34"));
        assert_eq!(decode(b"00000001940{", p), d("194.00"));
    }

    #[test]
    fn truncation() {
        let p = Pic::display(4, 2, false);
        assert_eq!(fit(d("-123.456"), p), d("23.45"));
        let p = Pic::display(11, 2, true);
        assert_eq!(fit(d("1.239"), p), d("1.23"));
    }

    #[test]
    fn comp3() {
        let p = Pic::comp3(12, 2, true);
        let mut b = vec![0u8; p.len()];
        assert_eq!(b.len(), 7);
        encode(d("-2500.00"), p, &mut b);
        assert_eq!(b, vec![0x00, 0x00, 0x00, 0x02, 0x50, 0x00, 0x0D]);
        assert_eq!(decode(&b, p), d("-2500.00"));
    }

    #[test]
    fn binary() {
        let p = Pic::binary(9, 0, false);
        let mut b = [0u8; 4];
        encode(d("123456789"), p, &mut b);
        assert_eq!(decode(&b, p), d("123456789"));
        let p = Pic::binary(4, 0, true);
        let mut b = [0u8; 2];
        encode(d("-13"), p, &mut b);
        assert_eq!(decode(&b, p), d("-13"));
    }

    #[test]
    fn numeric_class() {
        assert!(is_numeric(b"00012", false));
        assert!(!is_numeric(b"0001 ", false));
        assert!(is_numeric(b"0001{", true));
        assert!(!is_numeric(b"0001{", false));
    }
}
