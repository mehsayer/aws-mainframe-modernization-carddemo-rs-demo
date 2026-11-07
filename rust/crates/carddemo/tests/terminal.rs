//! The terminal UI in a real pseudo-terminal: whatever ends the program (quit, a kill, a failed setup, a
//! crash, libcob ending the process), the terminal is left as it was found: out of the alternate screen
//! (left exactly once) and back in canonical mode with echo. Linux only (forkpty).
#![cfg(target_os = "linux")]

use std::ffi::{c_int, CString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

enum Step {
    /// wait (up to 20 s) until the screen text contains this (whitespace ignored: the terminal is not sent
    /// the blanks between words, only cursor moves)
    WaitFor(&'static str),
    /// wait (up to 20 s) until the UI has entered the alternate screen this many times
    UiOn(usize),
    /// wait (up to 20 s) until the UI has left the alternate screen this many times
    UiOff(usize),
    Keys(&'static [u8]),
    Signal(c_int),
}

struct Run {
    out: Vec<u8>,
    status: c_int,
    /// the terminal's local modes after the program ended
    lflag: libc::tcflag_t,
}

impl Run {
    fn count(&self, seq: &[u8]) -> usize {
        self.out.windows(seq.len()).filter(|w| *w == seq).count()
    }

    /// Text written after the UI left the alternate screen, without escape sequences.
    fn after_ui(&self) -> String {
        let leave = b"\x1b[?1049l";
        let start = self.out.windows(leave.len()).rposition(|w| w == leave).map_or(0, |i| i + leave.len());
        strip(&self.out[start..])
    }

    fn assert_terminal_restored(&self) {
        self.assert_restored_after(1);
    }

    /// The UI was put on `times` times and taken off as often, and the terminal is as it was.
    fn assert_restored_after(&self, times: usize) {
        assert_eq!(self.count(b"\x1b[?1049h"), times, "entered the alternate screen {times} time(s)");
        assert_eq!(self.count(b"\x1b[?1049l"), times, "left the alternate screen {times} time(s)");
        assert!(self.lflag & libc::ICANON != 0 && self.lflag & libc::ECHO != 0, "terminal back in canonical mode with echo");
    }
}

fn strip(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn home(name: &str) -> PathBuf {
    let h = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("terminal-{name}"));
    let _ = std::fs::remove_dir_all(&h);
    h
}

/// Run `carddemo <args>` in a 30x100 pseudo-terminal with `env` added, driving it with `steps`.
fn run(args: &[&str], env: &[(&str, String)], stdout_full: bool, steps: &[Step]) -> Run {
    run_program(Path::new(env!("CARGO_BIN_EXE_carddemo")), Path::new("."), args, env, stdout_full, steps)
}

/// The same for the program at `prog`, started in directory `cwd`.
fn run_program(prog: &Path, cwd: &Path, args: &[&str], env: &[(&str, String)], stdout_full: bool, steps: &[Step]) -> Run {
    // everything the child needs is prepared before fork (only async-signal-safe calls after it)
    let prog = CString::new(prog.to_str().unwrap()).unwrap();
    let cwd = CString::new(cwd.to_str().unwrap()).unwrap();
    let argv_c: Vec<CString> = std::iter::once(prog.clone()).chain(args.iter().map(|a| CString::new(*a).unwrap())).collect();
    let mut argv: Vec<*const libc::c_char> = argv_c.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());
    let envp_c: Vec<CString> = std::env::vars()
        .filter(|(k, _)| !env.iter().any(|(e, _)| e == k))
        .map(|(k, v)| format!("{k}={v}"))
        .chain(env.iter().map(|(k, v)| format!("{k}={v}")))
        .map(|s| CString::new(s).unwrap())
        .collect();
    let mut envp: Vec<*const libc::c_char> = envp_c.iter().map(|c| c.as_ptr()).collect();
    envp.push(std::ptr::null());
    let full = c"/dev/full";

    let ws = libc::winsize { ws_row: 30, ws_col: 100, ws_xpixel: 0, ws_ypixel: 0 };
    let mut master: c_int = -1;
    let pid = unsafe { libc::forkpty(&mut master, std::ptr::null_mut(), std::ptr::null(), &ws) };
    assert!(pid >= 0, "forkpty");
    if pid == 0 {
        unsafe {
            // default signal handling, whatever the test runner inherited (nohup ignores SIGHUP)
            let mut dfl: libc::sigaction = std::mem::zeroed();
            dfl.sa_sigaction = libc::SIG_DFL;
            for sig in 1..32 {
                if sig != libc::SIGKILL && sig != libc::SIGSTOP {
                    libc::sigaction(sig, &dfl, std::ptr::null_mut());
                }
            }
            let mut none: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut none);
            libc::sigprocmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
            libc::chdir(cwd.as_ptr());
            if stdout_full {
                let fd = libc::open(full.as_ptr(), libc::O_WRONLY);
                libc::dup2(fd, 1);
            }
            libc::execve(prog.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::_exit(127);
        }
    }
    let mut out = Vec::new();
    let pump = |out: &mut Vec<u8>, ms: u64| {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end {
            let mut p = libc::pollfd { fd: master, events: libc::POLLIN, revents: 0 };
            if unsafe { libc::poll(&mut p, 1, 50) } > 0 {
                let mut buf = [0u8; 65536];
                let n = unsafe { libc::read(master, buf.as_mut_ptr().cast(), buf.len()) };
                if n <= 0 {
                    return;
                }
                out.extend_from_slice(&buf[..n as usize]);
            }
        }
    };
    let wait_until = |out: &mut Vec<u8>, what: &str, done: &dyn Fn(&[u8]) -> bool| {
        let end = Instant::now() + Duration::from_secs(20);
        while !done(out) {
            assert!(Instant::now() < end, "timed out waiting for {what}; output: {}", strip(out));
            pump(out, 100);
        }
    };
    for s in steps {
        match s {
            Step::WaitFor(text) => {
                let want: String = text.split_whitespace().collect();
                wait_until(&mut out, text, &|o: &[u8]| strip(o).split_whitespace().collect::<String>().contains(&want))
            }
            Step::UiOn(n) => wait_until(&mut out, "the alternate screen", &|o: &[u8]| {
                o.windows(8).filter(|w| *w == b"\x1b[?1049h").count() >= *n
            }),
            Step::UiOff(n) => wait_until(&mut out, "leaving the alternate screen", &|o: &[u8]| {
                o.windows(8).filter(|w| *w == b"\x1b[?1049l").count() >= *n
            }),
            Step::Keys(k) => {
                unsafe { libc::write(master, k.as_ptr().cast(), k.len()) };
                pump(&mut out, 300);
            }
            Step::Signal(sig) => unsafe {
                libc::kill(pid, *sig);
            },
        }
    }
    // wait for the end (and read what it writes on the way out)
    let mut status = 0;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        pump(&mut out, 100);
        if unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } == pid {
            break;
        }
        if Instant::now() > deadline {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                libc::waitpid(pid, &mut status, 0);
            }
            panic!("carddemo did not end; output: {}", strip(&out));
        }
    }
    pump(&mut out, 200);
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    unsafe { libc::tcgetattr(master, &mut t) };
    unsafe { libc::close(master) };
    Run { out, status, lflag: t.c_lflag }
}

fn data(name: &str) -> Vec<(&'static str, String)> {
    vec![
        ("CARDDEMO_HOME", home(name).display().to_string()),
        ("CARDDEMO_SEED", batch::repro::sample_seed_dir().display().to_string()),
        ("RUST_BACKTRACE", "0".into()),
    ]
}

fn exit_code(r: &Run) -> Option<c_int> {
    libc::WIFEXITED(r.status).then(|| libc::WEXITSTATUS(r.status))
}

fn signal(r: &Run) -> Option<c_int> {
    libc::WIFSIGNALED(r.status).then(|| libc::WTERMSIG(r.status))
}

#[test]
fn quit() {
    let r = run(&["online"], &data("quit"), false, &[Step::WaitFor("Sign-on"), Step::Keys(b"\x11")]);
    assert_eq!(exit_code(&r), Some(0), "output: {}", strip(&r.out));
    assert!(strip(&r.out).contains("CardDemo"), "sign-on screen drawn");
    r.assert_terminal_restored();
}

#[test]
fn killed() {
    for sig in [libc::SIGTERM, libc::SIGHUP, libc::SIGUSR1] {
        let r = run(&["online"], &data(&format!("killed-{sig}")), false, &[Step::WaitFor("Sign-on"), Step::Signal(sig)]);
        assert_eq!(signal(&r), Some(sig), "ended by the signal");
        r.assert_terminal_restored();
    }
}

#[test]
fn setup_fails() {
    // stdout cannot be written: the setup fails after raw mode is on
    let r = run(&["online"], &data("setup"), true, &[]);
    assert_eq!(exit_code(&r), Some(1));
    assert!(strip(&r.out).contains("No space left on device"), "output: {}", strip(&r.out));
    assert!(r.lflag & libc::ICANON != 0 && r.lflag & libc::ECHO != 0, "terminal back in canonical mode");
}

#[test]
fn stack_overflow() {
    if !cfg!(debug_assertions) {
        return; // the trigger exists in debug builds only
    }
    let mut env = data("overflow");
    env.push(("CARDDEMO_TEST_STACK_OVERFLOW", "1".into()));
    let r = run(&["online"], &env, false, &[Step::UiOn(1)]);
    assert_eq!(signal(&r), Some(libc::SIGABRT), "Rust aborts after reporting the overflow");
    assert!(r.after_ui().contains("has overflowed its stack"), "Rust's message reaches the terminal: {:?}", r.after_ui());
    r.assert_terminal_restored();
}

/// The COBOL module directory, if the modules are built and libcob is there.
fn cobol() -> Option<PathBuf> {
    cobridge::check().ok().map(|_| cobridge::modules_dir())
}

#[test]
fn cobol_init_fails() {
    let Some(_) = cobol() else { return eprintln!("COBOL modules not built: skipped") };
    let mut env = data("cobinit");
    env.push(("COB_RUNTIME_CONFIG", "/nonexistent/runtime.cfg".into()));
    let r = run(&["online", "--cobol"], &env, false, &[]);
    assert_eq!(exit_code(&r), Some(1));
    let text = strip(&r.out);
    assert!(text.contains("the COBOL runtime ended the program"), "reported on the terminal: {text:?}");
    let sysout = std::fs::read_to_string(Path::new(&env[0].1).join("cobol-sysout.log")).unwrap_or_default();
    assert!(sysout.contains("configuration error"), "libcob's message in the sysout file: {sysout:?}");
    assert_eq!(r.count(b"\x1b[?1049h"), 0, "ended before the UI was on");
}

#[test]
fn cobol_runtime_error_under_the_ui() {
    let Some(modules) = cobol() else { return eprintln!("COBOL modules not built: skipped") };
    // a copy of the modules without CSUTLDTC: the report screen's date check CALLs it
    let mods = home("rterr-modules");
    std::fs::create_dir_all(&mods).unwrap();
    for e in std::fs::read_dir(&modules).unwrap() {
        let p = e.unwrap().path();
        if p.file_name().is_some_and(|n| n != "CSUTLDTC.so") {
            std::fs::copy(&p, mods.join(p.file_name().unwrap())).unwrap();
        }
    }
    let mut env = data("rterr");
    env.push(("CARDDEMO_COBOL_MODULES", mods.display().to_string()));
    // 8-character fields autoskip to the next field, so no Tab after a full field
    let steps = [
        Step::WaitFor("Sign-on"),
        Step::Keys(b"USER0001"),
        Step::Keys(b"PASSWORD"),
        Step::Keys(b"\r"),
        Step::WaitFor("Main Menu"),
        Step::Keys(b"9"),
        Step::Keys(b"\r"),
        Step::WaitFor("Reports"),
        Step::Keys(b"\t"),
        Step::Keys(b"\t"),
        Step::Keys(b"X"),
        Step::Keys(b"01"),
        Step::Keys(b"01"),
        Step::Keys(b"2022"),
        Step::Keys(b"12"),
        Step::Keys(b"31"),
        Step::Keys(b"2022"),
        Step::Keys(b"Y"),
        Step::Keys(b"\r"),
    ];
    let r = run(&["online", "--cobol"], &env, false, &steps);
    assert_eq!(exit_code(&r), Some(1), "libcob ended the program; output: {:?}", r.after_ui());
    assert!(r.after_ui().contains("the COBOL runtime ended the program"), "after the UI: {:?}", r.after_ui());
    r.assert_terminal_restored();
    let sysout = std::fs::read_to_string(Path::new(&env[0].1).join("cobol-sysout.log")).unwrap_or_default();
    assert!(sysout.contains("module 'CSUTLDTC' not found"), "libcob's message in the sysout file: {sysout:?}");
}

#[test]
fn ui_taken_off_mid_session_comes_back() {
    if !cfg!(debug_assertions) {
        return; // the trigger exists in debug builds only
    }
    // as if the panic hook had misjudged a panic the bridge then caught: term::ensure_on puts the UI back
    let mut env = data("uioff");
    env.push(("CARDDEMO_TEST_UI_OFF", "1".into()));
    let r = run(&["online"], &env, false, &[Step::UiOn(2), Step::WaitFor("Sign-on"), Step::Keys(b"\x11")]);
    assert_eq!(exit_code(&r), Some(0), "output: {}", strip(&r.out));
    r.assert_restored_after(2);
}

#[test]
fn cobol_bridge_panic_ends_the_task_and_the_ui_stays() {
    let Some(_) = cobol().filter(|_| cfg!(debug_assertions)) else { return eprintln!("needs the COBOL modules and a debug build") };
    let mut env = data("bridgepanic");
    env.push(("CARDDEMO_TEST_PANIC", "unwind".into()));
    let r = run(&["online", "--cobol"], &env, false, &[Step::WaitFor("abend ASRA"), Step::Keys(b"\x11")]);
    assert_eq!(exit_code(&r), Some(0), "the session went on: {}", strip(&r.out));
    r.assert_terminal_restored();
    assert!(!r.after_ui().contains("panicked"), "the panic was not printed over the terminal: {:?}", r.after_ui());
    let sysout = std::fs::read_to_string(Path::new(&env[0].1).join("cobol-sysout.log")).unwrap_or_default();
    assert!(sysout.contains("test seam: EXEC CICS command panics"), "the panic is in the sysout file: {sysout:?}");
}

#[test]
fn cobol_bridge_panic_that_cannot_unwind_is_reported() {
    let Some(_) = cobol().filter(|_| cfg!(debug_assertions)) else { return eprintln!("needs the COBOL modules and a debug build") };
    let mut env = data("nounwind");
    env.push(("CARDDEMO_TEST_PANIC", "nounwind".into()));
    let r = run(&["online", "--cobol"], &env, false, &[Step::UiOn(1)]);
    assert_eq!(signal(&r), Some(libc::SIGABRT), "the process aborts");
    r.assert_terminal_restored();
    assert!(r.after_ui().contains("could not be caught and ended the program"), "the note reaches the terminal: {:?}", r.after_ui());
}

#[test]
fn demo_menu_as_released() {
    // the release layout: the program with the sample data next to it, started from elsewhere (a double
    // click), no data directory given: the menu finds the data and keeps its own next to the program
    let stage = home("release");
    std::fs::create_dir_all(stage.join("data/EBCDIC")).unwrap();
    for e in std::fs::read_dir(batch::repro::sample_seed_dir()).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, stage.join("data/EBCDIC").join(p.file_name().unwrap())).unwrap();
    }
    let prog = stage.join("carddemo");
    std::fs::copy(env!("CARGO_BIN_EXE_carddemo"), &prog).unwrap();
    let steps = [
        Step::WaitFor("Choose:"),
        Step::Keys(b"2\r"), // nightly batch
        Step::WaitFor("CREASTMT: MAXCC=0000"),
        Step::Keys(b"\r"),
        Step::Keys(b"4\r"), // output files
        Step::WaitFor("View file number"),
        Step::Keys(b"3\r"), // DALYREJS, STATEMNT.HTML, STATEMNT.PS, ...: the text statements
        Step::WaitFor("START OF STATEMENT"),
        Step::Keys(b"q"),
        Step::WaitFor("View file number"),
        Step::Keys(b"0\r"),
        Step::WaitFor("Choose:"),
        Step::Keys(b"3\r"), // the transaction report, default dates (up to today: the batch posted today)
        Step::WaitFor("Start date"),
        Step::Keys(b"\r"),
        Step::Keys(b"\r"),
        Step::WaitFor("TRANREPT: MAXCC=0000"),
        Step::Keys(b"\r"),
        Step::Keys(b"4\r"),
        Step::WaitFor("View file number"),
        Step::Keys(b"5\r"), // ..., SYSTRAN, TRANREPT: the report, with its transactions
        Step::WaitFor("Daily Transaction Report"),
        Step::WaitFor("Account Total"),
        Step::Keys(b"q"),
        Step::WaitFor("View file number"),
        Step::Keys(b"0\r"),
        Step::WaitFor("Choose:"),
        Step::Keys(b"1\r"), // the online screens, and back with Ctrl+Q
        Step::WaitFor("Sign-on"),
        Step::Keys(b"\x11"),
        Step::UiOn(3),
        Step::Keys(b"0\r"),
    ];
    let env = [("RUST_BACKTRACE", "0".to_string())];
    let r = run_program(&prog, Path::new("/"), &[], &env, false, &steps);
    assert_eq!(exit_code(&r), Some(0), "output: {}", strip(&r.out));
    r.assert_restored_after(3); // the viewer twice, the online screens
    assert!(stage.join("carddemo-data/datasets/AWS.M2.CARDDEMO.STATEMNT.PS").is_file(), "data kept next to the program");
}

#[test]
fn demo_cobol_after_rust_screens_still_restores_on_a_kill() {
    // libcob installs its signal handlers when it starts; ours must come back on top when the COBOL
    // screens follow the Rust screens in the same process (demo menu 1, then 6)
    let Some(_) = cobol() else { return eprintln!("COBOL modules not built: skipped") };
    let steps = [
        Step::WaitFor("Choose:"),
        Step::Keys(b"1\r"),
        Step::WaitFor("Sign-on"),
        Step::Keys(b"\x11"),
        Step::UiOff(1),
        Step::Keys(b"6\r"),
        Step::UiOn(2),
        Step::Signal(libc::SIGTERM),
    ];
    let r = run(&["demo"], &data("cobolafter"), false, &steps);
    assert!(signal(&r).is_some(), "ended by the signal");
    r.assert_restored_after(2);
}
