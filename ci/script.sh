#!/bin/bash

# build, test and generate docs in this phase

set -ex

. "$(dirname $0)/utils.sh"

main() {
    # disable_cross_doctests
    cargo build --target "${TARGET}" --verbose --all
    if [ "$(architecture)" = "amd64" ] || [ "$(architecture)" = "i386" ]; then
        cargo test --target "${TARGET}" --verbose --all
        "$( dirname "${0}" )/test_complete.sh"
    fi
    # sanity check the file type
    file target/$TARGET/debug/rg
}

main
