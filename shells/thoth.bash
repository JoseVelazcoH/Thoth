if (( BASH_VERSINFO[0] < 5 )); then return; fi
command -v tth >/dev/null 2>&1 || return

: ${TTH_SESSION_ID:=$(tth new-session-id)}
export TTH_SESSION_ID

_THOTH_IN_PROMPT=0
_THOTH_PREV_DEBUG=""

_thoth_preexec() {
    [[ $_THOTH_IN_PROMPT -eq 1 ]] && return
    [[ "$BASH_COMMAND" == "_thoth_precmd" ]] && return
    local _cmd="$BASH_COMMAND"
    case "$_cmd" in
        tth|tth\ *) return ;;
    esac
    _THOTH_CMD="$_cmd"
    _THOTH_START=$EPOCHREALTIME
    if [[ -n "$_THOTH_PREV_DEBUG" ]]; then
        eval "$_THOTH_PREV_DEBUG"
    fi
}

_thoth_precmd() {
    local _exit=$?
    _THOTH_IN_PROMPT=1
    [[ -n "${_THOTH_CMD:-}" ]] || { _THOTH_IN_PROMPT=0; return; }
    local _dur=$(( (${EPOCHREALTIME/./} - ${_THOTH_START/./}) / 1000 ))
    ( tth record \
        --cmd "$_THOTH_CMD" \
        --dir "$PWD" \
        --exit "$_exit" \
        --duration "$_dur" \
        --timestamp "$EPOCHSECONDS" \
        --tags "${TTH_ACTIVE_TAGS:-[]}" \
        --terminal-id "$TTH_SESSION_ID" \
        --workspace "${TTH_ACTIVE_WORKSPACE:-}" & )
    _THOTH_CMD=""
    _THOTH_IN_PROMPT=0
}

_thoth_chain_debug() {
    local _raw _body_quoted
    _raw="$(trap -p DEBUG 2>/dev/null)"
    trap - DEBUG
    if [[ -n "$_raw" ]]; then
        _body_quoted="${_raw#trap -- }"
        _body_quoted="${_body_quoted% DEBUG}"
        eval "_THOTH_PREV_DEBUG=${_body_quoted}"
    fi
    trap '_thoth_preexec' DEBUG
}
_thoth_chain_debug

PROMPT_COMMAND="_thoth_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"

_tth_widget() {
    command -v tth >/dev/null 2>&1 || return
    local out
    out="$(tth </dev/tty 2>/dev/null)"
    if [[ "$out" == RUN:* ]]; then
        READLINE_LINE="${out#RUN:}"
        READLINE_POINT=${#READLINE_LINE}
        READLINE_MARK=-1
        ( eval "$READLINE_LINE" )
    elif [[ "$out" == EDIT:* ]]; then
        READLINE_LINE="${out#EDIT:}"
        READLINE_POINT=${#READLINE_LINE}
    elif [[ "$out" == REPLAY:* ]]; then
        bash "${out#REPLAY:}"
    fi
}

bind -x '"\C-r": _tth_widget'

tth-tag() {
    eval "$(command tth tag "$1")"
}

tth-untag() {
    if [[ "$1" == "--all" ]]; then
        eval "$(command tth untag --all)"
    else
        eval "$(command tth untag "$1")"
    fi
}

tth-sw() {
    eval "$(command tth workspace start "$1")"
}

tth-ew() {
    eval "$(command tth workspace end)"
}
