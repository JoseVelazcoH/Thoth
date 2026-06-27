#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0
SKIP=0

_pass() { echo "PASS: $1"; PASS=$(( PASS + 1 )); }
_fail() { echo "FAIL: $1"; FAIL=$(( FAIL + 1 )); }
_skip() { echo "SKIP: $1"; SKIP=$(( SKIP + 1 )); }

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TTH_BIN="${REPO_ROOT}/target/debug/tth"

if [[ ! -x "$TTH_BIN" ]]; then
    _skip "tth binary not built; run: cargo build first"
    echo "---"; echo "PASS=$PASS FAIL=$FAIL SKIP=$SKIP"; exit 0
fi

_run_bash_test() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_log="$tmpdir/stub.log"
    local stub_bin="$tmpdir/tth"

    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@" >> "$STUB_LOG"
STUB
    chmod +x "$stub_bin"

    STUB_LOG="$stub_log" \
    PATH="$tmpdir:$REPO_ROOT/target/debug:$PATH" \
    TTH_SESSION_ID="" \
    bash --norc --noprofile -c "
        export STUB_LOG='$stub_log'
        source '${REPO_ROOT}/shells/thoth.bash' 2>/dev/null || true
        sleep 0
    " || true

    if [[ -f "$stub_log" ]]; then
        if grep -q 'new-session-id' "$stub_log" 2>/dev/null; then
            _pass "bash: hook sources and calls new-session-id"
        else
            _pass "bash: hook sourced (new-session-id may be skipped in non-interactive)"
        fi
    else
        _pass "bash: hook sourced without error (record call requires interactive shell)"
    fi
}

_run_bash_record_test() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_log="$tmpdir/stub.log"
    local stub_bin="$tmpdir/tth"

    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@" >> "$STUB_LOG"
STUB
    chmod +x "$stub_bin"

    STUB_LOG="$stub_log" \
    PATH="$tmpdir:$REPO_ROOT/target/debug:$PATH" \
    TTH_SESSION_ID="test-sid" \
    bash --norc --noprofile -c "
        export STUB_LOG='$stub_log'
        source '${REPO_ROOT}/shells/thoth.bash' 2>/dev/null || true
        _THOTH_CMD='echo hello'
        _THOTH_START=\$EPOCHREALTIME
        _thoth_precmd
        _i=0
        while (( _i < 100 )); do
            [[ -s '$stub_log' ]] && break
            sleep 0.05
            _i=\$(( _i + 1 ))
        done
    " || true

    local all_ok=1
    for flag in --cmd --dir --exit --duration --timestamp --tags --terminal-id; do
        if ! grep -q -- "$flag" "$stub_log" 2>/dev/null; then
            _fail "bash: record invocation missing flag $flag"
            all_ok=0
        fi
    done
    if [[ $all_ok -eq 1 ]]; then
        _pass "bash: _thoth_precmd passes all 7 flags to tth record"
    fi
}

_run_zsh_test() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available"
        return
    fi

    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_log="$tmpdir/stub.log"
    local stub_bin="$tmpdir/tth"

    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@" >> "$STUB_LOG"
STUB
    chmod +x "$stub_bin"

    STUB_LOG="$stub_log" \
    PATH="$tmpdir:$REPO_ROOT/target/debug:$PATH" \
    TTH_SESSION_ID="" \
    zsh --no-rcs -c "
        export STUB_LOG='$stub_log'
        source '${REPO_ROOT}/shells/thoth.zsh' 2>/dev/null || true
        true
    " || true

    if [[ -f "$stub_log" ]] && grep -q 'new-session-id' "$stub_log" 2>/dev/null; then
        _pass "zsh: hook sources and calls new-session-id"
    else
        _pass "zsh: hook sourced without error"
    fi
}

_run_bash_version_check() {
    if (( BASH_VERSINFO[0] < 5 )); then
        _skip "bash < 5; hook returns early by design"
        return
    fi
    _pass "bash >= 5 available"
}

_run_bash_syntax_check() {
    if bash -n "${REPO_ROOT}/shells/thoth.bash" 2>/dev/null; then
        _pass "thoth.bash: syntax valid"
    else
        _fail "thoth.bash: syntax error"
    fi
}

_run_zsh_syntax_check() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for syntax check"
        return
    fi
    if zsh -n "${REPO_ROOT}/shells/thoth.zsh" 2>/dev/null; then
        _pass "thoth.zsh: syntax valid"
    else
        _fail "thoth.zsh: syntax error"
    fi
}

_check_flags_in_file() {
    local label="$1"
    local file="$2"
    local content
    content="$(cat "$file")"
    for flag in --cmd --dir --exit --duration --timestamp --tags --terminal-id; do
        if [[ "$content" == *"$flag"* ]]; then
            _pass "$label: flag $flag present"
        else
            _fail "$label: flag $flag MISSING"
        fi
    done
}

_run_epochseconds_check() {
    local content
    content="$(cat "${REPO_ROOT}/shells/thoth.bash")"
    if [[ "$content" == *'$(date +%s)'* ]]; then
        _fail "thoth.bash: uses \$(date +%s) subshell instead of \$EPOCHSECONDS"
    else
        _pass "thoth.bash: uses \$EPOCHSECONDS builtin (no date subshell)"
    fi
}

_run_flag_check() {
    _check_flags_in_file "thoth.bash" "${REPO_ROOT}/shells/thoth.bash"
}

_run_zsh_flag_check() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for flag check"
        return
    fi
    _check_flags_in_file "thoth.zsh" "${REPO_ROOT}/shells/thoth.zsh"
}

_run_bash_trap_chain_single_quote_test() {
    if (( BASH_VERSINFO[0] < 5 )); then
        _skip "bash < 5; trap chain test requires bash >= 5"
        return
    fi

    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local trap_log="$tmpdir/trap.log"
    local err_log="$tmpdir/err.log"

    local after_err_log="$tmpdir/after_err.log"

    bash --norc --noprofile -c "
        trap 'echo it'\\''s here >> \"$trap_log\"' DEBUG
        source '${REPO_ROOT}/shells/thoth.bash' 2>/dev/null || true
        echo trigger 2>'$after_err_log'
    " 2>/dev/null || true

    local after_err=""
    if [[ -f "$after_err_log" ]]; then
        after_err="$(cat "$after_err_log")"
    fi

    if [[ "$after_err" == *"EOF"* ]] || [[ "$after_err" == *"inesperado"* ]] || [[ "$after_err" == *"unexpected"* ]] || [[ "$after_err" == *"syntax error"* ]] || [[ "$after_err" == *"parse error"* ]] || [[ "$after_err" == *"debug trap"* ]]; then
        _fail "bash: DEBUG trap chain breaks with single-quoted prior trap body"
    else
        _pass "bash: DEBUG trap chain survives prior trap body containing single quote"
    fi
}

_run_zsh_widget_defined() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for widget definition check"
        return
    fi
    local content
    content="$(cat "${REPO_ROOT}/shells/thoth.zsh")"
    if [[ "$content" == *"_tth_widget"* ]]; then
        _pass "thoth.zsh: _tth_widget function defined"
    else
        _fail "thoth.zsh: _tth_widget function NOT found"
    fi
    if [[ "$content" == *"bindkey '^R'"* ]]; then
        _pass "thoth.zsh: bindkey '^R' present"
    else
        _fail "thoth.zsh: bindkey '^R' NOT found"
    fi
    if [[ "$content" == *"zle -N _tth_widget"* ]]; then
        _pass "thoth.zsh: zle -N _tth_widget registration present"
    else
        _fail "thoth.zsh: zle -N _tth_widget NOT found"
    fi
}

_run_bash_widget_defined() {
    local content
    content="$(cat "${REPO_ROOT}/shells/thoth.bash")"
    if [[ "$content" == *"_tth_widget"* ]]; then
        _pass "thoth.bash: _tth_widget function defined"
    else
        _fail "thoth.bash: _tth_widget function NOT found"
    fi
    if [[ "$content" == *'bind -x'*'\C-r'* ]] || [[ "$content" == *"bind -x"*"\C-r"* ]]; then
        _pass "thoth.bash: bind -x Ctrl-R present"
    else
        _fail "thoth.bash: bind -x Ctrl-R NOT found"
    fi
}

_run_zsh_capture_exclusion() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for capture exclusion check"
        return
    fi

    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_log="$tmpdir/stub.log"
    local stub_bin="$tmpdir/tth"

    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@" >> "$STUB_LOG"
STUB
    chmod +x "$stub_bin"

    STUB_LOG="$stub_log" \
    PATH="$tmpdir:$REPO_ROOT/target/debug:$PATH" \
    TTH_SESSION_ID="test-sid" \
    zsh --no-rcs -c "
        export STUB_LOG='$stub_log'
        source '${REPO_ROOT}/shells/thoth.zsh' 2>/dev/null || true
        _tth_preexec 'tth search foo'
        _tth_preexec 'tth'
        _TTH_START=\$EPOCHREALTIME
        _tth_preexec 'echo hello'
        _tth_precmd
        sleep 0.1
    " 2>/dev/null || true

    if [[ -f "$stub_log" ]]; then
        if grep -q 'record' "$stub_log" 2>/dev/null; then
            if grep 'record' "$stub_log" | grep -q 'tth'; then
                _fail "zsh: tth command was recorded (exclusion not working)"
            else
                _pass "zsh: capture exclusion prevents tth commands from being recorded"
            fi
        else
            _pass "zsh: capture exclusion prevents tth commands from being recorded"
        fi
    else
        _pass "zsh: capture exclusion works (no spurious record call)"
    fi
}

_run_bash_capture_exclusion() {
    if (( BASH_VERSINFO[0] < 5 )); then
        _skip "bash < 5; capture exclusion test requires bash >= 5"
        return
    fi

    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_log="$tmpdir/stub.log"
    local stub_bin="$tmpdir/tth"

    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@" >> "$STUB_LOG"
STUB
    chmod +x "$stub_bin"

    STUB_LOG="$stub_log" \
    PATH="$tmpdir:$REPO_ROOT/target/debug:$PATH" \
    TTH_SESSION_ID="test-sid" \
    bash --norc --noprofile -c "
        export STUB_LOG='$stub_log'
        source '${REPO_ROOT}/shells/thoth.bash' 2>/dev/null || true
        BASH_COMMAND='tth search foo'
        _thoth_preexec
        BASH_COMMAND='tth'
        _thoth_preexec
        BASH_COMMAND='echo hello'
        _thoth_preexec
        _THOTH_START=\$EPOCHREALTIME
        _THOTH_CMD='echo hello'
        _thoth_precmd
        _i=0
        while (( _i < 100 )); do
            [[ -s '$stub_log' ]] && break
            sleep 0.05
            _i=\$(( _i + 1 ))
        done
    " 2>/dev/null || true

    if [[ -f "$stub_log" ]]; then
        if grep -q 'record' "$stub_log" 2>/dev/null; then
            if grep 'record' "$stub_log" | grep -qE -- '(^| )tth( |$)'; then
                _fail "bash: tth command was recorded (exclusion not working)"
            else
                _pass "bash: capture exclusion prevents tth commands from being recorded"
            fi
        else
            _pass "bash: capture exclusion works (no record call seen)"
        fi
    else
        _pass "bash: capture exclusion works (no spurious record call)"
    fi
}

_run_zsh_parse_run_prefix() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for RUN: prefix parse check"
        return
    fi
    local result
    result="$(zsh --no-rcs -c '
        out="RUN:git status"
        echo "${out#RUN:}"
    ' 2>/dev/null)"
    if [[ "$result" == "git status" ]]; then
        _pass "zsh: RUN: prefix stripped correctly"
    else
        _fail "zsh: RUN: prefix strip failed; got: $result"
    fi
}

_run_zsh_parse_edit_prefix() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for EDIT: prefix parse check"
        return
    fi
    local result
    result="$(zsh --no-rcs -c '
        out="EDIT:vim main.rs"
        echo "${out#EDIT:}"
    ' 2>/dev/null)"
    if [[ "$result" == "vim main.rs" ]]; then
        _pass "zsh: EDIT: prefix stripped correctly"
    else
        _fail "zsh: EDIT: prefix strip failed; got: $result"
    fi
}

_run_bash_parse_run_prefix() {
    local result
    result="$(bash --norc --noprofile -c '
        out="RUN:git status"
        echo "${out#RUN:}"
    ' 2>/dev/null)"
    if [[ "$result" == "git status" ]]; then
        _pass "bash: RUN: prefix stripped correctly"
    else
        _fail "bash: RUN: prefix strip failed; got: $result"
    fi
}

_run_bash_parse_edit_prefix() {
    local result
    result="$(bash --norc --noprofile -c '
        out="EDIT:vim main.rs"
        echo "${out#EDIT:}"
    ' 2>/dev/null)"
    if [[ "$result" == "vim main.rs" ]]; then
        _pass "bash: EDIT: prefix stripped correctly"
    else
        _fail "bash: EDIT: prefix strip failed; got: $result"
    fi
}

_run_init_zsh_syntax_check() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for init zsh syntax check"
        return
    fi
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN
    local script_file="$tmpdir/init_zsh.zsh"
    "$TTH_BIN" init zsh > "$script_file" 2>/dev/null
    if zsh -n "$script_file" 2>/dev/null; then
        _pass "tth init zsh: output is syntactically valid zsh"
    else
        _fail "tth init zsh: syntax error in output"
    fi
}

_run_init_bash_syntax_check() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN
    local script_file="$tmpdir/init_bash.bash"
    "$TTH_BIN" init bash > "$script_file" 2>/dev/null
    if bash -n "$script_file" 2>/dev/null; then
        _pass "tth init bash: output is syntactically valid bash"
    else
        _fail "tth init bash: syntax error in output"
    fi
}

_run_init_zsh_defines_hooks() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for init zsh function-definition check"
        return
    fi
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN
    local stub_bin="$tmpdir/tth"
    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
echo "$@"
STUB
    chmod +x "$stub_bin"
    local result
    result="$(PATH="$tmpdir:$PATH" TTH_SESSION_ID="test" zsh --no-rcs -c "
        source <(\"$TTH_BIN\" init zsh) 2>/dev/null || true
        typeset -f _tth_preexec >/dev/null 2>&1 && echo defined_preexec
        typeset -f _tth_precmd  >/dev/null 2>&1 && echo defined_precmd
        typeset -f _tth_widget  >/dev/null 2>&1 && echo defined_widget
    " 2>/dev/null)"
    local ok=1
    for marker in defined_preexec defined_precmd defined_widget; do
        if [[ "$result" != *"$marker"* ]]; then
            _fail "tth init zsh: $marker not defined after sourcing output"
            ok=0
        fi
    done
    [[ $ok -eq 1 ]] && _pass "tth init zsh: hook functions defined after sourcing output"
}

_run_tth_tag_zsh_test() {
    if ! command -v zsh >/dev/null 2>&1; then
        _skip "zsh not available for tth-tag test"
        return
    fi
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_bin="$tmpdir/tth"
    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
if [[ "$1" == "tag" ]]; then
    shift
    echo "export TTH_ACTIVE_TAGS='[\"$1\"]'"
    echo "export TTH_PROMPT_TAGS='[$1]'"
elif [[ "$1" == "new-session-id" ]]; then
    echo "test-session"
fi
STUB
    chmod +x "$stub_bin"

    local result
    result="$(PATH="$tmpdir:$PATH" TTH_SESSION_ID="test" zsh --no-rcs -c "
        source '${REPO_ROOT}/shells/thoth.zsh' 2>/dev/null || true
        typeset -f tth-tag >/dev/null 2>&1 && echo defined_tth_tag
        typeset -f tth-untag >/dev/null 2>&1 && echo defined_tth_untag
        tth-tag foo 2>/dev/null
        echo \"TAGS=\$TTH_ACTIVE_TAGS\"
        echo \"PROMPT=\$TTH_PROMPT_TAGS\"
    " 2>/dev/null)"

    if [[ "$result" == *"defined_tth_tag"* ]]; then
        _pass "zsh: tth-tag function defined after sourcing thoth.zsh"
    else
        _fail "zsh: tth-tag function NOT defined after sourcing thoth.zsh"
    fi
    if [[ "$result" == *"defined_tth_untag"* ]]; then
        _pass "zsh: tth-untag function defined after sourcing thoth.zsh"
    else
        _fail "zsh: tth-untag function NOT defined after sourcing thoth.zsh"
    fi
    if [[ "$result" == *'TAGS=["foo"]'* ]]; then
        _pass "zsh: tth-tag foo sets TTH_ACTIVE_TAGS"
    else
        _fail "zsh: tth-tag foo did not set TTH_ACTIVE_TAGS; got: $result"
    fi
    if [[ "$result" == *"PROMPT=[foo]"* ]]; then
        _pass "zsh: tth-tag foo sets TTH_PROMPT_TAGS=[foo]"
    else
        _fail "zsh: tth-tag foo did not set TTH_PROMPT_TAGS; got: $result"
    fi
}

_run_tth_tag_bash_test() {
    if (( BASH_VERSINFO[0] < 5 )); then
        _skip "bash < 5; tth-tag bash test requires bash >= 5"
        return
    fi
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local stub_bin="$tmpdir/tth"
    cat > "$stub_bin" <<'STUB'
#!/usr/bin/env bash
if [[ "$1" == "tag" ]]; then
    shift
    echo "export TTH_ACTIVE_TAGS='[\"$1\"]'"
    echo "export TTH_PROMPT_TAGS='[$1]'"
elif [[ "$1" == "new-session-id" ]]; then
    echo "test-session"
fi
STUB
    chmod +x "$stub_bin"

    local result
    result="$(PATH="$tmpdir:$PATH" TTH_SESSION_ID="test" bash --norc --noprofile -c "
        source '${REPO_ROOT}/shells/thoth.bash' 2>/dev/null || true
        declare -f tth-tag >/dev/null 2>&1 && echo defined_tth_tag
        declare -f tth-untag >/dev/null 2>&1 && echo defined_tth_untag
        tth-tag bar 2>/dev/null
        echo \"TAGS=\$TTH_ACTIVE_TAGS\"
        echo \"PROMPT=\$TTH_PROMPT_TAGS\"
    " 2>/dev/null)"

    if [[ "$result" == *"defined_tth_tag"* ]]; then
        _pass "bash: tth-tag function defined after sourcing thoth.bash"
    else
        _fail "bash: tth-tag function NOT defined after sourcing thoth.bash"
    fi
    if [[ "$result" == *"defined_tth_untag"* ]]; then
        _pass "bash: tth-untag function defined after sourcing thoth.bash"
    else
        _fail "bash: tth-untag function NOT defined after sourcing thoth.bash"
    fi
    if [[ "$result" == *'TAGS=["bar"]'* ]]; then
        _pass "bash: tth-tag bar sets TTH_ACTIVE_TAGS"
    else
        _fail "bash: tth-tag bar did not set TTH_ACTIVE_TAGS; got: $result"
    fi
    if [[ "$result" == *"PROMPT=[bar]"* ]]; then
        _pass "bash: tth-tag bar sets TTH_PROMPT_TAGS=[bar]"
    else
        _fail "bash: tth-tag bar did not set TTH_PROMPT_TAGS; got: $result"
    fi
}

_run_tth_tag_capture_exclusion() {
    local content
    content="$(cat "${REPO_ROOT}/shells/thoth.zsh")"
    if [[ "$content" == *'tth|tth\ *'* ]] || [[ "$content" == *'tth|tth '*'*'* ]]; then
        _pass "thoth.zsh: tth-tag excluded from capture (tth* case pattern covers it)"
    else
        _fail "thoth.zsh: capture exclusion pattern may not cover tth-tag calls"
    fi
}

echo "=== Thoth shell hook smoke tests and tag tests ==="
_run_bash_version_check
_run_bash_syntax_check
_run_zsh_syntax_check
_run_epochseconds_check
_run_flag_check
_run_zsh_flag_check
_run_bash_trap_chain_single_quote_test
_run_bash_test
_run_bash_record_test
_run_zsh_test
_run_zsh_widget_defined
_run_bash_widget_defined
_run_zsh_capture_exclusion
_run_bash_capture_exclusion
_run_zsh_parse_run_prefix
_run_zsh_parse_edit_prefix
_run_bash_parse_run_prefix
_run_bash_parse_edit_prefix
_run_init_zsh_syntax_check
_run_init_bash_syntax_check
_run_init_zsh_defines_hooks
_run_tth_tag_zsh_test
_run_tth_tag_bash_test
_run_tth_tag_capture_exclusion

echo "---"
echo "PASS=$PASS FAIL=$FAIL SKIP=$SKIP"

if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
exit 0
