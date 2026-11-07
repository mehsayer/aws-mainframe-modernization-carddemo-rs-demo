//! The demo menu: what `carddemo` shows when it is started without arguments, e.g. by a double click on
//! `carddemo.exe` in Windows. The online screens, the nightly batch, the transaction report, a viewer for
//! the output files, a reset of the data, and (Linux, with the GnuCOBOL runtime) the original COBOL screens.

use crate::{online, open_store, report_job, term};
use anyhow::{Context, Result};
use batch::jobs::{run_job, Env, JobOpts};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

/// The nightly batch of the CardDemo README.
const NIGHTLY: [&str; 6] = ["TRANBKP", "POSTTRAN", "INTCALC", "TRANBKP", "COMBTRAN", "CREASTMT"];

pub fn run(home: &Path, seed: Option<PathBuf>) -> Result<()> {
    let Some(seed) = seed else {
        println!("The sample data was not found. It belongs in the folder data\\EBCDIC (data/EBCDIC) next to this program.");
        pause();
        return Ok(());
    };
    let cobol = cobol_available();
    loop {
        clear();
        println!("  CardDemo: the AWS mainframe sample application, translated from COBOL/CICS to Rust");
        println!("  Data: {}", home.display());
        println!();
        println!("  1  Online system: the 3270 screens");
        println!("     Sign on with USER0001 / PASSWORD (or ADMIN001 / PASSWORD for the administrator).");
        println!("     Enter: send, F3: back, Esc: clear, Tab: next field, Ctrl+Q: back to this menu");
        println!("  2  Nightly batch: post the daily transactions, interest, statements");
        println!("  3  Transaction report for a date range");
        println!("  4  View the output files (statements, reports, rejected transactions)");
        println!("  5  Reset the data to the shipped sample data");
        match &cobol {
            Ok(()) => println!("  6  The ORIGINAL COBOL screen programs (GnuCOBOL), for comparison with 1"),
            Err(e) if cfg!(target_os = "linux") => println!("  (6  original COBOL screens: not available here: {e})"),
            Err(_) => {}
        }
        println!("  0  Quit");
        println!();
        let choice = ask("  Choose: ")?;
        let r = match choice.as_deref() {
            None | Some("0") | Some("q") | Some("Q") => return Ok(()),
            Some("1") => with_data(home, &seed).and_then(|_| online(home, Some(seed.clone()), "CC00", false)),
            Some("2") => with_data(home, &seed).and_then(|_| nightly(home, &seed)),
            Some("3") => with_data(home, &seed).and_then(|_| report(home, &seed)),
            Some("4") => files(home),
            Some("5") => reset(home, &seed),
            Some("6") if cobol.is_ok() => with_data(home, &seed).and_then(|_| online(home, Some(seed.clone()), "CC00", true)),
            _ => continue,
        };
        if let Err(e) = r {
            println!("\n  Error: {e:#}");
            pause();
        }
    }
}

/// Where the program is: the release layout keeps the sample data and the COBOL modules next to it.
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

/// The original COBOL programs: their modules (shipped next to the program in the Linux release, see
/// `cobridge::modules_dir`) and GnuCOBOL's runtime installed.
fn cobol_available() -> Result<(), String> {
    cobridge::check()
}

/// Load the sample data on first use.
fn with_data(home: &Path, seed: &Path) -> Result<()> {
    let v = open_store(home)?;
    if v.count("USRSEC").unwrap_or(0) == 0 {
        println!("  Loading the sample data ...");
        batch::repro::reset(&v, seed).map_err(anyhow::Error::msg)?;
    }
    Ok(())
}

fn nightly(home: &Path, seed: &Path) -> Result<()> {
    let v = open_store(home)?;
    let env = Env { vsam: &v, root: home.to_path_buf(), seed: seed.to_path_buf(), echo: false };
    println!("\n  Nightly batch (job, step, program, return code):\n");
    for j in NIGHTLY {
        if report_job(j, run_job(&env, j, &JobOpts::default())) > 4 {
            break;
        }
    }
    println!("\n  The statements and the other output files: menu option 4.");
    pause();
    Ok(())
}

/// The transaction report. Its default range starts where the original JCL's does and ends today: the
/// nightly batch stamps the transactions it posts with the day it runs, after the JCL's fixed end date.
fn report(home: &Path, seed: &Path) -> Result<()> {
    let d = JobOpts::default();
    let today = cobol::clock::now().format("%Y-%m-%d").to_string();
    println!("\n  The report lists the transactions processed in a date range (run the nightly batch first).");
    let or_default = |answer: Option<String>, default: String| answer.filter(|s| !s.is_empty()).unwrap_or(default);
    let start = or_default(ask(&format!("  Start date (YYYY-MM-DD) [{}]: ", d.report_start))?, d.report_start.clone());
    let end = or_default(ask(&format!("  End date (YYYY-MM-DD) [{today}]: "))?, today);
    let v = open_store(home)?;
    let env = Env { vsam: &v, root: home.to_path_buf(), seed: seed.to_path_buf(), echo: false };
    println!();
    report_job("TRANREPT", run_job(&env, "TRANREPT", &JobOpts { report_start: start, report_end: end, ..d }));
    println!("\n  The report is among the output files: menu option 4.");
    pause();
    Ok(())
}

fn reset(home: &Path, seed: &Path) -> Result<()> {
    if ask("\n  Reset all data to the shipped sample data? (y/N): ")?.is_some_and(|a| a.eq_ignore_ascii_case("y")) {
        let v = open_store(home)?;
        for (ds, n) in batch::repro::reset(&v, seed).map_err(anyhow::Error::msg)? {
            println!("  {ds:<10} {n:>5} records");
        }
        let _ = std::fs::remove_dir_all(home.join("datasets"));
        std::fs::create_dir_all(home.join("datasets"))?;
        println!("  Done; the output files of earlier runs were removed.");
        pause();
    }
    Ok(())
}

/// The output files of the batch jobs, to view (or, for the HTML statements, to open in the browser).
fn files(home: &Path) -> Result<()> {
    loop {
        let mut list: Vec<PathBuf> = std::fs::read_dir(home.join("datasets"))
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect())
            .unwrap_or_default();
        list.sort();
        clear();
        if list.is_empty() {
            println!("  No output files yet: run the nightly batch (2) or the transaction report (3) first.");
            pause();
            return Ok(());
        }
        println!("  Output files in {}:\n", home.join("datasets").display());
        for (i, p) in list.iter().enumerate() {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            let size = p.metadata().map(|m| m.len()).unwrap_or(0);
            println!("  {:>2}  {name:<45} {size:>8} bytes  {}", i + 1, describe(&name));
        }
        println!("\n   0  Back");
        let Some(choice) = ask("\n  View file number: ")? else { return Ok(()) };
        let Ok(n) = choice.parse::<usize>() else { continue };
        if n == 0 {
            return Ok(());
        }
        let Some(p) = list.get(n - 1) else { continue };
        let html = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("html"));
        if html && ask("  Open it in the web browser? (Y/n): ")?.is_none_or(|a| !a.eq_ignore_ascii_case("n")) {
            open_in_browser(p);
            continue;
        }
        view(p)?;
    }
}

fn describe(name: &str) -> &'static str {
    match () {
        _ if name.contains("STATEMNT.HTML") => "customer statements (HTML)",
        _ if name.contains("STATEMNT.PS") => "customer statements (text)",
        _ if name.contains("DALYREJS") => "rejected daily transactions",
        _ if name.contains("TRANREPT") => "transaction report",
        _ if name.contains("SYSTRAN") => "interest transactions",
        _ if name.contains("TRANSACT.BKUP") => "transaction master backup",
        _ if name.contains("COMBINED") => "combined transactions",
        _ if name.contains("TRXFL") => "statement work file",
        _ => "",
    }
}

fn open_in_browser(p: &Path) {
    #[cfg(windows)]
    let r = std::process::Command::new("cmd").arg("/C").arg("start").arg("").arg(p).status();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(p).status();
    if r.map(|s| !s.success()).unwrap_or(true) {
        println!("  Could not open a browser; the file is {}", p.display());
        pause();
    }
}

/// The lines of an output file: text lines, or fixed-length records (the record length of the data set)
/// when the file has no line ends. Bytes that are not text (packed decimal, binary zeros) show as '.'.
fn lines_of(p: &Path) -> Result<Vec<String>> {
    let bytes = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    let show = |b: &[u8]| -> String { b.iter().map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' }).collect() };
    Ok(if bytes.contains(&b'\n') {
        bytes.split(|&c| c == b'\n').map(|l| show(l.strip_suffix(b"\r").unwrap_or(l))).collect()
    } else {
        let reclen = if name.contains("DALYREJS") { 430 } else { 350 };
        bytes.chunks(reclen).map(show).collect()
    })
}

/// A full-screen viewer: arrows, PgUp/PgDn, Home/End; Left/Right for wide records; q or Esc to go back.
fn view(p: &Path) -> Result<()> {
    let lines = lines_of(p)?;
    let title = format!(" {} ({} lines) ", p.file_name().unwrap_or_default().to_string_lossy(), lines.len());
    let (_on, mut t) = term::start()?;
    let (mut top, mut left) = (0usize, 0usize);
    loop {
        let mut height = 0usize;
        t.draw(|f| {
            let [body, foot] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(f.area());
            height = body.height.saturating_sub(2) as usize;
            let text: Vec<Line> =
                lines.iter().skip(top).take(height).map(|l| Line::raw(l.chars().skip(left).collect::<String>())).collect();
            f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(title.as_str())), body);
            f.render_widget(
                Paragraph::new(format!(
                    " line {}/{}  column {}   Up/Down PgUp/PgDn Home/End: scroll  Left/Right: sideways  q or Esc: back",
                    (top + 1).min(lines.len()),
                    lines.len(),
                    left + 1
                ))
                .style(Style::default().fg(Color::DarkGray)),
                foot,
            );
        })?;
        let Event::Key(k) = event::read()? else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let last = lines.len().saturating_sub(height.max(1));
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
            KeyCode::Down => top = (top + 1).min(last),
            KeyCode::Up => top = top.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => top = (top + height).min(last),
            KeyCode::PageUp => top = top.saturating_sub(height),
            KeyCode::Home => (top, left) = (0, 0),
            KeyCode::End => top = last,
            KeyCode::Right => left += 10,
            KeyCode::Left => left = left.saturating_sub(10),
            _ => {}
        }
    }
}

fn clear() {
    use crossterm::{cursor::MoveTo, terminal};
    let _ = crossterm::execute!(std::io::stdout(), terminal::Clear(terminal::ClearType::All), MoveTo(0, 0));
}

/// A line from the keyboard (trimmed); None at the end of input.
fn ask(prompt: &str) -> Result<Option<String>> {
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut s = String::new();
    Ok(match std::io::stdin().lock().read_line(&mut s)? {
        0 => None,
        _ => Some(s.trim().to_string()),
    })
}

fn pause() {
    let _ = ask("\n  Press Enter to continue ...");
}
