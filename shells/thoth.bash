if (( BASH_VERSINFO[0] < 5 )); then return; fi
command -v tth >/dev/null 2>&1 || return

: ${TTH_SESSION_ID:=$(tth new-session-id)}
export TTH_SESSION_ID

_THOTH_IN_PROMPT=0

_thoth_preexec() {
    [[ $_THOTH_IN_PROMPT -eq 1 ]] && return
    [[ "$BASH_COMMAND" == "_thoth_precmd" ]] && return
    _THOTH_CMD="$BASH_COMMAND"
    _THOTH_START=$EPOCHREALTIME
}

_thoth_precmd() {
    local _exit=$?
    _THOTH_IN_PROMPT=1
    [[ -n "${_THOTH_CMD:-}" ]] || { _THOTH_IN_PROMPT=0; return; }
    local _dur=$(( (${EPOCHREALTIME/./} - ${_THOTH_START/./}) / 1000 ))
    tth record \
        --cmd "$_THOTH_CMD" \
        --dir "$PWD" \
        --exit "$_exit" \
        --duration "$_dur" \
        --timestamp "$(date +%s)" \
        --tags "${TTH_ACTIVE_TAGS:-[]}" \
        --terminal-id "$TTH_SESSION_ID" &
    _THOTH_CMD=""
    _THOTH_IN_PROMPT=0
}

_thoth_chain_debug() {
    local _existing
    _existing="$(trap -p DEBUG)"
    if [[ -n "$_existing" ]]; then
        _existing="${_existing#trap -- \'}"
        _existing="${_existing%\' DEBUG}"
        trap "_thoth_preexec; $_existing" DEBUG
    else
        trap '_thoth_preexec' DEBUG
    fi
}
_thoth_chain_debug

PROMPT_COMMAND="_thoth_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
