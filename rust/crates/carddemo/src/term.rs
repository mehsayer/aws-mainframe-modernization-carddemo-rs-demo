//! The terminal while the full-screen UI is on: raw mode and the alternate screen, set up as
//! `ratatui::try_init()` does but with this program's own panic hook, and taken off again on every way out:
//! the normal end or an error ([`UiOn`]), a panic ([`panic_hook`]), a process exit, e.g. libcob ending the
//! process from inside a COBOL program ([`at_exit`]), and a fatal signal (`signals`). With `--cobol`,
//! descriptor 2 points at the COBOL sysout file while the UI is on: libcob writes a few messages straight
//! to it, and they must not land on the screen.

use anyhow::Result;
use ratatui::prelude::*;
#[cfg(unix)]
use std::sync::atomic::AtomicI32;
use std::sync::atomic::{AtomicBool, Ordering};

/// True while the UI is on the terminal (see [`end`]).
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// The real descriptor 2 while it points at the COBOL sysout file (-1 otherwise).
#[cfg(unix)]
static SAVED_STDERR: AtomicI32 = AtomicI32::new(-1);

/// Install the panic hook and the exit handler (once). Call it first thing: with `--cobol`, before
/// `cobridge::start()`, which may already end the process (libcob's initialisation) and must be reported.
pub fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(panic_hook(std::panic::take_hook(), end, cobridge::write_sysout));
        #[cfg(unix)]
        unsafe {
            libc::atexit(at_exit);
        }
    });
}

/// Put the UI on the terminal. The returned guard takes it off again when dropped, also when a later step
/// of the setup fails. With `--cobol`, call `cobridge::start()` first (libcob installs its signal handlers
/// there; ours must come after them, to run first and pass the signal on).
pub fn start() -> Result<(UiOn, ratatui::DefaultTerminal)> {
    install();
    #[cfg(unix)]
    signals::install(); // saves the terminal settings: before raw mode
    let on = UiOn;
    put_on()?;
    Ok((on, Terminal::new(CrosstermBackend::new(std::io::stdout()))?))
}

/// Raw mode, descriptor 2 on the sysout file (`--cobol`), the alternate screen.
fn put_on() -> Result<()> {
    use crossterm::{execute, terminal};
    terminal::enable_raw_mode()?;
    ACTIVE.store(true, Ordering::SeqCst);
    #[cfg(unix)]
    stderr_to_sysout()?;
    execute!(std::io::stdout(), terminal::EnterAlternateScreen)?;
    Ok(())
}

/// Put the UI back if it was taken off while the program goes on. That happens only if the panic hook
/// misjudged a panic the COBOL bridge then caught (see `cobridge::note_panic`): the task ended with ASRA,
/// and the session continues on a restored screen instead of a terminal in cooked mode.
pub fn ensure_on(term: &mut ratatui::DefaultTerminal) -> Result<()> {
    if !ACTIVE.load(Ordering::SeqCst) {
        put_on()?;
        term.clear()?;
    }
    Ok(())
}

/// The UI is on the terminal; dropping this takes it off ([`end`]).
pub struct UiOn;

impl Drop for UiOn {
    fn drop(&mut self) {
        end();
    }
}

/// Take the UI off the terminal, once: whichever comes first (the guard, the panic hook, the exit handler,
/// a signal handler) puts descriptor 2 back and restores the terminal, so a second LeaveAlternateScreen
/// cannot move the cursor over later output, and panic and exit messages reach the terminal. Never panics.
pub fn end() {
    if ACTIVE.swap(false, Ordering::SeqCst) {
        stderr_back();
        let _ = ratatui::try_restore();
    }
}

/// With a COBOL sysout file: point descriptor 2 at it, keeping the real one for [`stderr_back`].
#[cfg(unix)]
fn stderr_to_sysout() -> std::io::Result<()> {
    let saved = unsafe { libc::dup(2) };
    if saved < 0 {
        return Err(std::io::Error::last_os_error());
    }
    match cobridge::send_to_sysout(2) {
        Ok(true) => {
            SAVED_STDERR.store(saved, Ordering::SeqCst);
            Ok(())
        }
        other => {
            unsafe { libc::close(saved) };
            other.map(|_| ())
        }
    }
}

/// Put the real descriptor 2 back, if it was pointed at the sysout file. Async-signal-safe.
fn stderr_back() {
    #[cfg(unix)]
    {
        let saved = SAVED_STDERR.swap(-1, Ordering::SeqCst);
        if saved >= 0 {
            unsafe {
                libc::dup2(saved, 2);
                libc::close(saved);
            }
        }
    }
}

/// Runs at every process exit. Takes the UI off if it is still on (a normal end has already done so). If a
/// COBOL program is running, libcob is ending the process (runtime error, STOP RUN): say where its
/// message is. Nothing here may panic (no unwinding out of exit).
#[cfg(unix)]
extern "C" fn at_exit() {
    end();
    if cobridge::in_cobol() {
        use std::io::Write;
        let file = cobridge::sysout_path().map(|p| p.display().to_string()).unwrap_or_else(|| "the COBOL sysout file".into());
        let _ = writeln!(std::io::stderr(), "carddemo: the COBOL runtime ended the program (runtime error or STOP RUN); its messages are in {file}");
    }
}

pub type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// The UI's panic hook: take the UI off the terminal (`end`), then the earlier hook prints the panic.
/// Except for a panic the COBOL bridge will catch (`cobridge::note_panic`): the task ends with ASRA while
/// the UI keeps running, so the message goes into the sysout file (`log`) instead of over the screen
/// (failing that, the earlier hook prints it to descriptor 2, which is the sysout file while the UI is on).
/// A bridge panic that turns into an abort is handled like any other panic.
pub fn panic_hook(prev: PanicHook, end: fn(), log: fn(&str) -> bool) -> PanicHook {
    Box::new(move |info| {
        if cobridge::note_panic() {
            let bt = std::backtrace::Backtrace::capture();
            let bt = if bt.status() == std::backtrace::BacktraceStatus::Captured { format!("\n{bt}") } else { String::new() };
            let name = std::thread::current().name().unwrap_or("<unnamed>").to_string();
            if !log(&format!("thread '{name}' {info}{bt}\ncarddemo: the task ends with abend ASRA (or its HANDLE ABEND routine)\n")) {
                prev(info);
            }
            return;
        }
        end();
        prev(info);
    })
}

/// Fatal signals while the UI is on (`kill`, a hangup, a crash): libcob's handler, the default action or
/// Rust's stack-overflow handler end the process without exit handlers, which would leave the terminal in
/// raw mode on the alternate screen. Ours restores it first with async-signal-safe calls only, then passes
/// the signal on to the handler that was there before.
#[cfg(unix)]
mod signals {
    use super::{stderr_back, ACTIVE};
    use std::ffi::c_int;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::OnceLock;

    const SIGNALS: [c_int; 9] =
        [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGABRT, libc::SIGSEGV, libc::SIGBUS, libc::SIGFPE, libc::SIGILL];

    /// The terminal and its settings before raw mode.
    static TTY: AtomicI32 = AtomicI32::new(-1);
    static TERMIOS: OnceLock<libc::termios> = OnceLock::new();
    /// The actions that were there before ours, per entry of SIGNALS.
    static PREV: OnceLock<[libc::sigaction; 9]> = OnceLock::new();

    /// Save the terminal settings and install the handlers (once).
    pub fn install() {
        if PREV.get().is_some() {
            return;
        }
        // the terminal crossterm puts in raw mode: /dev/tty, else stdout
        let tty = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
        let tty = if tty >= 0 { tty } else { 1 };
        let mut t: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(tty, &mut t) } != 0 {
            return; // not a terminal: nothing to restore
        }
        TTY.store(tty, Ordering::SeqCst);
        let _ = TERMIOS.set(t);
        let mut prev: [libc::sigaction; 9] = unsafe { std::mem::zeroed() };
        for (i, &sig) in SIGNALS.iter().enumerate() {
            unsafe { libc::sigaction(sig, std::ptr::null(), &mut prev[i]) };
        }
        let _ = PREV.set(prev);
        for (i, &sig) in SIGNALS.iter().enumerate() {
            if prev[i].sa_sigaction == libc::SIG_IGN {
                continue; // ignored (e.g. SIGHUP under nohup): the process goes on, the UI too
            }
            let mut act: libc::sigaction = unsafe { std::mem::zeroed() };
            act.sa_sigaction = on_signal as extern "C" fn(c_int, *mut libc::siginfo_t, *mut libc::c_void) as usize;
            act.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK; // a stack overflow arrives as SIGSEGV
            unsafe {
                libc::sigemptyset(&mut act.sa_mask);
                libc::sigaction(sig, &act, std::ptr::null_mut());
            }
        }
    }

    /// Faults: the instruction that caused them runs again once the handler returns.
    const FAULTS: [c_int; 4] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGFPE, libc::SIGILL];

    extern "C" fn on_signal(sig: c_int, info: *mut libc::siginfo_t, _ctx: *mut libc::c_void) {
        if ACTIVE.swap(false, Ordering::SeqCst) {
            stderr_back(); // so the next handler's message (libcob's) reaches the terminal
            if let Some(t) = TERMIOS.get() {
                unsafe { libc::tcsetattr(TTY.load(Ordering::SeqCst), libc::TCSANOW, t) };
            }
            let leave = b"\x1b[?25h\x1b[?1049l"; // show the cursor, leave the alternate screen
            unsafe { libc::write(1, leave.as_ptr().cast(), leave.len()) };
        }
        // pass it on to the earlier action. A fault raised by the kernel (si_code > 0) recurs when this
        // handler returns, and reaches that action with its real details (Rust's stack-overflow handler
        // looks at the fault address); a signal that was sent (kill, raise, abort) is sent again.
        if let (Some(prev), Some(i)) = (PREV.get(), SIGNALS.iter().position(|&s| s == sig)) {
            unsafe { libc::sigaction(sig, &prev[i], std::ptr::null_mut()) };
            let kernel_fault = FAULTS.contains(&sig) && !info.is_null() && unsafe { (*info).si_code } > 0;
            if !kernel_fault {
                unsafe { libc::raise(sig) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // The only test in this binary: it replaces the process's panic hook.
    use super::*;
    use cics::{Aid, Session};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    static ENDS: AtomicUsize = AtomicUsize::new(0);
    static PRINTS: AtomicUsize = AtomicUsize::new(0);
    static LOGGED: Mutex<String> = Mutex::new(String::new());

    fn count_end() {
        ENDS.fetch_add(1, Ordering::SeqCst);
    }

    fn keep_log(text: &str) -> bool {
        LOGGED.lock().unwrap_or_else(|e| e.into_inner()).push_str(text);
        true
    }

    #[test]
    fn panic_hook_keeps_the_ui_for_bridge_panics_only() {
        let saved = std::panic::take_hook();
        // `end`, `log` and the earlier hook only record, so nothing touches the terminal or prints
        std::panic::set_hook(panic_hook(
            Box::new(|_| {
                PRINTS.fetch_add(1, Ordering::SeqCst);
            }),
            count_end,
            keep_log,
        ));
        let simulated = cobridge::as_cdcics_call(|| panic!("bridge bug"));
        let in_bridge_after = cobridge::in_cdcics();
        let rust = std::panic::catch_unwind(|| panic!("rust bug"));
        // the real path, through the bridge's CDCICS entry point (needs the COBOL modules)
        let real = cobridge::check().is_ok().then(|| {
            let r = cobridge::region(batch::repro::sample_store().unwrap());
            let mut s = Session::default();
            cobridge::panic_in_next_command();
            r.run(&mut s, Some("CC00"), Aid::Enter);
            s.screen.text.unwrap_or_default()
        });
        std::panic::set_hook(saved);

        assert_eq!(simulated, None::<()>, "the bridge catches its panic");
        assert!(!in_bridge_after, "in_cdcics() is false again after a bridge panic");
        assert!(rust.is_err());
        assert_eq!(ENDS.load(Ordering::SeqCst), 1, "UI taken off for the Rust panic only");
        assert_eq!(PRINTS.load(Ordering::SeqCst), 1, "only the Rust panic printed over the screen");
        let logged = LOGGED.lock().unwrap().clone();
        assert!(logged.contains("bridge bug"), "bridge panic logged: {logged:?}");
        match real {
            Some(text) => {
                assert!(text.contains("failed with abend ASRA"), "screen: {text:?}");
                assert!(logged.contains("test-hooks: EXEC CICS command panics"), "real bridge panic logged: {logged:?}");
                assert!(!cobridge::in_cobol(), "no COBOL program running afterwards");
            }
            None => eprintln!("COBOL modules not built: real ASRA path not tested"),
        }
    }
}
