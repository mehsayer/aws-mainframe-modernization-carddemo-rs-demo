//! `carddemo` — CardDemo in Rust: terminal UI (CICS online) and batch job runner.

use anyhow::{bail, Context, Result};
use batch::jobs::{run_job, Env, JobOpts, JOBS};
use cics::bms::Color;
use cics::{Aid, Session};
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

mod term;

#[derive(Parser)]
#[command(name = "carddemo", version, about = "AWS CardDemo, translated from COBOL/CICS/VSAM to Rust")]
struct Cli {
    /// Data directory (SQLite store and sequential data sets)
    #[arg(long, env = "CARDDEMO_HOME", default_value = "carddemo-data")]
    home: PathBuf,
    /// Directory with the shipped EBCDIC sample data (cobol/app/data/EBCDIC)
    #[arg(long, env = "CARDDEMO_SEED")]
    seed: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Reset the data store to the shipped sample data
    Reset,
    /// List the batch jobs
    Jobs,
    /// Run a batch job (as its JCL would)
    Job {
        name: String,
        /// INTCALC run date PARM (default 2022071800)
        #[arg(long)]
        parm: Option<String>,
        /// TRANREPT start date (YYYY-MM-DD)
        #[arg(long)]
        start: Option<String>,
        /// TRANREPT end date (YYYY-MM-DD)
        #[arg(long)]
        end: Option<String>,
        /// Daily transaction input for POSTTRAN / DALYVRFY
        #[arg(long)]
        dalytran: Option<PathBuf>,
    },
    /// Run a sequence of jobs (default: the full nightly batch of the README)
    Batch {
        #[arg(default_values_t = ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN", "CREASTMT"].map(String::from))]
        jobs: Vec<String>,
    },
    /// Start the online system (3270-style terminal)
    Online {
        /// Initial transaction
        #[arg(long, default_value = "CC00")]
        tran: String,
        /// Run the ORIGINAL COBOL screen programs (GnuCOBOL, CICS bridge) instead of the Rust translation.
        /// Build the modules first: python3 rust/cobridge/cicsprep.py
        #[arg(long)]
        cobol: bool,
    },
    /// Unload every KSDS to DIR/<DATASET>.dat (fixed-length ASCII records, key order) — IDCAMS REPRO OUTFILE
    Unload { dir: PathBuf },
    /// Print the current screen of a scripted session (for documentation/screenshots)
    Script {
        /// Steps: `type:FIELD=value`, `key:ENTER|PF3|...`
        steps: Vec<String>,
    },
}

fn find_seed(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let mut dir = std::env::current_dir()?;
    loop {
        // works from anywhere inside the repository (cobol/app/data/EBCDIC) or inside cobol/
        for rel in ["cobol/app/data/EBCDIC", "app/data/EBCDIC"] {
            let c = dir.join(rel);
            if c.is_dir() {
                return Ok(c);
            }
        }
        if !dir.pop() {
            bail!("sample data not found: pass --seed <dir with AWS.M2.CARDDEMO.*.PS>");
        }
    }
}

fn open_store(home: &Path) -> Result<vsam::Vsam> {
    std::fs::create_dir_all(home.join("datasets"))?;
    vsam::Vsam::open(home.join("carddemo.db")).context("opening data store")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Reset => {
            let seed = find_seed(cli.seed)?;
            let v = open_store(&cli.home)?;
            for (ds, n) in batch::repro::reset(&v, &seed).map_err(anyhow::Error::msg)? {
                println!("{ds:<10} {n:>5} records");
            }
            println!("Data store {} reset from {}", cli.home.display(), seed.display());
        }
        Cmd::Jobs => {
            for (j, d) in JOBS {
                println!("{j:<9} {d}");
            }
        }
        Cmd::Job { name, parm, start, end, dalytran } => {
            let seed = find_seed(cli.seed)?;
            let v = open_store(&cli.home)?;
            let env = Env { vsam: &v, root: cli.home.clone(), seed, echo: true };
            let mut o = JobOpts { dalytran, ..JobOpts::default() };
            if let Some(p) = parm {
                o.intcalc_parm = p;
            }
            if let Some(s) = start {
                o.report_start = s;
            }
            if let Some(e) = end {
                o.report_end = e;
            }
            std::process::exit(report_job(&name, run_job(&env, &name.to_uppercase(), &o)));
        }
        Cmd::Batch { jobs } => {
            let seed = find_seed(cli.seed)?;
            let v = open_store(&cli.home)?;
            let env = Env { vsam: &v, root: cli.home.clone(), seed, echo: false };
            for j in jobs {
                let rc = report_job(&j, run_job(&env, &j.to_uppercase(), &JobOpts::default()));
                if rc > 4 {
                    std::process::exit(rc);
                }
            }
        }
        Cmd::Unload { dir } => {
            let v = open_store(&cli.home)?;
            std::fs::create_dir_all(&dir)?;
            for ds in vsam::CATALOG {
                let n = batch::repro::repro_out(&v, ds.name, &dir.join(format!("{}.dat", ds.name)), batch::seqfile::Format::Fixed)
                    .map_err(anyhow::Error::msg)?;
                println!("{:<10} {n:>5} records", ds.name);
            }
        }
        Cmd::Online { tran, cobol } => {
            let seed = find_seed(cli.seed.clone()).ok();
            online(&cli.home, seed, &tran, cobol)?;
        }
        Cmd::Script { steps } => {
            let v = open_store(&cli.home)?;
            let region = online::region(v);
            let mut s = Session::default();
            region.run(&mut s, Some("CC00"), Aid::Enter);
            for st in steps {
                if let Some(rest) = st.strip_prefix("type:") {
                    let (f, val) = rest.split_once('=').unwrap_or((rest, ""));
                    s.screen.type_into(f, val);
                } else if let Some(k) = st.strip_prefix("key:") {
                    region.run(&mut s, None, parse_aid(k)?);
                }
            }
            for l in s.screen.render() {
                println!("{l}");
            }
        }
    }
    Ok(())
}

fn parse_aid(k: &str) -> Result<Aid> {
    Ok(match k.to_uppercase().as_str() {
        "ENTER" => Aid::Enter,
        "CLEAR" => Aid::Clear,
        "PA1" => Aid::Pa1,
        "PA2" => Aid::Pa2,
        p if p.starts_with("PF") => Aid::Pf(p[2..].parse()?),
        _ => bail!("unknown key {k}"),
    })
}

fn report_job(name: &str, r: Result<Vec<batch::jobs::StepLog>, (Vec<batch::jobs::StepLog>, batch::Abend)>) -> i32 {
    let (logs, abend) = match r {
        Ok(l) => (l, None),
        Err((l, a)) => (l, Some(a)),
    };
    let mut maxcc = 0;
    for l in &logs {
        match &l.rc {
            Ok(rc) => {
                maxcc = maxcc.max(*rc);
                println!("{name:<9} {:<8} {:<8} RC={rc:04}", l.step, l.program);
            }
            Err(a) => println!("{name:<9} {:<8} {:<8} {a}", l.step, l.program),
        }
    }
    if let Some(a) = abend {
        println!("{name}: JOB ENDED WITH {a}");
        return 16;
    }
    println!("{name}: MAXCC={maxcc:04}");
    maxcc
}

// ============================================================================ TUI

struct Ui {
    cursor: (usize, usize),
    status: String,
}

fn color(c: Color, prot: bool, bright: bool) -> ratatui::style::Color {
    use ratatui::style::Color as C;
    match c {
        Color::Blue => C::LightBlue,
        Color::Red => C::LightRed,
        Color::Pink => C::LightMagenta,
        Color::Green => C::LightGreen,
        Color::Turquoise => C::LightCyan,
        Color::Yellow => C::LightYellow,
        Color::Neutral => C::White,
        Color::Default => match (prot, bright) {
            (true, false) => C::LightBlue,
            (true, true) => C::White,
            (false, false) => C::LightGreen,
            (false, true) => C::LightRed,
        },
    }
}

fn draw(f: &mut Frame, sess: &Session, ui: &Ui) {
    let area = f.area();
    let ox = area.width.saturating_sub(80) / 2;
    let oy = area.height.saturating_sub(26) / 2;
    let buf = f.buffer_mut();
    let scr = &sess.screen;
    if let Some(t) = &scr.text {
        for (i, ch) in t.chars().enumerate().take(80 * 24) {
            let (r, c) = ((i / 80) as u16, (i % 80) as u16);
            if oy + r < area.height && ox + c < area.width {
                buf[(ox + c, oy + r)].set_char(ch).set_fg(ratatui::style::Color::LightGreen);
            }
        }
    }
    for fld in &scr.fields {
        if fld.attr.dark || fld.row == 0 || fld.row > 24 {
            continue;
        }
        let mut style = Style::default().fg(color(fld.color, fld.attr.protected, fld.attr.bright));
        if fld.attr.bright {
            style = style.add_modifier(Modifier::BOLD);
        }
        let underline = fld.underline || !fld.attr.protected;
        if underline {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        for (i, &b) in fld.data.iter().enumerate() {
            let c = (fld.col - 1 + i) as u16;
            let r = (fld.row - 1) as u16;
            if c < 80 && oy + r < area.height && ox + c < area.width {
                let ch = if b == 0 { ' ' } else { b as char };
                buf[(ox + c, oy + r)].set_char(ch).set_style(style);
            }
        }
    }
    let help = "Enter  F1-F12=PF1-12  Shift+F1-F12=PF13-24  Esc=Clear  Tab/Shift+Tab  Ctrl+E=EraseEOF  Ctrl+Q=Quit";
    let status = if ui.status.is_empty() { help.to_string() } else { ui.status.clone() };
    let p = Paragraph::new(Line::from(vec![Span::styled(status, Style::default().fg(ratatui::style::Color::DarkGray))]));
    f.render_widget(p, Rect { x: ox, y: (oy + 25).min(area.height.saturating_sub(1)), width: 80.min(area.width), height: 1 });
    f.set_cursor_position(Position { x: ox + ui.cursor.1 as u16 - 1, y: oy + ui.cursor.0 as u16 - 1 });
}

fn unprotected(s: &cics::Screen) -> Vec<usize> {
    let mut v: Vec<usize> = s.fields.iter().enumerate().filter(|(_, f)| !f.attr.protected && f.len > 0 && f.name.is_some()).map(|(i, _)| i).collect();
    v.sort_by_key(|&i| (s.fields[i].row, s.fields[i].col));
    v
}

fn online(home: &Path, seed: Option<PathBuf>, tran: &str, cobol: bool) -> Result<()> {
    // panic hook and exit handler first: with --cobol, libcob's initialisation below may end the process
    term::install();
    let v = open_store(home)?;
    if v.count("USRSEC").unwrap_or(0) == 0 {
        if let Some(seed) = &seed {
            batch::repro::reset(&v, seed).map_err(anyhow::Error::msg)?;
        }
    }
    let mut region = if cobol { cobol_region(v, home)? } else { online::region(v) };
    let queue: Arc<Mutex<Vec<Vec<Vec<u8>>>>> = Arc::new(Mutex::new(Vec::new()));
    let q2 = queue.clone();
    region.on_job = Some(Box::new(move |deck: &[Vec<u8>]| q2.lock().unwrap().push(deck.to_vec())));
    // the UI goes on before the first transaction, so that already runs under the UI's panic hook and
    // (--cobol) with descriptor 2 on the sysout file; the guard takes it off on every way out
    let (_ui_on, mut term) = term::start()?;
    #[cfg(debug_assertions)]
    if std::env::var_os("CARDDEMO_TEST_STACK_OVERFLOW").is_some() {
        test_stack_overflow(0); // tests/terminal.rs: a crash while the UI is on
    }
    let mut sess = Session::default();
    region.run(&mut sess, Some(tran), Aid::Enter);
    let mut ui = Ui { cursor: sess.screen.cursor, status: String::new() };
    (|| -> Result<()> {
        loop {
            term::ensure_on(&mut term)?;
            term.draw(|f| draw(f, &sess, &ui))?;
            let Event::Key(k) = event::read()? else { continue };
            if k.kind != event::KeyEventKind::Press {
                continue;
            }
            if let Some(aid) = aid_of(&k) {
                ui.status.clear();
                if sess.next_transid.is_none() && sess.screen.text.is_some() && aid == Aid::Enter {
                    region.run(&mut sess, Some("CC00"), Aid::Enter);
                } else {
                    region.run(&mut sess, None, aid);
                }
                ui.cursor = sess.screen.cursor;
                if sess.next_transid.is_none() && sess.screen.text.is_some() {
                    ui.status = "Session ended. Press Enter to sign on again, Ctrl+Q to quit.".into();
                }
                // internal reader: run submitted report jobs
                let decks: Vec<_> = queue.lock().unwrap().drain(..).collect();
                for d in decks {
                    ui.status = run_submitted(home, seed.clone(), &d);
                }
                continue;
            }
            if !edit_key(&mut sess, &mut ui, &k) {
                break;
            }
        }
        Ok(())
    })()
}

/// Recurses until the stack overflows (debug builds, `CARDDEMO_TEST_STACK_OVERFLOW`; see tests/terminal.rs).
#[cfg(debug_assertions)]
#[allow(unconditional_recursion)]
fn test_stack_overflow(n: u64) -> u64 {
    let frame = std::hint::black_box([n; 512]);
    test_stack_overflow(n + 1) + frame[0]
}

fn aid_of(k: &KeyEvent) -> Option<Aid> {
    let shift = k.modifiers.contains(KeyModifiers::SHIFT);
    match k.code {
        KeyCode::Enter => Some(Aid::Enter),
        KeyCode::Esc => Some(Aid::Clear),
        KeyCode::F(n) if (1..=12).contains(&n) => Some(Aid::Pf(if shift { n + 12 } else { n })),
        _ => None,
    }
}

/// Keyboard editing of unprotected fields. Returns false to quit.
fn edit_key(sess: &mut Session, ui: &mut Ui, k: &KeyEvent) -> bool {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let scr = &mut sess.screen;
    let (r, c) = ui.cursor;
    match k.code {
        KeyCode::Char('q') | KeyCode::Char('c') if ctrl => return false,
        KeyCode::Char('e') if ctrl => {
            if let Some(i) = scr.field_at(r, c) {
                let f = &mut scr.fields[i];
                if !f.attr.protected {
                    for x in f.data.iter_mut().skip(c - f.col) {
                        *x = 0;
                    }
                    f.mdt = true;
                }
            }
        }
        KeyCode::Tab | KeyCode::BackTab => {
            let u = unprotected(scr);
            if !u.is_empty() {
                let pos = |i: usize| (scr.fields[i].row, scr.fields[i].col);
                let next = if k.code == KeyCode::Tab {
                    u.iter().find(|&&i| pos(i) > (r, c)).or(u.first())
                } else {
                    u.iter().rev().find(|&&i| pos(i) < (r, c)).or(u.last())
                };
                if let Some(&i) = next {
                    ui.cursor = pos(i);
                }
            }
        }
        KeyCode::Left => ui.cursor.1 = if c > 1 { c - 1 } else { 80 },
        KeyCode::Right => ui.cursor.1 = if c < 80 { c + 1 } else { 1 },
        KeyCode::Up => ui.cursor.0 = if r > 1 { r - 1 } else { 24 },
        KeyCode::Down => ui.cursor.0 = if r < 24 { r + 1 } else { 1 },
        KeyCode::Home => {
            if let Some(&i) = unprotected(scr).first() {
                ui.cursor = (scr.fields[i].row, scr.fields[i].col);
            }
        }
        KeyCode::Backspace => {
            if let Some(i) = scr.field_at(r, c.saturating_sub(1)) {
                let f = &mut scr.fields[i];
                if !f.attr.protected {
                    f.data[c - 1 - f.col] = 0;
                    f.mdt = true;
                    ui.cursor.1 = c - 1;
                }
            }
        }
        KeyCode::Delete => {
            if let Some(i) = scr.field_at(r, c) {
                let f = &mut scr.fields[i];
                if !f.attr.protected {
                    let p = c - f.col;
                    f.data.remove(p);
                    f.data.push(0);
                    f.mdt = true;
                }
            }
        }
        KeyCode::Char(ch) if !ctrl => {
            let Some(i) = scr.field_at(r, c) else {
                ui.status = "X PROT: cursor is not in an input field".into();
                return true;
            };
            let f = &mut scr.fields[i];
            if f.attr.protected {
                ui.status = "X PROT: protected field".into();
                return true;
            }
            if f.attr.numeric && !(ch.is_ascii_digit() || ch == '.' || ch == '-') {
                ui.status = "X NUM: numeric field".into();
                return true;
            }
            let p = c - f.col;
            f.data[p] = if ch.is_ascii() { ch as u8 } else { b'?' };
            f.mdt = true;
            ui.status.clear();
            if p + 1 < f.len {
                ui.cursor.1 = c + 1;
            } else {
                // auto-advance to the next input field
                let (fr, fc) = (f.row, f.col);
                let u = unprotected(scr);
                if let Some(&n) = u.iter().find(|&&j| (scr.fields[j].row, scr.fields[j].col) > (fr, fc)).or(u.first()) {
                    ui.cursor = (scr.fields[n].row, scr.fields[n].col);
                }
            }
        }
        _ => {}
    }
    true
}

/// Internal reader: run a TRANREPT job submitted by CORPT00C.
fn run_submitted(home: &Path, seed: Option<PathBuf>, deck: &[Vec<u8>]) -> String {
    let lines: Vec<String> = deck.iter().map(|l| String::from_utf8_lossy(l).trim_end().to_string()).collect();
    let dates = lines
        .iter()
        .skip_while(|l| !l.starts_with("//STEP10R.DATEPARM"))
        .nth(1)
        .cloned()
        .unwrap_or_default();
    let (start, end) = (dates.get(0..10).unwrap_or("").to_string(), dates.get(11..21).unwrap_or("").to_string());
    let _ = std::fs::write(home.join("datasets/JES2.SPOOL.TRNRPT00.jcl"), lines.join("\n") + "\n");
    let Ok(v) = open_store(home) else { return "TRNRPT00: cannot open data store".into() };
    let env = Env { vsam: &v, root: home.to_path_buf(), seed: seed.unwrap_or_default(), echo: false };
    let o = JobOpts { report_start: start.clone(), report_end: end.clone(), ..JobOpts::default() };
    match run_job(&env, "TRANREPT", &o) {
        Ok(_) => format!("Job TRNRPT00 ended RC=0000: report {start}..{end} in {}", env.gdg("TRANREPT", 0).display()),
        Err((_, a)) => format!("Job TRNRPT00 ended with {a}"),
    }
}

/// The original COBOL programs on the CICS bridge. Their DISPLAY output and any libcob message go to
/// `<home>/cobol-sysout.log` (the terminal UI keeps the terminal); if libcob ends the process (runtime
/// error, STOP RUN), the terminal is restored on the way out and the user is told where libcob's message
/// is (see the term module).
fn cobol_region(v: vsam::Vsam, home: &Path) -> Result<cics::Region> {
    cobridge::set_sysout(&home.join("cobol-sysout.log"))?;
    // libcob now, before the UI: its signal handlers must be in place before the UI adds its own
    cobridge::start().map_err(anyhow::Error::msg)?;
    Ok(cobridge::region(v))
}
