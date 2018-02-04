mktempd() {
    echo $(mktemp -d 2>/dev/null || mktemp -d -t tmp)
}

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
        aarch64-unknown-linux-gnu)
            echo aarch64-linux-gnu-
            ;;
        arm*-gnueabihf)
            echo arm-linux-gnueabihf-
            ;;
        *)
            return
            ;;
    esac
}

dobin() {
    [ -z $MAKE_DEB ] && die 'dobin: $MAKE_DEB not set'
    [ $# -lt 1 ] && die "dobin: at least one argument needed"

    local f prefix=$(gcc_prefix)
    for f in "$@"; do
        install -m0755 $f $dtd/debian/usr/bin/
        ${prefix}strip -s $dtd/debian/usr/bin/$(basename $f)
    done
}

architecture() {
    case ${TARGET:?} in
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
    case "${TARGET}" in
        i686-unknown-netbsd) return 1 ;; # i686-unknown-netbsd - SSE2
        i686*|x86_64*)       return 0 ;;
    esac
    return 1
}
