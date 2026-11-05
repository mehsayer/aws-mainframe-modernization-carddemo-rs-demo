#!/bin/sh
# Build docs/requirements.pdf from the markdown sources.
cd "$(dirname "$0")/requirements" && pandoc 00-intro.md 10-data-dictionary.md 20-online-common-security.md \
  21-online-accounts-cards.md 22-online-transactions.md 30-batch.md 40-nonfunctional.md 90-traceability.md \
  --pdf-engine=xelatex --columns=200 -o ../requirements.pdf
