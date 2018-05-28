#!/bin/bash

# install stuff needed for the `script` phase

# Where rustup gets installed.
export PATH="$PATH:$HOME/.cargo/bin"

set -ex

. "$(dirname $0)/utils.sh"

install_rustup() {
    curl https://sh.rustup.rs -sSf \
      | sh -s -- -y --default-toolchain="$TRAVIS_RUST_VERSION"
    rustc -V
    cargo -V
}

install_targets() {
    if [ $(host) != "$TARGET" ]; then
        rustup target add $TARGET
    fi
}

install_osx_dependencies() {
    if ! is_osx; then
      return
    fi

    brew install asciidoc docbook-xsl
}

configure_cargo() {
    local prefix=$(gcc_prefix)
    if [ -n "${prefix}" ]; then
        local gcc_suffix=
        if [ -n "$GCC_VERSION" ]; then
          gcc_suffix="-$GCC_VERSION"
        fi
        local gcc="${prefix}gcc${gcc_suffix}"

        # information about the cross compiler
        "${gcc}" -v

        # tell cargo which linker to use for cross compilation
        mkdir -p .cargo
        cat >>.cargo/config <<EOF
[target.$TARGET]
linker = "${gcc}"
EOF
    fi
}

main() {
    install_osx_dependencies
    install_rustup
    install_targets
    configure_cargo
}

main
