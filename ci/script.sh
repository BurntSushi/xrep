#!/bin/bash

# build, test and generate docs in this phase

set -ex

. "$(dirname $0)/utils.sh"

main() {
    CARGO="$(builder)"

    if is_arm; then
        export CC="$(gcc_full_name)"
        export QEMU_LD_PREFIX=/usr/arm-linux-gnueabihf
    fi

    # Test a normal debug build.
    "$CARGO" build --target "$TARGET" --verbose --all --features 'pcre2'

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

    # Does not find "zsh" in the ARM matrix...
    if ! is_arm; then
        # Test that zsh completions are in sync with ripgrep's actual args.
        "$(dirname "${0}")/test_complete.sh"
    fi

    # Run tests for ripgrep and all sub-crates.
    "$CARGO" test --target "$TARGET" --verbose --all --features 'pcre2'
}

main
