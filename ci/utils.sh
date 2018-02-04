#!/bin/bash

# Various utility functions used through CI.

host() {
    case "$TRAVIS_OS_NAME" in
        linux)
            echo x86_64-unknown-linux-gnu
            ;;
        osx)
            echo x86_64-apple-darwin
            ;;
    esac
}

gcc_prefix() {
    case "$TARGET" in
        arm*-gnueabihf)
            echo arm-linux-gnueabihf-
            ;;
        *)
            return
            ;;
    esac
}

architecture() {
    case "$TARGET" in
        x86_64-*)
            echo amd64
            ;;
        i686-*|i586-*|i386-*)
            echo i386
            ;;
        arm*-unknown-linux-gnueabihf)
            echo armhf
            ;;
        *)
            die "architecture: unexpected target $TARGET"
            ;;
    esac
}

is_ssse3_target() {
    case "$TARGET" in
        x86_64*)  return 0 ;;
        *)        return 1 ;;
    esac
}
