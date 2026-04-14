#!/usr/bin/env bash

set -e

echo "::group::Setting up"
export REPO_MANIFEST="master.xml"
export MANIFEST_URL="https://github.com/seL4/sel4bench-manifest.git"
checkout-manifest.sh

fetch-branches.sh
echo "::endgroup::"

# start test
python3 /builds/build.py
