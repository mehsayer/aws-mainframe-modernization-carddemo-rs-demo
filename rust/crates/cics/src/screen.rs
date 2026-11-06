//! Terminal screen state (what a 3270 would hold) and the symbolic map used by
//! programs (the COBOL `xxxxAI` / `xxxxAO` areas).

use crate::bms::{Attr, Color, MapDef};
use cobol::{edit, text, Dec};

/// Attribute byte values a program can move into `xxxA` (DFHBMSCA).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrByte {
    /// DFHBMPRF: protected, MDT on
    ProtFset,
    /// DFHBMPRO: protected
    Prot,
    /// DFHBMFSE: unprotected, MDT on
    UnprotFset,
    /// DFHBMDAR: dark
    Dark,
    /// DFHBMBRY: bright
    Bright,
    /// DFHBMASK: autoskip
    Askip,
}

impl AttrByte {
    fn apply(self, base: Attr) -> Attr {
        let mut a = base;
        match self {
            AttrByte::ProtFset => {
                a.protected = true;
                a.fset = true;
                a.dark = false
            }
            AttrByte::Prot => {
                a.protected = true;
                a.dark = false;
                a.fset = false
            }
            AttrByte::UnprotFset => {
                a.protected = false;
                a.askip = false;
                a.fset = true;
                a.dark = false
            }
            AttrByte::Dark => {
                a.dark = true;
                a.bright = false
            }
            AttrByte::Bright => {
                a.bright = true;
                a.dark = false
            }
            AttrByte::Askip => {
                a.protected = true;
                a.askip = true
            }
        }
        a
    }
}

/// One field as shown on the terminal.
#[derive(Clone, Debug)]
pub struct ScreenField {
    pub name: Option<String>,
    pub row: usize,
    /// 1-based column of the first data character
    pub col: usize,
    pub len: usize,
    pub attr: Attr,
    pub color: Color,
    pub underline: bool,
    pub justify_right: bool,
    pub justify_zero: bool,
    /// cell contents; 0 = null
    pub data: Vec<u8>,
    pub mdt: bool,
}

impl ScreenField {
    /// Text as transmitted to the host (nulls removed).
    pub fn transmitted(&self) -> Vec<u8> {
        self.data.iter().copied().filter(|&c| c != 0).collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Screen {
    pub mapset: String,
    pub map: String,
    pub fields: Vec<ScreenField>,
    /// cursor (row, col), 1-based
    pub cursor: (usize, usize),
    /// plain text screen (SEND TEXT)
    pub text: Option<String>,
}

impl Screen {
    pub fn field(&self, name: &str) -> Option<&ScreenField> {
        self.fields.iter().find(|f| f.name.as_deref() == Some(name))
    }
    pub fn field_mut(&mut self, name: &str) -> Option<&mut ScreenField> {
        self.fields.iter_mut().find(|f| f.name.as_deref() == Some(name))
    }
    /// Field index at a position (data area only).
    pub fn field_at(&self, row: usize, col: usize) -> Option<usize> {
        self.fields.iter().position(|f| f.row == row && col >= f.col && col < f.col + f.len)
    }
    /// Type a value into a named field as a user would (used by tests / scripts).
    pub fn type_into(&mut self, name: &str, value: &str) {
        if let Some(f) = self.field_mut(name) {
            let mut d = vec![0u8; f.len];
            for (i, b) in value.bytes().take(f.len).enumerate() {
                d[i] = b;
            }
            f.data = d;
            f.mdt = true;
        }
    }
    /// The full 24x80 character matrix (for rendering and screenshots).
    pub fn render(&self) -> Vec<String> {
        let mut g = vec![vec![' '; 80]; 24];
        if let Some(t) = &self.text {
            for (i, ch) in t.chars().enumerate().take(80 * 24) {
                g[i / 80][i % 80] = ch;
            }
        }
        for f in &self.fields {
            if f.attr.dark {
                continue;
            }
            for (i, &b) in f.data.iter().enumerate() {
                let c = f.col - 1 + i;
                if f.row >= 1 && f.row <= 24 && c < 80 {
                    g[f.row - 1][c] = if b == 0 { ' ' } else { b as char };
                }
            }
        }
        g.into_iter().map(|r| r.into_iter().collect()).collect()
    }
}

/// Program-side storage of one map field (`xxxL`, `xxxA`, `xxxC`, `xxxI`/`xxxO`).
#[derive(Clone, Debug)]
pub struct MapField {
    pub data: Vec<u8>,
    pub len: i16,
    pub attr: Option<AttrByte>,
    pub color: Option<Color>,
    /// DFHDFCOL moved to the colour: use the map default
    pub default_color: bool,
}

/// Symbolic map (input and output share storage, as the COBOL REDEFINES do).
#[derive(Clone, Debug)]
pub struct Map {
    pub def: MapDef,
    pub fields: Vec<MapField>,
}

impl Map {
    pub fn new(def: &MapDef) -> Self {
        let fields = def
            .fields
            .iter()
            .map(|f| MapField { data: vec![0u8; f.len], len: 0, attr: None, color: None, default_color: false })
            .collect();
        Map { def: def.clone(), fields }
    }
    fn idx(&self, name: &str) -> usize {
        self.def.field(name).unwrap_or_else(|| panic!("map {} has no field {name}", self.def.name)).0
    }
    /// MOVE LOW-VALUES TO xxxxAO
    pub fn clear(&mut self) {
        for f in &mut self.fields {
            f.data.fill(0);
            f.len = 0;
            f.attr = None;
            f.color = None;
            f.default_color = false;
        }
    }
    pub fn get(&self, name: &str) -> &[u8] {
        &self.fields[self.idx(name)].data
    }
    /// MOVE value TO xxxO / xxxI (alphanumeric move: pad with spaces)
    pub fn set(&mut self, name: &str, v: &[u8]) {
        let i = self.idx(name);
        text::mv(&mut self.fields[i].data, v);
    }
    pub fn set_str(&mut self, name: &str, v: &str) {
        self.set(name, v.as_bytes())
    }
    /// MOVE LOW-VALUES TO xxxO
    pub fn set_low(&mut self, name: &str) {
        let i = self.idx(name);
        self.fields[i].data.fill(0);
    }
    /// MOVE numeric TO xxxO of a PICOUT field (edited)
    pub fn set_num(&mut self, name: &str, v: Dec) {
        let i = self.idx(name);
        let pic = self.def.fields[i].picout.clone();
        match pic {
            Some(p) => self.set(name, edit::edit(v, &p).as_bytes()),
            None => self.set(name, v.to_string().as_bytes()),
        }
    }
    /// MOVE -1 TO xxxL (cursor position)
    pub fn cursor(&mut self, name: &str) {
        let i = self.idx(name);
        self.fields[i].len = -1;
    }
    pub fn len(&self, name: &str) -> i16 {
        self.fields[self.idx(name)].len
    }
    pub fn color(&mut self, name: &str, c: Color) {
        let i = self.idx(name);
        self.fields[i].color = Some(c);
        self.fields[i].default_color = false;
    }
    /// MOVE DFHDFCOL TO xxxC
    pub fn default_color(&mut self, name: &str) {
        let i = self.idx(name);
        self.fields[i].color = None;
        self.fields[i].default_color = true;
    }
    pub fn attr(&mut self, name: &str, a: AttrByte) {
        let i = self.idx(name);
        self.fields[i].attr = Some(a);
    }
    pub fn has(&self, name: &str) -> bool {
        self.def.field(name).is_some()
    }

    /// Build the terminal screen for SEND MAP.
    pub fn to_screen(&self, prev: Option<&Screen>, erase: bool) -> Screen {
        let mut s = Screen { mapset: self.def.mapset.clone(), map: self.def.name.clone(), ..Default::default() };
        let mut cursor = None;
        let mut ic = None;
        for (d, m) in self.def.fields.iter().zip(&self.fields) {
            let attr = m.attr.map_or(d.attr, |a| a.apply(d.attr));
            let color = m.color.unwrap_or(d.color);
            let mut data = vec![0u8; d.len];
            let named = d.name.is_some();
            let prev_data = if !erase {
                prev.and_then(|p| d.name.as_ref().and_then(|n| p.field(n))).map(|f| f.data.clone())
            } else {
                None
            };
            if named && m.data.first().is_some_and(|&c| c != 0) {
                data.copy_from_slice(&m.data);
            } else if let Some(p) = prev_data {
                data = p;
            } else {
                let init = d.initial.as_bytes();
                data[..init.len().min(d.len)].copy_from_slice(&init[..init.len().min(d.len)]);
            }
            let sf = ScreenField {
                name: d.name.clone(),
                row: d.row,
                col: d.col + 1,
                len: d.len,
                attr,
                color,
                underline: d.underline,
                justify_right: d.justify_right,
                justify_zero: d.justify_zero,
                data,
                mdt: attr.fset,
            };
            if m.len == -1 && cursor.is_none() {
                cursor = Some((sf.row, sf.col));
            }
            if d.attr.ic && ic.is_none() {
                ic = Some((sf.row, sf.col));
            }
            s.fields.push(sf);
        }
        let first_unprot = s.fields.iter().find(|f| !f.attr.protected).map(|f| (f.row, f.col));
        s.cursor = cursor.or(ic).or(first_unprot).unwrap_or((1, 1));
        s
    }

    /// RECEIVE MAP: fill the symbolic map from the terminal screen.
    pub fn from_screen(def: &MapDef, scr: &Screen) -> Map {
        let mut m = Map::new(def);
        for (i, d) in def.fields.iter().enumerate() {
            let Some(n) = &d.name else { continue };
            let Some(sf) = scr.field(n) else { continue };
            if scr.map != def.name || !sf.mdt {
                continue; // not received: nulls, length 0
            }
            let t = sf.transmitted();
            let f = &mut m.fields[i];
            f.len = t.len() as i16;
            if t.is_empty() {
                continue;
            }
            if d.justify_right {
                let pad = if d.justify_zero { b'0' } else { b' ' };
                let mut v = vec![pad; d.len];
                let tt = text::trim(&t);
                let n = tt.len().min(d.len);
                v[d.len - n..].copy_from_slice(&tt[tt.len() - n..]);
                f.data = v;
            } else {
                text::mv(&mut f.data, &t);
            }
        }
        m
    }
}
