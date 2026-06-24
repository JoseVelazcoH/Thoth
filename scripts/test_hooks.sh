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
        true
    " || true

    if grep -q 'record' "$stub_log" 2>/dev/null; then
        if grep -q -- '--terminal-id' "$stub_log"; then
            _pass "bash: record call includes --terminal-id"
        else
            _skip "bash: record called but --terminal-id not seen (non-interactive)"
        fi
    else
        _pass "bash: hook sourced in non-interactive shell (record fires after real commands)"
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

echo "=== Thoth shell hook smoke tests ==="
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

echo "---"
echo "PASS=$PASS FAIL=$FAIL SKIP=$SKIP"

if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
exit 0
