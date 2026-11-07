//! Windows builds: the program's icon (`assets/carddemo.ico`, drawn in `assets/carddemo.svg`) and its version
//! information (Explorer's Properties > Details), compiled by MinGW's `windres` into a resource object that is
//! linked into `carddemo.exe`. Other targets need nothing; without `windres` the build goes on with a warning.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/carddemo.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if var("CARGO_CFG_TARGET_OS") != "windows" {
        return;
    }
    if var("CARGO_CFG_TARGET_ENV") != "gnu" {
        println!("cargo:warning=no icon: the Windows resources are compiled with MinGW's windres (target *-windows-gnu)");
        return;
    }
    let out = PathBuf::from(var("OUT_DIR"));
    let icon = PathBuf::from(var("CARGO_MANIFEST_DIR")).join("assets").join("carddemo.ico");
    let version = var("CARGO_PKG_VERSION");
    let mut parts: Vec<u32> = version.split(['.', '-']).filter_map(|p| p.parse().ok()).take(3).collect();
    parts.resize(4, 0);
    let numeric = parts.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let rc = out.join("carddemo.rc");
    std::fs::write(
        &rc,
        format!(
            r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Illia Burov"
      VALUE "FileDescription", "CardDemo: the AWS mainframe sample, translated from COBOL to Rust"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "carddemo"
      VALUE "LegalCopyright", "Apache License 2.0; the original CardDemo (c) Amazon.com, Inc. or its affiliates"
      VALUE "OriginalFilename", "carddemo.exe"
      VALUE "ProductName", "CardDemo"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
            icon = icon.display().to_string().replace('\\', "/"),
        ),
    )
    .expect("writing the resource script");
    // the cross compiler's windres (x86_64-w64-mingw32-windres), or `windres` on a native MinGW toolchain
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| {
        let cross = format!("{}-w64-mingw32-windres", var("CARGO_CFG_TARGET_ARCH"));
        if Command::new(&cross).arg("--version").output().is_ok() { cross } else { "windres".into() }
    });
    let obj = out.join("carddemo-resources.o");
    match Command::new(&windres).arg("-i").arg(&rc).arg("-O").arg("coff").arg("-o").arg(&obj).status() {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        r => println!("cargo:warning=no icon: {windres} failed ({r:?}); the program is built without it"),
    }
}
