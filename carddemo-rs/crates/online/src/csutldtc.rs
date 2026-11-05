//! CSUTLDTC — date validation through LE CEEDAYS (FR-BAT-UTIL-02).
//!
//! Returns the 80-byte result area of the original: severity `9(4)`, `Mesg Code:`,
//! message number `9(4)`, result text, `TstDate:` + date, `Mask used:` + mask.
//! CEEDAYS feedback codes: 2507 insufficient data, 2508 bad date value, 2513 year
//! outside 1582..9999, 2517 invalid month, 2518 bad picture string, 2520 non-numeric.

pub struct DateCheck {
    pub severity: u16,
    pub msg_no: u16,
    pub result: &'static str,
}

impl DateCheck {
    pub fn severity_str(&self) -> String {
        format!("{:04}", self.severity)
    }
    pub fn msg_no_str(&self) -> String {
        format!("{:04}", self.msg_no)
    }
}

fn days_in_month(y: u32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Validate `date` (10 bytes) against `mask` (`YYYY-MM-DD` or `YYYYMMDD`).
pub fn csutldtc(date: &[u8], mask: &[u8]) -> DateCheck {
    let ok = DateCheck { severity: 0, msg_no: 0, result: "Date is valid" };
    let bad = |n: u16, r: &'static str| DateCheck { severity: 3, msg_no: n, result: r };
    let mask = cobol::text::s(mask);
    let mask = mask.trim();
    let (yy, mm, dd): (&[u8], &[u8], &[u8]) = match mask {
        "YYYY-MM-DD" => {
            if date.len() < 10 {
                return bad(2507, "Insufficient");
            }
            (&date[0..4], &date[5..7], &date[8..10])
        }
        "YYYYMMDD" => {
            if date.len() < 8 {
                return bad(2507, "Insufficient");
            }
            (&date[0..4], &date[4..6], &date[6..8])
        }
        _ => return bad(2518, "Bad Pic String"),
    };
    if date.iter().take(mask.len()).all(|&c| c == b' ' || c == 0) {
        return bad(2507, "Insufficient");
    }
    let num = |b: &[u8]| -> Option<u32> {
        if b.iter().all(|c| c.is_ascii_digit()) {
            std::str::from_utf8(b).ok()?.parse().ok()
        } else {
            None
        }
    };
    let (Some(y), Some(m), Some(d)) = (num(yy), num(mm), num(dd)) else {
        return bad(2520, "Nonnumeric data");
    };
    if !(1..=12).contains(&m) {
        return bad(2517, "Invalid month");
    }
    if !(1582..=9999).contains(&y) {
        return bad(2513, "Unsupp. Range");
    }
    if d == 0 || d > days_in_month(y, m) {
        return bad(2508, "Datevalue error");
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates() {
        assert_eq!(csutldtc(b"2022-02-28", b"YYYY-MM-DD").severity, 0);
        assert_eq!(csutldtc(b"2024-02-29", b"YYYY-MM-DD").severity, 0);
        assert_eq!(csutldtc(b"2023-02-29", b"YYYY-MM-DD").msg_no, 2508);
        assert_eq!(csutldtc(b"2023-13-01", b"YYYY-MM-DD").msg_no, 2517);
        assert_eq!(csutldtc(b"2023-1a-01", b"YYYY-MM-DD").msg_no, 2520);
        assert_eq!(csutldtc(b"1200-01-01", b"YYYY-MM-DD").msg_no, 2513);
        assert_eq!(csutldtc(b"20230115", b"YYYYMMDD").severity, 0);
    }
}
