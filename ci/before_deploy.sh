#!/bin/bash

# package the build artifacts

set -ex

. "$(dirname $0)/utils.sh"

# Generate artifacts for release
mk_artifacts() {
    if is_ssse3_target; then
        RUSTFLAGS="-C target-feature=+ssse3" \
        cargo build --target "$TARGET" --release --features simd-accel
    else
        cargo build --target "$TARGET" --release
    fi
}

mk_tarball() {
    local gcc_prefix="$(gcc_prefix)"
    local td="$(mktemp -d)"
    local name="${PROJECT_NAME}-${TRAVIS_TAG}-${TARGET}"
    mkdir -p "$td/$name/complete"
    mkdir deployment
    local out_dir="$(pwd)/deployment"

    cp target/$TARGET/release/rg "$td/$name/rg"
    "${gcc_prefix}strip" "$td/$name/rg"
    cp {doc/rg.1,README.md,UNLICENSE,COPYING,LICENSE-MIT} "$td/$name/"
    cp \
      target/"$TARGET"/release/build/ripgrep-*/out/{rg.bash,rg.fish,_rg.ps1} \
      "$td/$name/complete/"
    cp complete/_rg "$td/$name/complete/"

    (cd "$td" && tar czf "$out_dir/$name.tar.gz" *)
    rm -rf "$td"
}

main() {
    mk_artifacts
    mk_tarball
}

main
