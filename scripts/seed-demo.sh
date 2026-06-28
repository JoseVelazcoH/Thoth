#!/usr/bin/env bash
# Seeds a throwaway Thoth database and config for the demo recording.
# Nothing here touches your real history: it writes only to $THOTH_DB / $THOTH_CONFIG
# and to a scratch directory under /tmp.
#
# Usage:
#   THOTH_DB=/tmp/thoth-demo.db THOTH_CONFIG=/tmp/thoth-demo-config.toml bash scripts/seed-demo.sh
set -euo pipefail

DB="${THOTH_DB:-/tmp/thoth-demo.db}"
CFG="${THOTH_CONFIG:-/tmp/thoth-demo-config.toml}"
PROJDIR="${THOTH_DEMO_PROJECTS:-/tmp/thoth-demo-projects}"
export THOTH_DB="$DB"
export THOTH_CONFIG="$CFG"

rm -f "$DB"

# Thoth infers the project name from the enclosing git repo, so create the demo
# projects as real (empty) git repos. Their basenames become the project names.
rm -rf "$PROJDIR"
for p in thoth webapp infra; do
  mkdir -p "$PROJDIR/$p"
  git -C "$PROJDIR/$p" init -q
done
THOTH="$PROJDIR/thoth"
WEB="$PROJDIR/webapp"
INFRA="$PROJDIR/infra"

# Demo config: a nice theme and a column layout that shows tags.
cat > "$CFG" <<EOF
[theme]
name = "mocha"

[tui]
orientation = "bottom"
columns = ["timestamp", "duration", "exit", "project", "tags", "command"]
EOF

now="$(date +%s)"

# rec <cmd> <dir> <exit> <duration_ms> <tags-json> <workspace> <seconds_ago>
rec() {
  tth record --cmd "$1" --dir "$2" --exit "$3" --duration "$4" \
    --tags "$5" --workspace "$6" --timestamp "$(( now - $7 ))" >/dev/null
}

# --- project: thoth ---
rec "cargo build"                            "$THOTH" 0 1820  '["rust"]'      "" 5400
rec "cargo clippy --all-targets"             "$THOTH" 0 2100  '["rust"]'      "" 5100
rec "cargo fmt"                              "$THOTH" 0 120   '[]'            "" 4800
rec "cargo test"                             "$THOTH" 1 4300  '["rust","ci"]' "" 4500
rec "git status"                             "$THOTH" 0 30    '[]'            "" 4200
rec "cargo test --release"                   "$THOTH" 1 9800  '["rust","ci"]' "" 3900
rec "cargo build --release"                  "$THOTH" 0 33000 '["rust"]'      "" 3600
rec "git commit -m 'feat: add fuzzy search'" "$THOTH" 0 80    '[]'            "" 3300

# --- project: webapp ---
rec "npm install"            "$WEB" 0 9100 '["node"]'   "" 3000
rec "npm run build"          "$WEB" 1 5200 '["node"]'   "" 2700
rec "docker compose up -d"   "$WEB" 0 4200 '["docker"]' "" 2400
rec "npm run dev"            "$WEB" 0 210  '["node"]'   "" 2100

# --- project: infra ---
rec "terraform plan"         "$INFRA" 0 6400 '["infra"]' "" 1800
rec "kubectl get pods"       "$INFRA" 0 540  '["k8s"]'   "" 1500

# --- a recorded workspace "release" (safe commands that run on replay) ---
rec "echo 'Running release checks...'" "$THOTH" 0 12 '[]' "release" 900
rec "cargo --version"                  "$THOTH" 0 40 '[]' "release" 880
rec "echo 'All checks passed.'"        "$THOTH" 0 9  '[]' "release" 860

echo "Seeded demo database at $DB (config $CFG)."
