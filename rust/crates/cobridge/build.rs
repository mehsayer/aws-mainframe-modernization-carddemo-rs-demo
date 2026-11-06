fn main() {
    // GnuCOBOL runtime (libcob); `cob-config --libs` gives the location when it is not in a system path
    if let Ok(out) = std::process::Command::new("cob-config").arg("--libs").output() {
        for tok in String::from_utf8_lossy(&out.stdout).split_whitespace() {
            if let Some(dir) = tok.strip_prefix("-L") {
                println!("cargo:rustc-link-search=native={dir}");
            }
        }
    }
    println!("cargo:rustc-link-lib=dylib=cob");
}
