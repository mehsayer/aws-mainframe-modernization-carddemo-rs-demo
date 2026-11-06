//! Numeric-edited pictures (`-ZZZ,ZZZ,ZZZ.ZZ`, `+99999999.99`, `Z(9).99-`, ...).
//!
//! Supported symbols: `9`, `Z`, `.`, `,`, `B`, `0`, `/`, a single fixed `+` or `-`
//! at the start or end, and floating `-`/`+` strings at the start. This covers every
//! edited picture used by CardDemo.

use crate::numeric::Dec;

fn expand(pic: &str) -> Vec<char> {
    let mut out = Vec::new();
    let cs: Vec<char> = pic.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if i + 1 < cs.len() && cs[i + 1] == '(' {
            let close = cs[i..].iter().position(|&x| x == ')').unwrap() + i;
            let n: usize = cs[i + 2..close].iter().collect::<String>().parse().unwrap();
            out.extend(std::iter::repeat_n(c, n));
            i = close + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Format `v` according to the edited picture `pic`.
pub fn edit(v: Dec, pic: &str) -> String {
    let sym = expand(&pic.to_ascii_uppercase());
    let neg = v.is_sign_negative() && !v.is_zero();
    // floating sign: two or more leading '+'/'-'
    let lead = sym[0];
    let floating = (lead == '+' || lead == '-') && sym.len() > 1 && sym[1] == lead;
    // digit positions
    let point = sym.iter().position(|&c| c == '.');
    let is_digit_pos = |i: usize, c: char| -> bool {
        matches!(c, '9' | 'Z') || (floating && c == lead && i > 0)
    };
    let scale = match point {
        Some(p) => sym[p..].iter().enumerate().filter(|(j, &c)| is_digit_pos(p + j, c)).count(),
        None => 0,
    };
    let ndig = sym.iter().enumerate().filter(|(i, &c)| is_digit_pos(*i, c)).count();
    let mag = v.abs().trunc_with_scale(scale as u32);
    let unscaled = mag.mantissa() * 10i128.pow((scale as u32).saturating_sub(mag.scale()));
    let s = format!("{:0w$}", unscaled, w = ndig);
    let digits: Vec<char> = s.chars().skip(s.len() - ndig).collect();

    let all_z = sym.iter().all(|&c| !matches!(c, '9')) && !floating;
    if all_z && unscaled == 0 && sym.contains(&'Z') {
        return " ".repeat(sym.len());
    }

    let sign_char = |c: char| -> char {
        match (c, neg) {
            ('+', false) => '+',
            ('+', true) | ('-', true) => '-',
            _ => ' ',
        }
    };

    let mut out = Vec::with_capacity(sym.len());
    let mut di = 0;
    let mut significant = false; // a non-suppressed digit has been emitted
    let mut float_pending = floating;
    for (i, &c) in sym.iter().enumerate() {
        match c {
            '9' => {
                if float_pending {
                    // place floating sign just before first 9 if not placed yet
                    let fs = sign_char(lead);
                    if let Some(last) = out.iter().rposition(|&x| x == ' ') {
                        out[last] = fs;
                    }
                    float_pending = false;
                }
                out.push(digits[di]);
                di += 1;
                significant = true;
            }
            'Z' => {
                let d = digits[di];
                di += 1;
                if !significant && d == '0' && point.is_none_or(|p| i < p) {
                    out.push(' ');
                } else {
                    significant = true;
                    out.push(d);
                }
            }
            '+' | '-' if floating && i > 0 && is_digit_pos(i, c) => {
                let d = digits[di];
                di += 1;
                if !significant && d == '0' {
                    out.push(' ');
                } else {
                    if !significant {
                        // first significant digit: sign goes in the previous position
                        if let Some(prev) = out.last_mut() {
                            *prev = sign_char(lead);
                        }
                        float_pending = false;
                    }
                    significant = true;
                    out.push(d);
                }
            }
            '+' | '-' => out.push(sign_char(c)),
            '.' => {
                if float_pending && !significant {
                    if let Some(prev) = out.last_mut() {
                        *prev = sign_char(lead);
                    }
                    float_pending = false;
                }
                significant = true;
                out.push('.');
            }
            ',' => out.push(if significant { ',' } else { ' ' }),
            'B' => out.push(' '),
            '0' => out.push('0'),
            '/' => out.push('/'),
            other => out.push(other),
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    fn d(s: &str) -> Dec {
        Dec::from_str(s).unwrap()
    }
    #[test]
    fn pictures() {
        assert_eq!(edit(d("1234.5"), "+99999999.99"), "+00001234.50");
        assert_eq!(edit(d("-1234.5"), "+99999999.99"), "-00001234.50");
        assert_eq!(edit(d("1234567.89"), "-ZZZ,ZZZ,ZZZ.ZZ"), "   1,234,567.89");
        assert_eq!(edit(d("-5.5"), "-ZZZ,ZZZ,ZZZ.ZZ"), "-          5.50");
        assert_eq!(edit(d("0"), "-ZZZ,ZZZ,ZZZ.ZZ"), " ".repeat(15));
        assert_eq!(edit(d("12.3"), "+ZZZ,ZZZ,ZZZ.ZZ"), format!("+{:>14}", "12.30"));
        assert_eq!(edit(d("-12.3"), "Z(9).99-"), format!("{:>12}-", "12.30"));
        assert_eq!(edit(d("12.3"), "9(9).99-"), "000000012.30 ");
        assert_eq!(edit(d("0.07"), "+ZZZ,ZZZ,ZZZ.99"), format!("+{:>14}", ".07"));
    }
}
