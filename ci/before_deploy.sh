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
    local staging="$td/$name"
    mkdir -p "$staging/complete"
    local out_dir="$(pwd)/deployment"
    mkdir -p "$out_dir"

    # Copy the ripgrep binary and strip it.
    cp target/$TARGET/release/rg "$staging/rg"
    "${gcc_prefix}strip" "$staging/rg"
    # Copy the README and licenses.
    cp {README.md,UNLICENSE,COPYING,LICENSE-MIT} "$staging/"
    # Copy shell completion files.
    cp \
      target/"$TARGET"/release/build/ripgrep-*/out/{rg.bash,rg.fish,_rg.ps1} \
      "$staging/complete/"
    cp complete/_rg "$td/$name/complete/"
    # Copy man page.
    cp \
      target/"$TARGET"/release/build/ripgrep-*/out/rg.1 \
      "$td/$name/"

    (cd "$td" && tar czf "$out_dir/$name.tar.gz" *)
    rm -rf "$td"
}

main() {
    mk_artifacts
    mk_tarball
}

main
