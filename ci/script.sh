#!/bin/bash

# build, test and generate docs in this phase

set -ex

. "$(dirname $0)/utils.sh"

main() {
    CARGO="$(builder)"

    # Test a normal debug build.
    if is_arm; then
        "$CARGO" build --target "$TARGET" --verbose
    else
        "$CARGO" build --target "$TARGET" --verbose --all --features 'pcre2'
    fi

    # Show the output of the most recent build.rs stderr.
    set +x
    stderr="$(find "target/$TARGET/debug" -name stderr -print0 | xargs -0 ls -t | head -n1)"
    if [ -s "$stderr" ]; then
      echo "===== $stderr ====="
      cat "$stderr"
      echo "====="
    fi
    set -x

    # sanity check the file type
    file target/"$TARGET"/debug/rg

    # Check that we've generated man page and other shell completions.
    outdir="$(cargo_out_dir "target/$TARGET/debug")"
    file "$outdir/rg.bash"
    file "$outdir/rg.fish"
    file "$outdir/_rg.ps1"
    file "$outdir/rg.1"

    # Apparently tests don't work on arm, so just bail now. I guess we provide
    # ARM releases on a best effort basis?
    if is_arm; then
      return 0
    fi

    # Test that zsh completions are in sync with ripgrep's actual args.
    "$(dirname "${0}")/test_complete.sh"

    # Run tests for ripgrep and all sub-crates.
    "$CARGO" test --target "$TARGET" --verbose --all --features 'pcre2'

    # Check formatting.
    if [ "$TRAVIS_RUST_VERSION" = "1.41.0" ]; then
      rustup component add rustfmt
      "$CARGO" fmt --all -- --check
    fi
}

main
