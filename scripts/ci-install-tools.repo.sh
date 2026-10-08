#!/bin/bash
# This repo's own CI installs: the system libraries the crate links against
# and the external tools its tests shell out to. The fleet-shared
# scripts/ci-install-tools.sh runs this in every rs* repo, in the test job
# (TARGET unset) and in the release build job (TARGET names the release
# triple), after it has put the rsconstruct release binary on PATH and before
# it installs the tools rsconstruct.toml declares. Keep it strict: anything
# that fails to install must fail the build here.
set -euo pipefail

# This crate links no system library and its tests shell out to nothing the
# shared setup lacks, so there is nothing to install. The file stays as an
# explicit no-op: the shared script requires it, a missing one fails the step.
