# Features

A deeper reference for what Thoth does. Run `tth --help` or `tth <command> --help` for the
exact flags of any command.

- [Capturing commands](#capturing-commands)
- [Searching](#searching)
- [Workspaces](#workspaces)
- [Themes](#themes)
- [Other commands](#other-commands)
- [Configuration](#configuration)

## Capturing commands

Once the shell integration is enabled (`tth install`), every command you run is recorded
with its context:

- the command text, working directory, and inferred project (the enclosing git repo)
- the exit code and how long it took
- the active tags and the active workspace, if any

Thoth's own commands (`tth`, `tth-sw`, `tth-tag`, ...) are never recorded. You can keep
other commands out of your history with a regex filter (see [History filter](#history-filter)),
which is the recommended way to avoid storing secrets. Everything is stored in a local
SQLite database under `~/.local/share/thoth/`.

## Searching

### From the command line

`tth search [QUERY]` does a full-text search with optional filters:

```sh
tth search cargo --project thoth --exit fail --since 2h
```

Filters: `--project`, `--tag` (repeatable), `--exit ok|fail|any`, `--duration '>30'`,
`--since`, `--until`, `--session`, `--limit`. Project and tag matches are
case-insensitive (project also matches as a substring).

### Interactive TUI

Press **`Ctrl-R`** (or run `tth`) to open the finder. As you type, the list filters live
with fuzzy matching across the command, project, directory, and tags. A preview pane on the
right shows the full details of the highlighted entry.

#### Modes (vim-style)

The finder is modal. **Insert** is the default (type to filter); `Esc` drops you into
**Normal** for navigation and actions.

| Key            | Mode   | Action                                  |
| -------------- | ------ | --------------------------------------- |
| _type_         | Insert | Filter the list live                    |
| `Enter`        | both   | Run the selected command               |
| `Tab`          | Insert | Put the command on your prompt to edit  |
| `Esc`          | Insert | Switch to Normal mode                   |
| `j` / `k`      | Normal | Move down / up (arrows work too)        |
| `d`            | Normal | Delete the selected command             |
| `e`            | Normal | Edit the selected command               |
| `i` or `/`     | Normal | Return to Insert mode                   |
| `:`            | Normal | Open the filter cmdline                 |
| `?`            | Normal | Open help                               |
| `q`            | Normal | Quit                                    |

#### Filter cmdline

In Normal mode, press **`:`** to open a floating cmdline and type a filter expression:

```
project:thoth exit:fail since:2h
```

| Field                | Matches                                  |
| -------------------- | ---------------------------------------- |
| `project:` / `p:`    | Project name                             |
| `tag:` / `t:`        | Tag (repeatable)                         |
| `exit:ok` / `fail`   | Exit status                              |
| `since:` / `until:`  | Time window (e.g. `since:2h`)            |
| `dur:>30`            | Duration in seconds (`>`, `<`, `=`)      |

Any free text becomes the fuzzy query. The active filters are shown as chips at the bottom.
Press `Enter` to apply, `Esc` to cancel.

#### Tabs

Use the `←`/`→` arrows to switch between the **History** tab and the **Workspaces** tab.

## Workspaces

A workspace is a named, ordered set of commands you can replay later.

```sh
tth-sw deploy     # start recording into the "deploy" workspace
# ... run your commands ...
tth-ew            # stop recording
tth workspaces    # list your workspaces
```

In the TUI, switch to the **Workspaces** tab: the left pane lists your workspaces and the
right pane shows the commands of the selected one in order. Press `Enter` to replay the
whole sequence in your shell (you confirm first). The commands run one after another, each
in its recorded directory.

## Themes

Thoth ships 11 built-in themes: `default`, `ember`, `frost`, the Catppuccin flavors
(`latte`, `frappe`, `macchiato`, `mocha`), plus `dracula`, `tokyonight`, `rosepine`,
and `solarized`.

```sh
tth theme list        # show available themes (built-in + your own)
tth theme mocha       # switch theme
```

Define your own theme as a file in `~/.config/thoth/themes/<name>.toml`:

```toml
extends = "mocha"          # optional: start from a built-in
selection_bg = "#b4befe"   # override any slot
accent       = "blue"
```

Colors accept hex (`#rrggbb`), ANSI names (`red`, `brightblue`), or a 256-color index.
Slots: `selection_bg`, `selection_fg`, `accent`, `dim`, `border`, `ok`, `fail`, `project`,
`command`, `header`, `controls`, `directory`, `tags`.

## Other commands

| Command                              | What it does                                                        |
| ------------------------------------ | ------------------------------------------------------------------- |
| `tth sessions`                       | List work sessions (commands grouped by terminal session)           |
| `tth stats`                          | Insights: top commands/projects, success rate, busiest hour, error-prone tools |
| `tth export`                         | Export matching commands as a runnable bash script                  |
| `tth forget`                         | Delete recent commands from history                                 |
| `tth tag` / `tth-tag` / `tth-untag`  | Manage active tags                                                  |
| `tth prompt`                         | Instructions to show active tags in your prompt                     |
| `tth doctor`                         | Diagnostics: hooks installed, database, theme, prompt visibility    |
| `tth config`                         | Show or edit the configuration (`tth config set <key> <value>`)     |

## Configuration

Configuration lives in `~/.config/thoth/config.toml` (created by `tth install`). All
settings are optional.

```toml
[session]
gap_minutes = 30           # inactivity gap that starts a new session

[tui]
orientation = "bottom"     # "bottom" or "top"
columns = ["timestamp", "duration", "exit", "project", "command"]

[search]
default_limit = 50
# columns = [...]          # columns for `tth search`
# filter  = ["^ls$"]       # hide matching commands from search results

[history]
# filter = ["--password", "export .*TOKEN"]   # never record matching commands

[theme]
name = "default"
```

### History filter

`[history] filter` is a list of regular expressions; any command matching one is **never
recorded**. By default it ignores Thoth's own commands; add your own patterns to keep
secrets out of your history.
