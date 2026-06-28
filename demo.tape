# Thoth demo recording.
#   Install vhs: https://github.com/charmbracelet/vhs   (needs ttyd + ffmpeg)
#   Render:      vhs demo.tape   ->   demo.gif
#
# Everything runs against a throwaway demo database (see scripts/seed-demo.sh),
# so your real history never appears.

Output demo.gif

Require tth

Set Shell "bash"
Set FontSize 18
Set Width 1280
Set Height 720
Set Padding 24
Set Theme "Catppuccin Mocha"
Set TypingSpeed 90ms

# --- hidden setup: PATH, demo db/config, seed, clean prompt, then enable the hooks ---
# The hooks record every command, so set the prompt and clear BEFORE sourcing them,
# and drop the two commands that do get recorded with `tth forget` (which is never recorded).
Hide
Type 'export PATH="$HOME/.cargo/bin:$PATH"' Enter
Type "export THOTH_DB=/tmp/thoth-demo.db" Enter
Type "export THOTH_CONFIG=/tmp/thoth-demo-config.toml" Enter
Type "PS1='> '" Enter
Type "bash scripts/seed-demo.sh >/dev/null 2>&1" Enter
Sleep 2500ms
# Enable the shell hooks for the Ctrl-R widget, then disable command capture so the
# demo session does not record anything: the database stays exactly as seeded.
Type 'eval "$(tth init bash)"' Enter
Type "trap - DEBUG; PROMPT_COMMAND=''; clear" Enter
Sleep 1s
Show

# --- Act 1: open the TUI (Ctrl-R) and filter live by typing ---
Sleep 700ms
Ctrl+R
Sleep 1400ms
Type "cargo"
Sleep 1800ms

# --- Act 2: Esc to Normal (vim) mode, then ':' for the filter cmdline ---
Escape
Sleep 700ms
Type ":"
Sleep 500ms
Type "project:thoth exit:fail"
Sleep 900ms
Enter
Sleep 1800ms

# --- Act 3: Tab puts the selected command onto the terminal prompt ---
Tab
Sleep 1800ms
Ctrl+U
Sleep 700ms

# --- Act 4: open the TUI again, go to Workspaces, and replay one ---
Ctrl+R
Sleep 1200ms
Right
Sleep 1200ms
Enter
Sleep 800ms
Type "y"
Sleep 2800ms
Sleep 1000ms
