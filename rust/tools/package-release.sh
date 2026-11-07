#!/bin/sh
# Build the downloadable demo packages of a release (docs/demo-quickstart.md):
#   rust/target/packages/carddemo-<version>-windows-x86_64.zip     carddemo.exe + sample data (double click)
#   rust/target/packages/carddemo-<version>-linux-x86_64.tar.gz    carddemo + sample data + COBOL modules
# Needs: the MinGW-w64 cross compiler (x86_64-w64-mingw32-gcc), zip, GnuCOBOL (for the COBOL modules),
# pandoc + xelatex (the quick-start PDF).
set -eu
cd "$(dirname "$0")/.."
RS=$(pwd)
ROOT=$(cd .. && pwd)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
OUT=$RS/target/packages
rm -rf "$OUT"
mkdir -p "$OUT"

echo "== building (version $VERSION)"
cargo build --release -p carddemo
CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc AR_x86_64_pc_windows_gnu=x86_64-w64-mingw32-ar \
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build --release -p carddemo --target x86_64-pc-windows-gnu
python3 cobridge/cicsprep.py >/dev/null
(cd "$ROOT/docs" && pandoc demo-quickstart.md --pdf-engine=xelatex -o demo-quickstart.pdf)

readme() { # $1: platform
  cat <<EOF
CardDemo $VERSION - the AWS mainframe sample application, translated from COBOL/CICS to Rust

Start: $( [ "$1" = windows ] && echo 'double-click carddemo.exe (unpack the whole zip first)' || echo './carddemo in a terminal' )
A menu offers the online screens (sign on with USER0001 / PASSWORD), the nightly batch, the
transaction report and a viewer for the output files. Instructions: demo-quickstart.pdf.

Project: https://github.com/mehsayer/aws-mainframe-modernization-carddemo-rs-demo
License: Apache-2.0 (LICENSE, NOTICE)
EOF
}

stage() { # $1: platform, $2: program file
  d="$OUT/carddemo-$VERSION-$1-x86_64"
  mkdir -p "$d/data/EBCDIC"
  cp "$2" "$d/"
  cp "$ROOT"/cobol/app/data/EBCDIC/* "$d/data/EBCDIC/"
  cp "$ROOT/docs/demo-quickstart.pdf" "$ROOT/LICENSE" "$ROOT/NOTICE" "$d/"
  echo "$d"
}

echo "== windows"
d=$(stage windows "$RS/target/x86_64-pc-windows-gnu/release/carddemo.exe")
readme windows | sed 's/$/\r/' > "$d/README.txt"
for f in LICENSE NOTICE; do sed -i 's/$/\r/' "$d/$f"; mv "$d/$f" "$d/$f.txt"; done
(cd "$OUT" && zip -qr "$(basename "$d").zip" "$(basename "$d")")

echo "== linux"
d=$(stage linux "$RS/target/release/carddemo")
mkdir -p "$d/cobol-modules"
cp "$RS"/cobridge/build/modules/*.so "$d/cobol-modules/"
readme linux > "$d/README.txt"
(cd "$OUT" && tar czf "$(basename "$d").tar.gz" "$(basename "$d")")

ls -la "$OUT"/*.zip "$OUT"/*.tar.gz
