#!/bin/bash
# Install the external tools this repo's tests shell out to. The canonical
# ci.yml runs this in every repo right after `cargo build`; a repo whose
# tests need no external tools keeps this script as an explicit no-op.
# Keep it strict: a tool that fails to install must fail the build here,
# not surface later as a confusing test failure.
set -euo pipefail

# Nothing to install: this repo's tests need no tools beyond the runner image.
