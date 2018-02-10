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
    # When cross-compiling, use the right `strip` tool on the binary.
    local gcc_prefix="$(gcc_prefix)"
    # Create a temporary dir that contains our staging area.
    # $tmpdir/$name is what eventually ends up as the deployed archive.
    local tmpdir="$(mktemp -d)"
    local name="${PROJECT_NAME}-${TRAVIS_TAG}-${TARGET}"
    local staging="$tmpdir/$name"
    mkdir -p "$staging"/{complete,doc}
    # The deployment directory is where the final archive will reside.
    # This path is known by the .travis.yml configuration.
    local out_dir="$(pwd)/deployment"
    mkdir -p "$out_dir"
    # Find the correct (most recent) Cargo "out" directory. The out directory
    # contains shell completion files and the man page.
    local cargo_out_dir="$(cargo_out_dir "target/$TARGET")"

    # Copy the ripgrep binary and strip it.
    cp "target/$TARGET/release/rg" "$staging/rg"
    "${gcc_prefix}strip" "$staging/rg"
    # Copy the licenses and README.
    cp {README.md,UNLICENSE,COPYING,LICENSE-MIT} "$staging/"
    # Copy documentation and man page.
    cp {CHANGELOG.md,FAQ.md,GUIDE.md} "$staging/doc/"
    if command -V a2x 2>&1 > /dev/null; then
      # The man page should only exist if we have asciidoc installed.
      cp "$cargo_out_dir/rg.1" "$staging/doc/"
    fi
    # Copy shell completion files.
    cp "$cargo_out_dir"/{rg.bash,rg.fish,_rg.ps1} "$staging/complete/"
    cp complete/_rg "$staging/complete/"

    (cd "$tmpdir" && tar czf "$out_dir/$name.tar.gz" "$name")
    rm -rf "$tmpdir"
}

main() {
    mk_artifacts
    mk_tarball
}

main
