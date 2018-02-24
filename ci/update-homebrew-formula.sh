#!/bin/bash
set -eu -o pipefail

[[ $# != 1 ]] && {
  echo 1>&2 "USAGE: $0 <tag>"
  exit 2
}

SRC_DIR="$(cd "${0%/*}/.." && pwd)"
VERSION="${1:?}"
TEMPLATE_FILE="${SRC_DIR}/pkg/brew/ripgrep-bin.rb.in"
HOMEBREW_FILE="${TEMPLATE_FILE%.*}"

OSX_FILE=ripgrep-${VERSION}-x86_64-apple-darwin.tar.gz
LINUX_FILE=ripgrep-${VERSION}-x86_64-unknown-linux-musl.tar.gz
URL_PREFIX=https://github.com/BurntSushi/ripgrep/releases/download/${VERSION}

# shellcheck disable=2027,2064
trap "rm -f $OSX_FILE $LINUX_FILE; exit 1" INT

SLEEP_INTERVAL=5
ROUND=0
while ! [[ -f $OSX_FILE && -f $LINUX_FILE ]]; do
  [[ $ROUND == 0 ]] && {
    echo 1>&2 "Waiting for '$OSX_FILE' and '$LINUX_FILE' to become available... (Ctrl+C to interrupt)"
  }
  ROUND=$((ROUND + 1))
  
  for file in "$OSX_FILE" "$LINUX_FILE"; do 
    [[ -f $file ]] && continue
    { curl --fail -sLo "$file" "$URL_PREFIX/$file" \
        && echo 1>&2 "Downloaded '$file'"; } || true
  done
  echo 1>&2 -n '.'
  sleep $SLEEP_INTERVAL
done

SHA_SUM=$(
  which sha256sum 2>/dev/null \
  || which gsha256sum 2>/dev/null \
  || { echo 1>&2 "sha256 program not found"; false; } \
)

OSX_SHA256="$($SHA_SUM "$OSX_FILE" | awk '{print $1}')"
LINUX_SHA256="$($SHA_SUM "$LINUX_FILE" | awk '{print $1}')"
TEMPLATE_NOTE="---> DO NOT EDIT <--- (this file was generated from $TEMPLATE_FILE"
export VERSION OSX_SHA256 LINUX_SHA256 TEMPLATE_NOTE

envsubst < "$TEMPLATE_FILE" > "$HOMEBREW_FILE" && {
  echo 1>&2 'homebrew update finished'
}
