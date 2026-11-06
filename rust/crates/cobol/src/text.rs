//! Alphanumeric helpers with COBOL semantics (MOVE, figurative constants, STRING,
//! INSPECT, intrinsic functions TRIM / UPPER-CASE / NUMVAL / NUMVAL-C).

use crate::numeric::Dec;
use std::str::FromStr;

pub const LOW: u8 = 0x00;
pub const HIGH: u8 = 0xFF;

/// `MOVE src TO dst` for alphanumeric items: left-justify, pad with spaces, truncate.
pub fn mv(dst: &mut [u8], src: &[u8]) {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    dst[n..].fill(b' ');
}

/// `MOVE src TO dst` for a JUSTIFIED RIGHT receiver.
pub fn mv_just_right(dst: &mut [u8], src: &[u8]) {
    if src.len() >= dst.len() {
        dst.copy_from_slice(&src[src.len() - dst.len()..]);
    } else {
        let pad = dst.len() - src.len();
        dst[..pad].fill(b' ');
        dst[pad..].copy_from_slice(src);
    }
}

pub fn is_spaces(b: &[u8]) -> bool {
    b.iter().all(|&c| c == b' ')
}
pub fn is_low(b: &[u8]) -> bool {
    b.iter().all(|&c| c == LOW)
}
pub fn is_zeros(b: &[u8]) -> bool {
    b.iter().all(|&c| c == b'0')
}
/// `X = SPACES OR X = LOW-VALUES`
pub fn is_blank(b: &[u8]) -> bool {
    is_spaces(b) || is_low(b)
}

/// Lossy conversion of a field to a Rust string (low-values become spaces).
pub fn s(b: &[u8]) -> String {
    b.iter().map(|&c| if c == 0 { ' ' } else { c as char }).collect()
}

/// FUNCTION TRIM (leading and trailing spaces).
pub fn trim(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|&c| c != b' ').unwrap_or(b.len());
    let end = b.iter().rposition(|&c| c != b' ').map_or(start, |e| e + 1);
    &b[start..end]
}

pub fn upper(b: &[u8]) -> Vec<u8> {
    b.to_ascii_uppercase()
}
pub fn lower(b: &[u8]) -> Vec<u8> {
    b.to_ascii_lowercase()
}

/// INSPECT ... CONVERTING `from` TO `to`.
pub fn inspect_converting(b: &mut [u8], from: &[u8], to: &[u8]) {
    for c in b.iter_mut() {
        if let Some(p) = from.iter().position(|&f| f == *c) {
            *c = to[p];
        }
    }
}

/// Bytes up to (not including) the first occurrence of `delim` (STRING ... DELIMITED BY).
pub fn until<'a>(b: &'a [u8], delim: &[u8]) -> &'a [u8] {
    if delim.is_empty() {
        return b;
    }
    b.windows(delim.len()).position(|w| w == delim).map_or(b, |p| &b[..p])
}

/// The STRING statement. Pieces are appended into `dst` starting at offset 0;
/// the rest of `dst` is left untouched (as in COBOL). Returns true on overflow.
pub fn string_into(dst: &mut [u8], pieces: &[&[u8]]) -> bool {
    let mut pos = 0;
    for p in pieces {
        for &c in *p {
            if pos >= dst.len() {
                return true;
            }
            dst[pos] = c;
            pos += 1;
        }
    }
    false
}

fn parse_numval(s: &str, currency: bool) -> Option<Dec> {
    let mut t = s.trim().to_string();
    let mut neg = false;
    // trailing sign / CR / DB
    for suf in ["-", "CR", "DB", "cr", "db"] {
        if t.ends_with(suf) {
            neg = true;
            t.truncate(t.len() - suf.len());
            t = t.trim_end().to_string();
            break;
        }
    }
    if t.ends_with('+') {
        t.pop();
        t = t.trim_end().to_string();
    }
    if let Some(r) = t.strip_prefix('-') {
        if neg {
            return None;
        }
        neg = true;
        t = r.trim_start().to_string();
    } else if let Some(r) = t.strip_prefix('+') {
        t = r.trim_start().to_string();
    }
    if currency {
        if let Some(r) = t.strip_prefix('$') {
            t = r.trim_start().to_string();
        }
        t = t.replace(',', "");
    }
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || c == '.') || t.matches('.').count() > 1 {
        return None;
    }
    if t == "." {
        return None;
    }
    let t = if t.starts_with('.') { format!("0{t}") } else { t };
    let mut d = Dec::from_str(&t).ok()?;
    if neg {
        d = -d;
    }
    Some(d)
}

/// FUNCTION NUMVAL; invalid input yields 0 (as the runtime does).
pub fn numval(b: &[u8]) -> Dec {
    parse_numval(&s(b), false).unwrap_or(Dec::ZERO)
}
/// FUNCTION NUMVAL-C
pub fn numval_c(b: &[u8]) -> Dec {
    parse_numval(&s(b), true).unwrap_or(Dec::ZERO)
}
/// FUNCTION TEST-NUMVAL: 0 when valid, otherwise non-zero.
pub fn test_numval(b: &[u8]) -> i32 {
    if parse_numval(&s(b), false).is_some() { 0 } else { 1 }
}
/// FUNCTION TEST-NUMVAL-C: 0 when valid, otherwise non-zero.
pub fn test_numval_c(b: &[u8]) -> i32 {
    if parse_numval(&s(b), true).is_some() { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numvals() {
        assert_eq!(numval_c(b"  -1,234.50 "), Dec::from_str("-1234.50").unwrap());
        assert_eq!(numval_c(b"+00001234.50"), Dec::from_str("1234.50").unwrap());
        assert_eq!(test_numval_c(b"12a"), 1);
        assert_eq!(test_numval(b"  07"), 0);
        assert_eq!(numval(b"07"), Dec::from(7));
    }
    #[test]
    fn moves() {
        let mut d = [0u8; 5];
        mv(&mut d, b"ab");
        assert_eq!(&d, b"ab   ");
        mv_just_right(&mut d, b"7");
        assert_eq!(&d, b"    7");
        assert_eq!(trim(b"  x y  "), b"x y");
        assert_eq!(until(b"JOHN  SMITH", b" "), b"JOHN");
    }
}
