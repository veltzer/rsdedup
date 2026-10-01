#!/bin/bash
# Install the system libraries this repo's build links against and the
# external tools its tests shell out to. The canonical ci.yml runs this in
# every repo before `cargo build`; a repo that needs neither keeps this
# script as an explicit no-op. Keep it strict: anything that fails to
# install must fail the build here, not surface later as a confusing
# build or test failure.
set -euo pipefail

# The cargo subcommands the workflow's later steps run (cargo deny, cargo
# nextest) are declared under [dependencies] cargo in rsconstruct.toml, the
# fleet's one list of them; nothing here or in ci.yml names a crate. The
# rsconstruct release binary that reads the list is downloaded rather than
# built (a 20 MB fetch against minutes of compile) into cargo's bin dir,
# which is on PATH and which rust-cache carries between runs together with
# the crates install-deps puts there.
bin="${CARGO_HOME:-${HOME}/.cargo}/bin"
curl -fsSL https://github.com/veltzer/rsconstruct/releases/latest/download/rsconstruct-linux-x86_64 -o "${bin}/rsconstruct"
chmod +x "${bin}/rsconstruct"
rsconstruct tools install-deps
