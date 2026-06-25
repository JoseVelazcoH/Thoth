command -v tth >/dev/null 2>&1 || return

autoload -Uz add-zsh-hook

: ${TTH_SESSION_ID:=$(tth new-session-id)}
export TTH_SESSION_ID

_tth_preexec() {
    _TTH_CMD="$1"
    _TTH_START=$EPOCHREALTIME
}

_tth_precmd() {
    local _TTH_EXIT=$?
    [[ -n $_TTH_CMD ]] || return
    local _dur=$(( int((EPOCHREALTIME - _TTH_START) * 1000) ))
    tth record \
        --cmd "$_TTH_CMD" \
        --dir "$PWD" \
        --exit "$_TTH_EXIT" \
        --duration "$_dur" \
        --timestamp "$EPOCHSECONDS" \
        --tags "${TTH_ACTIVE_TAGS:-[]}" \
        --terminal-id "$TTH_SESSION_ID" &!
    _TTH_CMD=""
}

add-zsh-hook preexec _tth_preexec
add-zsh-hook precmd _tth_precmd
