#!/bin/sh
# Rebuild all PDF deliverables.
set -e
cd "$(dirname "$0")"
./build-requirements.sh
python3 tests/build.py
pandoc deck/progress.md -t beamer --pdf-engine=xelatex --columns=200 -o progress-presentation.pdf
ls -la requirements.pdf manual-test-suite.pdf progress-presentation.pdf
