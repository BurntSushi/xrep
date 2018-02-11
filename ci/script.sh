#!/bin/bash

# build, test and generate docs in this phase

set -ex

. "$(dirname $0)/utils.sh"

main() {
    # Test a normal debug build.
    cargo build --target "$TARGET" --verbose --all

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

    # Apparently tests don't work on arm, so just bail now. I guess we provide
    # ARM releases on a best effort basis?
    if is_arm; then
      return 0
    fi

    # Test that zsh completions are in sync with ripgrep's actual args.
    "$(dirname "${0}")/test_complete.sh"

    # Check that we've generated man page and other shell completions.
    outdir="$(cargo_out_dir "target/$TARGET/debug")"
    file "$outdir/rg.bash"
    file "$outdir/rg.fish"
    file "$outdir/_rg.ps1"
    # N.B. man page isn't generated on ARM cross-compile, but we gave up
    # long before this anyway.
    file "$outdir/rg.1"

    # Run tests for ripgrep and all sub-crates.
    cargo test --target "$TARGET" --verbose --all
}

main
