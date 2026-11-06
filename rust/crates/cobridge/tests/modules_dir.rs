//! modules_dir() reads CARDDEMO_COBOL_MODULES: a test binary of its own, so changing the variable cannot
//! affect other tests running at the same time.

#[test]
fn modules_dir_rule() {
    let saved = std::env::var_os("CARDDEMO_COBOL_MODULES");
    // an empty value means "not set", like the Python side
    std::env::set_var("CARDDEMO_COBOL_MODULES", "");
    assert!(cobridge::modules_dir().ends_with("cobridge/build/modules"));
    std::env::set_var("CARDDEMO_COBOL_MODULES", "rel/mods");
    let d = cobridge::modules_dir();
    assert!(d.is_absolute() && d.ends_with("rel/mods"));
    match saved {
        Some(v) => std::env::set_var("CARDDEMO_COBOL_MODULES", v),
        None => std::env::remove_var("CARDDEMO_COBOL_MODULES"),
    }
}
