#!/bin/sh
# Rebuild all PDF deliverables.
set -e
cd "$(dirname "$0")"
./build-requirements.sh
python3 tests/build.py
pandoc deck/progress.md -t beamer --slide-level=2 --pdf-engine=xelatex --columns=60 -o progress-presentation.pdf
pandoc demo-quickstart.md --pdf-engine=xelatex -o demo-quickstart.pdf
pandoc architecture/cobol.md --pdf-engine=xelatex -o architecture-cobol.pdf
pandoc architecture/rust.md --pdf-engine=xelatex -o architecture-rust.pdf
ls -la requirements.pdf manual-test-suite.pdf progress-presentation.pdf demo-quickstart.pdf architecture-cobol.pdf architecture-rust.pdf
