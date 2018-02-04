#!/usr/bin/env zsh

##
# Compares options in `rg --help` output to options in zsh completion function

emulate -R zsh
setopt extended_glob
setopt no_function_argzero
setopt no_unset

get_comp_args() {
    setopt local_options unset

    # Our completion function recognises a special variable which tells it to
    # dump the _arguments specs and then just return. But do this in a sub-shell
    # anyway to avoid any weirdness
    ( _RG_COMPLETE_LIST_ARGS=1 source $1 )
    return $?
}

main() {
    local diff
    local  rg="${${0:a}:h}/../target/${TARGET:-}/release/rg"
    local _rg="${${0:a}:h}/../complete/_rg"
    local -a help_args comp_args

    [[ -e $rg ]] || rg=${rg/%\/release\/rg/\/debug\/rg}

    [[ -e $rg ]] || {
        printf >&2 'File not found: %s\n' $rg
        return 1
    }
    [[ -e $_rg ]] || {
        printf >&2 'File not found: %s\n' $_rg
        return 1
    }

    printf 'Comparing options:\n-%s\n+%s\n' $rg $_rg

    # 'Parse' options out of the `--help` output. To prevent false positives we
    # only look at lines where the first non-white-space character is `-`
    help_args=( ${(f)"$(
        $rg --help |
        $rg -- '^\s*-' |
        $rg -io -- '[\t ,](-[a-z0-9]|--[a-z0-9-]+)\b' |
        tr -d '\t ,' |
        sort -u
    )"} )

    # 'Parse' options out of the completion function
    comp_args=( ${(f)"$( get_comp_args $_rg )"} )

    comp_args=( ${comp_args#\(*\)}    ) # Strip excluded options
    comp_args=( ${comp_args#\*}       ) # Strip repetition indicator
    comp_args=( ${comp_args%%-[:[]*}  ) # Strip everything after -optname-
    comp_args=( ${comp_args%%[:+=[]*} ) # Strip everything after other optspecs
    comp_args=( ${comp_args##[^-]*}   ) # Remove non-options

    # This probably isn't necessary, but we should ensure the same order
    comp_args=( ${(f)"$( printf '%s\n' $comp_args | sort -u )"} )

    (( $#help_args )) || {
        printf >&2 'Failed to get help_args\n'
        return 1
    }
    (( $#comp_args )) || {
        printf >&2 'Failed to get comp_args\n'
        return 1
    }

    diff="$(
        if diff --help 2>&1 | grep -qF -- '--label'; then
            diff -U2 \
                --label '`rg --help`' \
                --label '`_rg`' \
                =( printf '%s\n' $help_args ) =( printf '%s\n' $comp_args )
        else
            diff -U2 \
                -L '`rg --help`' \
                -L '`_rg`' \
                =( printf '%s\n' $help_args ) =( printf '%s\n' $comp_args )
        fi
    )"

    (( $#diff )) && {
        printf >&2 '%s\n' 'zsh completion options differ from `--help` options:'
        printf >&2 '%s\n' $diff
        return 1
    }
    printf 'OK\n'
    return 0
}

main "${@}"
