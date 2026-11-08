//! The terminal while the full-screen UI is on: raw mode and the alternate screen, set up as
//! `ratatui::try_init()` does but with this program's own panic hook, and taken off again on every way out:
//! the normal end or an error ([`UiOn`]), a panic ([`panic_hook`]), a process exit, e.g. libcob ending the
//! process from inside a COBOL program ([`at_exit`]), and a signal whose default action ends the process
//! (`signals`; real-time signals and SIGKILL excepted). With `--cobol`, descriptor 2 points at the COBOL
//! sysout file while the UI is on: libcob writes a few messages straight to it, and they must not land on
//! the screen.

use anyhow::Result;
use ratatui::prelude::*;
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, AtomicPtr};
use std::sync::atomic::{AtomicU8, Ordering};

/// The UI's state: off, on, or being taken off by [`end`]. A signal handler restores the terminal in the
/// last two states; "on" is set before raw mode goes on and "off" only after the terminal is restored,
/// so a signal in between still finds the terminal to restore.
static STATE: AtomicU8 = AtomicU8::new(OFF);
const OFF: u8 = 0;
const ON: u8 = 1;
const ENDING: u8 = 2;

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
    signals::install(); // before raw mode (it saves the terminal settings), and after libcob's handlers
    let on = UiOn;
    put_on()?;
    Ok((on, Terminal::new(CrosstermBackend::new(std::io::stdout()))?))
}

/// Raw mode, descriptor 2 on the sysout file (`--cobol`), the alternate screen.
fn put_on() -> Result<()> {
    use crossterm::{execute, terminal};
    STATE.store(ON, Ordering::SeqCst);
    terminal::enable_raw_mode()?;
    #[cfg(unix)]
    stderr_to_sysout()?;
    execute!(std::io::stdout(), terminal::EnterAlternateScreen)?;
    Ok(())
}

/// Put the UI back if it was taken off while the program goes on. That happens only if the panic hook
/// misjudged a panic the COBOL bridge then caught (see `cobridge::note_panic`): the task ended with ASRA,
/// and the session continues on a restored screen instead of a terminal in cooked mode. Called before
/// every screen: a bridge panic noted earlier has been caught by now, so no abort is pending for it.
pub fn ensure_on(term: &mut ratatui::DefaultTerminal) -> Result<()> {
    #[cfg(unix)]
    abort_note::clear();
    if STATE.load(Ordering::SeqCst) == OFF {
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
    if STATE.compare_exchange(ON, ENDING, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
        stderr_back();
        let _ = ratatui::try_restore();
        STATE.store(OFF, Ordering::SeqCst);
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
        let _ = writeln!(
            std::io::stderr(),
            "carddemo: the COBOL runtime ended the program (runtime error or STOP RUN); its messages are in {file}"
        );
    }
}

pub type PanicHook = Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;

/// The UI's panic hook: take the UI off the terminal (`end`), then the earlier hook prints the panic.
/// Except for a panic the COBOL bridge will catch (`cobridge::note_panic`): the task ends with ASRA while
/// the UI keeps running, so the message goes into the sysout file (`log`) instead of over the screen
/// (failing that, the earlier hook prints it to descriptor 2, which is the sysout file while the UI is on).
/// A second panic in flight means an abort: handled like any other panic. A first panic that cannot unwind
/// (std's debug-build precondition checks) also aborts, and stable Rust cannot tell it apart here: for that
/// case the hook leaves a note, which the SIGABRT handler prints once the terminal is restored.
pub fn panic_hook(prev: PanicHook, end: fn(), log: fn(&str) -> bool) -> PanicHook {
    Box::new(move |info| {
        if cobridge::note_panic() {
            let bt = std::backtrace::Backtrace::capture();
            let bt = if bt.status() == std::backtrace::BacktraceStatus::Captured { format!("\n{bt}") } else { String::new() };
            let name = std::thread::current().name().unwrap_or("<unnamed>").to_string();
            let note = "the bridge ends the task with abend ASRA (or its HANDLE ABEND routine), unless this panic cannot be caught";
            let logged = log(&format!("thread '{name}' {info}{bt}\ncarddemo: {note}\n"));
            #[cfg(unix)]
            abort_note::set(&format!(
                "carddemo: a panic inside an EXEC CICS command could not be caught and ended the program{}\n",
                cobridge::sysout_path().filter(|_| logged).map(|p| format!("; its message is in {}", p.display())).unwrap_or_default()
            ));
            if !logged {
                prev(info);
            }
            return;
        }
        end();
        prev(info);
    })
}

/// A message for the SIGABRT handler, set by the panic hook for a bridge panic that it expects to be caught
/// (if the process aborts instead, the user learns why), cleared once the panic has been caught.
#[cfg(unix)]
mod abort_note {
    use super::AtomicPtr;
    use std::sync::atomic::Ordering;

    static NOTE: AtomicPtr<Vec<u8>> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set(text: &str) {
        let old = NOTE.swap(Box::into_raw(Box::new(text.as_bytes().to_vec())), Ordering::SeqCst);
        if !old.is_null() {
            drop(unsafe { Box::from_raw(old) });
        }
    }

    pub fn clear() {
        let old = NOTE.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if !old.is_null() {
            drop(unsafe { Box::from_raw(old) });
        }
    }

    /// Write the note to descriptor 2, if there is one. Async-signal-safe (the note is never freed here).
    pub fn write() {
        let p = NOTE.load(Ordering::SeqCst);
        if !p.is_null() {
            let v = unsafe { &*p };
            unsafe { libc::write(2, v.as_ptr().cast(), v.len()) };
        }
    }
}

/// Signals whose default action ends the process (`kill`, a hangup, a crash, a CPU limit): libcob's
/// handler, the default action or Rust's stack-overflow handler end the process without exit handlers,
/// which would leave the terminal in raw mode on the alternate screen. Ours restores it first with
/// async-signal-safe calls only, then passes the signal on to the handler that was there before.
#[cfg(unix)]
mod signals {
    use super::{abort_note, stderr_back, ENDING, ON, STATE};
    use std::ffi::c_int;
    use std::cell::UnsafeCell;
    use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
    use std::sync::OnceLock;

    /// Every signal whose default action ends the process, except SIGKILL and SIGSTOP (no handler) and
    /// the real-time signals.
    const SIGNALS: [c_int; 19] = [
        libc::SIGHUP,
        libc::SIGINT,
        libc::SIGQUIT,
        libc::SIGILL,
        libc::SIGTRAP,
        libc::SIGABRT,
        libc::SIGBUS,
        libc::SIGFPE,
        libc::SIGUSR1,
        libc::SIGSEGV,
        libc::SIGUSR2,
        libc::SIGPIPE,
        libc::SIGALRM,
        libc::SIGTERM,
        libc::SIGXCPU,
        libc::SIGXFSZ,
        libc::SIGVTALRM,
        libc::SIGPROF,
        libc::SIGSYS,
    ];

    /// Faults: the instruction that caused them runs again once the handler returns.
    const FAULTS: [c_int; 4] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGFPE, libc::SIGILL];

    /// The terminal crossterm puts in raw mode, and its settings before raw mode.
    static TTY: AtomicI32 = AtomicI32::new(-1);
    static TERMIOS: OnceLock<libc::termios> = OnceLock::new();

    /// The actions that were there before ours, per entry of SIGNALS: written by `install` only while
    /// those signals are blocked, read by `on_signal`.
    struct PrevActions(UnsafeCell<[libc::sigaction; 19]>);
    unsafe impl Sync for PrevActions {}
    static PREV: PrevActions = PrevActions(UnsafeCell::new(unsafe { std::mem::zeroed() }));
    /// Bit i set: PREV holds the earlier action of SIGNALS[i] (ours is installed for it).
    static HAVE_PREV: AtomicU32 = AtomicU32::new(0);

    fn ours() -> libc::sighandler_t {
        on_signal as extern "C" fn(c_int, *mut libc::siginfo_t, *mut libc::c_void) as libc::sighandler_t
    }

    /// Save the terminal settings (the first time) and put our handlers in front of whatever handles each
    /// signal now. Called on every start of the UI: libcob's initialisation installs its own handlers
    /// over ours, and the demo menu may start a COBOL session after a Rust one in the same process.
    pub fn install() {
        if TERMIOS.get().is_none() {
            // the terminal crossterm puts in raw mode (crossterm's tty_fd): stdin if it is one, else /dev/tty
            let tty = if unsafe { libc::isatty(0) } == 1 {
                0
            } else {
                unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) }
            };
            let mut t: libc::termios = unsafe { std::mem::zeroed() };
            if tty < 0 || unsafe { libc::tcgetattr(tty, &mut t) } != 0 {
                if tty > 0 {
                    unsafe { libc::close(tty) };
                }
                return; // no terminal: crossterm cannot use raw mode either, nothing to restore
            }
            TTY.store(tty, Ordering::SeqCst);
            let _ = TERMIOS.set(t);
        }
        // no signal may arrive while PREV changes
        let (mut block, mut old): (libc::sigset_t, libc::sigset_t) = unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
        unsafe {
            libc::sigemptyset(&mut block);
            for &sig in &SIGNALS {
                libc::sigaddset(&mut block, sig);
            }
            libc::pthread_sigmask(libc::SIG_BLOCK, &block, &mut old);
        }
        for (i, &sig) in SIGNALS.iter().enumerate() {
            let mut cur: libc::sigaction = unsafe { std::mem::zeroed() };
            unsafe { libc::sigaction(sig, std::ptr::null(), &mut cur) };
            if cur.sa_sigaction == ours() || cur.sa_sigaction == libc::SIG_IGN {
                continue; // ours already; or ignored (SIGHUP under nohup, SIGPIPE in Rust): the UI goes on
            }
            unsafe { (*PREV.0.get())[i] = cur };
            HAVE_PREV.fetch_or(1 << i, Ordering::SeqCst);
            let mut act: libc::sigaction = unsafe { std::mem::zeroed() };
            act.sa_sigaction = ours();
            act.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK; // a stack overflow arrives as SIGSEGV
            unsafe {
                libc::sigemptyset(&mut act.sa_mask);
                libc::sigaction(sig, &act, std::ptr::null_mut());
            }
        }
        unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &old, std::ptr::null_mut()) };
    }

    extern "C" fn on_signal(sig: c_int, info: *mut libc::siginfo_t, _ctx: *mut libc::c_void) {
        let was = STATE.swap(super::OFF, Ordering::SeqCst);
        if was == ON || was == ENDING {
            stderr_back(); // so the next handler's message (libcob's, Rust's) reaches the terminal
            if let Some(t) = TERMIOS.get() {
                unsafe { libc::tcsetattr(TTY.load(Ordering::SeqCst), libc::TCSANOW, t) };
            }
            let leave = b"\x1b[?25h\x1b[?1049l"; // show the cursor, leave the alternate screen
            unsafe { libc::write(1, leave.as_ptr().cast(), leave.len()) };
        }
        if sig == libc::SIGABRT {
            abort_note::write();
        }
        // pass it on to the earlier action. A fault raised by the kernel (si_code > 0) recurs when this
        // handler returns, and reaches that action with its real details (Rust's stack-overflow handler
        // looks at the fault address); a signal that was sent (kill, raise, abort) is sent again.
        if let Some(i) = SIGNALS.iter().position(|&s| s == sig).filter(|&i| HAVE_PREV.load(Ordering::SeqCst) & (1 << i) != 0) {
            unsafe { libc::sigaction(sig, &(*PREV.0.get())[i], std::ptr::null_mut()) };
            let kernel_fault = FAULTS.contains(&sig) && !info.is_null() && unsafe { (*info).si_code } > 0;
            if kernel_fault {
                // the next handler may abort while it is on this same signal stack (Rust's does, after reporting
                // a stack overflow), where a second signal frame may not fit, and ours has nothing left to do
                unsafe { libc::signal(libc::SIGABRT, libc::SIG_DFL) };
            } else {
                unsafe { libc::raise(sig) };
            }
        }
    }
}

#[cfg(all(test, debug_assertions))] // the bridge's test seams exist in debug builds only
mod tests {
    // The only test in this binary that replaces the process's panic hook (the others must not panic).
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
                assert!(logged.contains("test seam: EXEC CICS command panics"), "real bridge panic logged: {logged:?}");
                assert!(!cobridge::in_cobol(), "no COBOL program running afterwards");
            }
            None => eprintln!("COBOL modules not built: real ASRA path not tested"),
        }
    }
}
