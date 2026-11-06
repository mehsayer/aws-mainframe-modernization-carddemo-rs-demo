#!/bin/sh
# Regenerate crates/cobol/src/copybooks.rs from the original copybooks.
cd "$(dirname "$0")/../../cobol/app/cpy" && python3 ../../../rust/tools/gen_records.py ../../../rust/crates/cobol/src/copybooks.rs \
  CSUSR01Y.cpy CVACT01Y.cpy CVACT02Y.cpy CVACT03Y.cpy CVCUS01Y.cpy CVTRA01Y.cpy CVTRA02Y.cpy CVTRA03Y.cpy \
  CVTRA04Y.cpy CVTRA05Y.cpy CVTRA06Y.cpy CVTRA07Y.cpy COSTM01.CPY CVEXPORT.cpy COCOM01Y.cpy
