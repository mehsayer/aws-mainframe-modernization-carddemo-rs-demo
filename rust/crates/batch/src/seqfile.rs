//! Sequential (QSAM-like) files: fixed-length records stored either as
//! line-sequential ASCII text (`\n` terminated, the format of `cobol/app/data/ASCII`) or as
//! raw fixed-length records (optionally EBCDIC, the format of `cobol/app/data/EBCDIC`).

use cobol::ebcdic;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// one record per line, ASCII; short lines padded with spaces
    Lines,
    /// fixed-length records, no separators, ASCII
    Fixed,
    /// fixed-length records, no separators, EBCDIC (cp037)
    Ebcdic,
}

impl Format {
    /// Guess the format from the file name / location.
    pub fn guess(path: &Path) -> Format {
        let s = path.to_string_lossy();
        if s.contains("EBCDIC") || s.ends_with(".PS") || s.ends_with(".INIT") {
            Format::Ebcdic
        } else if s.ends_with(".dat") || s.ends_with(".bin") {
            Format::Fixed
        } else {
            Format::Lines
        }
    }
}

/// Read all records of a sequential file.
pub fn read_all(path: &Path, rec_len: usize, fmt: Format) -> std::io::Result<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    match fmt {
        Format::Lines => {
            for line in BufReader::new(File::open(path)?).split(b'\n') {
                let mut l = line?;
                if l.last() == Some(&b'\r') {
                    l.pop();
                }
                l.resize(rec_len, b' ');
                out.push(l);
            }
        }
        Format::Fixed | Format::Ebcdic => {
            let mut b = Vec::new();
            File::open(path)?.read_to_end(&mut b)?;
            if fmt == Format::Ebcdic {
                ebcdic::to_ascii(&mut b);
            }
            for c in b.chunks(rec_len) {
                let mut r = c.to_vec();
                r.resize(rec_len, b' ');
                out.push(r);
            }
        }
    }
    Ok(out)
}

/// Sequential input file (READ ... AT END).
pub struct SeqIn {
    recs: std::vec::IntoIter<Vec<u8>>,
}

impl SeqIn {
    pub fn open(path: &Path, rec_len: usize, fmt: Format) -> std::io::Result<Self> {
        Ok(SeqIn { recs: read_all(path, rec_len, fmt)?.into_iter() })
    }
    pub fn from_records(recs: Vec<Vec<u8>>) -> Self {
        SeqIn { recs: recs.into_iter() }
    }
    /// Next record, or None at end of file (FILE STATUS '10').
    pub fn read(&mut self) -> Option<Vec<u8>> {
        self.recs.next()
    }
}

/// Sequential output file. Line-sequential output trims nothing: each record is
/// written with its full length followed by `\n` (fixed-length line records).
pub struct SeqOut {
    w: BufWriter<File>,
    fmt: Format,
    pub path: PathBuf,
}

impl SeqOut {
    pub fn create(path: &Path, fmt: Format) -> std::io::Result<Self> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        Ok(SeqOut { w: BufWriter::new(File::create(path)?), fmt, path: path.to_path_buf() })
    }
    /// DISP=MOD: append.
    pub fn append(path: &Path, fmt: Format) -> std::io::Result<Self> {
        let f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        Ok(SeqOut { w: BufWriter::new(f), fmt, path: path.to_path_buf() })
    }
    pub fn write(&mut self, rec: &[u8]) -> std::io::Result<()> {
        match self.fmt {
            Format::Lines => {
                self.w.write_all(rec)?;
                self.w.write_all(b"\n")
            }
            Format::Fixed => self.w.write_all(rec),
            Format::Ebcdic => {
                let mut r = rec.to_vec();
                ebcdic::to_ebcdic(&mut r);
                self.w.write_all(&r)
            }
        }
    }
    pub fn close(mut self) -> std::io::Result<()> {
        self.w.flush()
    }
}
