//! A panic inside an EXEC CICS call must not reach an earlier panic hook (in the TUI ratatui's, which
//! restores the terminal), whichever was installed first; panics elsewhere still must. A test binary of
//! its own: it replaces the process's panic hook.

use std::sync::atomic::{AtomicUsize, Ordering};

static EARLIER_HOOK_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Stands in for ratatui::init(): wraps the current hook and counts its own runs.
fn install_tui_like_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        EARLIER_HOOK_RUNS.fetch_add(1, Ordering::SeqCst);
        prev(info);
    }));
}

#[test]
fn bridge_panics_skip_the_tui_hook_in_either_order() {
    // carddemo online --cobol: the first program call (installs the bridge hook) runs before
    // ratatui::init(), which wraps it; the TUI then installs the bridge hook again
    cobridge::install_panic_hook();
    install_tui_like_hook();
    cobridge::install_panic_hook();

    assert_eq!(cobridge::guarded_for_test(|| panic!("bridge bug")), None, "panic caught");
    assert_eq!(EARLIER_HOOK_RUNS.load(Ordering::SeqCst), 0, "TUI hook not run for a bridge panic");
    assert!(!cobridge::in_cdcics(), "flag cleared after the panic");
    assert_eq!(cobridge::guarded_for_test(|| 7), Some(7));

    // a panic outside a CDCICS call still reaches the TUI hook
    let _ = std::panic::catch_unwind(|| panic!("rust bug"));
    assert_eq!(EARLIER_HOOK_RUNS.load(Ordering::SeqCst), 1, "TUI hook run for other panics");
}
