command -v tth >/dev/null 2>&1 || return

autoload -Uz add-zsh-hook

: ${TTH_SESSION_ID:=$(tth new-session-id)}
export TTH_SESSION_ID

_tth_preexec() {
    local _cmd="$1"
    case "$_cmd" in
        tth|tth\ *) return ;;
    esac
    _TTH_CMD="$_cmd"
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
        --terminal-id "$TTH_SESSION_ID" \
        --workspace "${TTH_ACTIVE_WORKSPACE:-}" &!
    _TTH_CMD=""
}

add-zsh-hook preexec _tth_preexec
add-zsh-hook precmd _tth_precmd

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

_tth_widget() {
    command -v tth >/dev/null 2>&1 || return
    local out
    out="$(tth </dev/tty 2>/dev/null)"
    local rc=$?
    if [[ "$out" == RUN:* ]]; then
        BUFFER="${out#RUN:}"
        zle reset-prompt
        zle accept-line
    elif [[ "$out" == EDIT:* ]]; then
        BUFFER="${out#EDIT:}"
        CURSOR=${#BUFFER}
        zle reset-prompt
    else
        zle reset-prompt
    fi
}

zle -N _tth_widget
bindkey '^R' _tth_widget
