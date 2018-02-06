#!/bin/bash

# build, test and generate docs in this phase

set -ex

. "$(dirname $0)/utils.sh"

main() {
    # Travis sometimes caches the target directory, which makes testing the
    # output of cargo a little trickier. So just wipe it.
    cargo clean
    # Test a normal debug build.
    cargo build --target "${TARGET}" --verbose --all

    # Show the output of build.rs stderr.
    set +x
    find ./target/"$TARGET"/debug -name stderr | while read stderr; do
      if [ -s "$stderr" ]; then
        echo "===== $stderr ====="
        cat "$stderr"
        echo "====="
      fi
    done
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
    find ./target/"$TARGET"/debug/build/ripgrep-* -name 'out' | \
        while read outdir; do
          file "$outdir/rg.bash"
          file "$outdir/rg.fish"
          file "$outdir/_rg.ps1"
          # man page requires asciidoc, and we only install it on Linux x86.
          if is_linux; then
              file "$outdir/rg.1"
          fi
        done

    # Run tests for ripgrep and all sub-crates.
    cargo test --target "${TARGET}" --verbose --all
}

main
