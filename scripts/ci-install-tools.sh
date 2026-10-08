#!/bin/bash
# Install what ci.yml's Build step (`rsconstruct build`) needs. This file is
# byte-identical across all rs* repos (rsmultigit check same): everything a
# crate needs beyond the shared setup - the system libraries its build links
# against, the external tools its tests shell out to - lives in
# scripts/ci-install-tools.repo.sh, which this script runs in every repo.
# Keep both strict: anything that fails to install must fail the build here,
# not surface later as a confusing build or test failure.
set -euo pipefail

bin="${CARGO_HOME:-${HOME}/.cargo}/bin"

# ci.yml's Build step is `rsconstruct build`, so rsconstruct is the one tool
# every repo installs. The release binary is downloaded rather than built (a
# 20 MB fetch against minutes of compile) into cargo's bin dir, which is on
# PATH and which rust-cache carries between runs together with everything
# the two install commands below put there.
#
# Test job only. TARGET is set by ci.yml's build job, which runs nothing but
# `cargo build --release`: there the crates would be compiled for no caller,
# once per release target, on a per-target cache that never holds them -
# an hour per Linux release job when this did run there (run 36876289729).
if [[ -z "${TARGET:-}" ]]; then
	curl -fsSL https://github.com/veltzer/rsconstruct/releases/latest/download/rsconstruct-linux-x86_64 -o "${bin}/rsconstruct"
	chmod +x "${bin}/rsconstruct"
fi

# The repo's own installs. It runs in both jobs (a cross build needs its
# foreign libraries too, and the hook reads TARGET to tell them apart), after
# the download so a repo may put its own binary on PATH in place of the
# release (rsconstruct itself installs the one the checkout builds), and
# before the install commands below so what they look for is in place. The
# file is required: a repo with nothing to add keeps an explicit no-op, and
# a missing one fails here rather than silently skipping its setup.
./scripts/ci-install-tools.repo.sh

# `tool install-deps` installs the cargo subcommands rsconstruct.toml declares
# under [dependencies] cargo (cargo deny, cargo nextest); `tool install`
# installs the external tools of its enabled processors (rumdl, ...). Nothing
# here or in ci.yml names a tool: rsconstruct.toml is the fleet's one list of
# them. Test job only, like the download above.
if [[ -z "${TARGET:-}" ]]; then
	rsconstruct tool install-deps
	rsconstruct tool install
fi
