# `script` phase: you usually build, test and generate docs in this phase

set -ex

. $(dirname $0)/utils.sh

# NOTE Workaround for rust-lang/rust#31907 - disable doc tests when cross compiling
# This has been fixed in the nightly channel but it would take a while to reach the other channels
disable_cross_doctests() {
    if [ $(host) != "$TARGET" ] && [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        if [ "$TRAVIS_OS_NAME" = "osx" ]; then
            brew install gnu-sed --default-names
        fi
        find src -name '*.rs' -type f | xargs sed -i -e 's:\(//.\s*```\):\1 ignore,:g'
    fi
}

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
