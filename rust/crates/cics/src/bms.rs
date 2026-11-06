//! BMS map source parser (DFHMSD / DFHMDI / DFHMDF macros).
//!
//! The original `cobol/app/bms/*.bms` files are parsed at start-up, so the screens are
//! rendered from the same source as on the mainframe.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Color {
    #[default]
    Default,
    Blue,
    Red,
    Pink,
    Green,
    Turquoise,
    Yellow,
    Neutral,
}

impl Color {
    pub fn parse(s: &str) -> Color {
        match s {
            "BLUE" => Color::Blue,
            "RED" => Color::Red,
            "PINK" => Color::Pink,
            "GREEN" => Color::Green,
            "TURQUOISE" => Color::Turquoise,
            "YELLOW" => Color::Yellow,
            "NEUTRAL" => Color::Neutral,
            _ => Color::Default,
        }
    }
}

/// Field attributes (3270 attribute byte, decoded).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Attr {
    pub protected: bool,
    pub askip: bool,
    pub numeric: bool,
    pub bright: bool,
    pub dark: bool,
    pub fset: bool,
    pub ic: bool,
}

impl Attr {
    pub fn parse(list: &str) -> Attr {
        let mut a = Attr::default();
        for t in list.trim_matches(|c| c == '(' || c == ')').split(',') {
            match t.trim() {
                "PROT" => a.protected = true,
                "ASKIP" => {
                    a.protected = true;
                    a.askip = true
                }
                "UNPROT" => a.protected = false,
                "NUM" => a.numeric = true,
                "BRT" => a.bright = true,
                "DRK" => a.dark = true,
                "NORM" => {}
                "FSET" => a.fset = true,
                "IC" => a.ic = true,
                _ => {}
            }
        }
        a
    }
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: Option<String>,
    /// 1-based row/column of the attribute byte; data starts at col + 1
    pub row: usize,
    pub col: usize,
    pub len: usize,
    pub attr: Attr,
    pub color: Color,
    pub underline: bool,
    pub initial: String,
    pub picout: Option<String>,
    pub justify_right: bool,
    pub justify_zero: bool,
}

#[derive(Clone, Debug)]
pub struct MapDef {
    pub mapset: String,
    pub name: String,
    pub fields: Vec<FieldDef>,
    index: HashMap<String, usize>,
}

impl MapDef {
    pub fn field(&self, name: &str) -> Option<(usize, &FieldDef)> {
        self.index.get(name).map(|&i| (i, &self.fields[i]))
    }
}

/// Join continuation lines (non-blank in column 72) into whole macro statements.
/// A literal continued across lines keeps its text up to column 71 and resumes in
/// column 16 of the next line.
fn statements(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for raw in src.lines() {
        if raw.starts_with('*') {
            continue;
        }
        let chars: Vec<char> = raw.chars().collect();
        let cont = chars.get(71).is_some_and(|&c| c != ' ');
        let mut line: String = chars.iter().take(71).collect();
        while line.chars().count() < 71 {
            line.push(' ');
        }
        let text: String = if in_quote {
            line.chars().skip(15).collect()
        } else if cur.is_empty() {
            line.clone()
        } else {
            line.trim_start().to_string()
        };
        let ends_in_quote = in_quote ^ (text.matches('\'').count() % 2 == 1);
        if ends_in_quote {
            cur.push_str(&text);
        } else {
            cur.push_str(text.trim_end());
        }
        in_quote = ends_in_quote;
        if !cont {
            out.push(std::mem::take(&mut cur));
            in_quote = false;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Split `A=1,B=(x,y),C='a,b'` into key/value pairs.
fn operands(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let (mut depth, mut q) = (0, false);
    let mut cur = String::new();
    let mut push = |cur: &mut String| {
        if let Some((k, v)) = cur.split_once('=') {
            out.push((k.trim().to_string(), v.trim().to_string()));
        }
        cur.clear();
    };
    for c in s.chars() {
        match c {
            '\'' => q = !q,
            '(' if !q => depth += 1,
            ')' if !q => depth -= 1,
            ',' if !q && depth == 0 => {
                push(&mut cur);
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    push(&mut cur);
    out
}

fn unquote(v: &str) -> String {
    v.trim_matches('\'').replace("''", "'").replace("&&", "&")
}

/// Parse one BMS mapset source into its maps.
pub fn parse_mapset(src: &str) -> Vec<MapDef> {
    let mut maps = Vec::new();
    let mut mapset = String::new();
    for st in statements(src) {
        let (label, rest) = if st.starts_with(' ') { (None, st.trim_start()) } else {
            let (l, r) = st.split_once(' ').unwrap_or((&st, ""));
            (Some(l.to_string()), r.trim_start())
        };
        let (op, rest) = rest.split_once(' ').unwrap_or((rest, ""));
        let rest = rest.trim().to_string();
        let op = op.to_string();
        let ops = operands(&rest);
        let get = |k: &str| ops.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        match op.as_str() {
            "DFHMSD" => {
                if let Some(l) = &label {
                    mapset = l.clone();
                }
            }
            "DFHMDI" => maps.push(MapDef { mapset: mapset.clone(), name: label.clone().unwrap_or_default(), fields: Vec::new(), index: HashMap::new() }),
            "DFHMDF" => {
                let Some(map) = maps.last_mut() else { continue };
                let pos = get("POS").unwrap_or_default();
                let (r, c) = pos.trim_matches(|c| c == '(' || c == ')').split_once(',').unwrap_or(("1", "1"));
                let just = get("JUSTIFY").unwrap_or_default();
                let f = FieldDef {
                    name: label.clone().filter(|s| !s.is_empty()),
                    row: r.trim().parse().unwrap_or(1),
                    col: c.trim().parse().unwrap_or(1),
                    len: get("LENGTH").and_then(|v| v.parse().ok()).unwrap_or(0),
                    attr: Attr::parse(&get("ATTRB").unwrap_or_default()),
                    color: Color::parse(&get("COLOR").unwrap_or_default()),
                    underline: get("HILIGHT").as_deref() == Some("UNDERLINE"),
                    initial: get("INITIAL").map(|v| unquote(&v)).unwrap_or_default(),
                    picout: get("PICOUT").map(|v| unquote(&v)),
                    justify_right: just.contains("RIGHT"),
                    justify_zero: just.contains("ZERO"),
                };
                if let Some(n) = &f.name {
                    map.index.insert(n.clone(), map.fields.len());
                }
                map.fields.push(f);
            }
            _ => {}
        }
    }
    maps
}

/// All maps of the application, by map name.
#[derive(Default)]
pub struct MapCatalog {
    pub maps: HashMap<String, MapDef>,
}

impl MapCatalog {
    pub fn load_dir(dir: &std::path::Path) -> std::io::Result<Self> {
        let mut c = MapCatalog::default();
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("bms")) {
                c.add(&std::fs::read_to_string(&p)?);
            }
        }
        Ok(c)
    }
    pub fn add(&mut self, src: &str) {
        for m in parse_mapset(src) {
            self.maps.insert(m.name.clone(), m);
        }
    }
    pub fn get(&self, name: &str) -> Option<&MapDef> {
        self.maps.get(name.trim())
    }
    /// Maps compiled into the binary (copies of cobol/app/bms).
    pub fn builtin() -> Self {
        let mut c = MapCatalog::default();
        for src in BUILTIN {
            c.add(src);
        }
        c
    }
}

const BUILTIN: &[&str] = &[
    include_str!("../../../assets/bms/COACTUP.bms"),
    include_str!("../../../assets/bms/COACTVW.bms"),
    include_str!("../../../assets/bms/COADM01.bms"),
    include_str!("../../../assets/bms/COBIL00.bms"),
    include_str!("../../../assets/bms/COCRDLI.bms"),
    include_str!("../../../assets/bms/COCRDSL.bms"),
    include_str!("../../../assets/bms/COCRDUP.bms"),
    include_str!("../../../assets/bms/COMEN01.bms"),
    include_str!("../../../assets/bms/CORPT00.bms"),
    include_str!("../../../assets/bms/COSGN00.bms"),
    include_str!("../../../assets/bms/COTRN00.bms"),
    include_str!("../../../assets/bms/COTRN01.bms"),
    include_str!("../../../assets/bms/COTRN02.bms"),
    include_str!("../../../assets/bms/COUSR00.bms"),
    include_str!("../../../assets/bms/COUSR01.bms"),
    include_str!("../../../assets/bms/COUSR02.bms"),
    include_str!("../../../assets/bms/COUSR03.bms"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signon_map() {
        let c = MapCatalog::builtin();
        assert_eq!(c.maps.len(), 17);
        let m = c.get("COSGN0A").unwrap();
        let (_, f) = m.field("USERID").unwrap();
        assert_eq!(f.len, 8);
        assert!(!f.attr.protected);
        // continued INITIAL literal
        assert!(m.fields.iter().any(|f| f.initial == "This is a Credit Card Demo Application for Mainframe Modernization"));
        let v = c.get("CACTVWA").unwrap();
        assert_eq!(v.field("ACURBAL").unwrap().1.picout.as_deref(), Some("+ZZZ,ZZZ,ZZZ.99"));
        for (n, m) in &c.maps {
            for f in &m.fields {
                assert!(f.col + f.len <= 81, "{n} field {:?} overflows", f.name);
            }
        }
    }
}
