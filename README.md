<p align="center">
  <picture><source media="(prefers-color-scheme: dark)" srcset="https://shieldcn.dev/header/surface.svg?title=Thoth&amp;subtitle=Your+shell+forgets.+Thoth+doesn%27t.&amp;logo=ri%3ABsTerminal&amp;logoColor=000000&amp;mode=dark&amp;theme=orange&amp;font=geist-mono&amp;border=false" /><img alt="header" src="https://shieldcn.dev/header/surface.svg?title=Thoth&amp;subtitle=Your+shell+forgets.+Thoth+doesn%27t.&amp;logo=ri%3ABsTerminal&amp;logoColor=000000&amp;mode=light&amp;theme=orange&amp;font=geist-mono&amp;border=false" /></picture>
</p>

## What is Thoth?
Thoth is an intelligent shell history. Instead of a flat list of commands, it records each command together with the context it ran in: the working directory, the inferred project, how long it took, its exit code, and the tags of the active work session.

Commands are automatically grouped into work sessions and can be searched by project, tag, date, result, and free text.

The name comes from Thoth, the Egyptian god of writing and memory. The binary is tth.


## Status

Early development. The capture engine is written in Rust and ships as a single static binary (SQLite with full-text search is bundled in). Shell hooks for automatic capture (bash and zsh) and the query commands are in progress.

A Python prototype that validated the original design lives under `prototype/python/`.

## Prompt setup

<a name="prompt-setup"></a>

After running `tth install`, the shell hook exports `TTH_PROMPT_TAGS` (e.g. `[work][api]`) and `TTH_ACTIVE_TAGS` (JSON) on every command. To show active tags in your prompt, pick the section for your framework below.

### Starship

Add to `~/.config/starship.toml`:

```toml
[env_var.thoth_tags]
variable = "TTH_PROMPT_TAGS"
format = "[$env_value]($style) "
style = "bold yellow"
```

**Important:** Starship does not render modules that are not referenced in the top-level `format` string. Add `${env_var.thoth_tags}` to your `format` where you want tags to appear:

```toml
format = "$git_status ${env_var.thoth_tags}$character"
```

### Powerlevel10k

Add to your `~/.zshrc` after p10k is loaded:

```zsh
# 1. Define a custom segment function:
prompt_tth_tags() {
  p10k segment -t "$TTH_PROMPT_TAGS"
}

# 2. Add tth_tags to your prompt elements, e.g.:
# POWERLEVEL9K_LEFT_PROMPT_ELEMENTS=(... tth_tags)
# or POWERLEVEL9K_RIGHT_PROMPT_ELEMENTS=(... tth_tags)
```

See [p10k custom segments](https://github.com/romkatv/powerlevel10k#batteries-included) for details.

### oh-my-posh

Add a `text` segment to your theme JSON or YAML that reads the env var:

```json
{
  "type": "text",
  "template": "{{ .Env.TTH_PROMPT_TAGS }}"
}
```

YAML equivalent:

```yaml
- type: text
  template: "{{ .Env.TTH_PROMPT_TAGS }}"
```

See [oh-my-posh text segment docs](https://ohmyposh.dev/docs/segments/system/text).

### Plain PS1 / PROMPT

```zsh
# zsh
PROMPT="${TTH_PROMPT_TAGS} $PROMPT"
```

```bash
# bash
PS1="${TTH_PROMPT_TAGS} $PS1"
```

You can also run `tth prompt --framework <starship|powerlevel10k|oh-my-posh|generic>` to print the snippet for your framework, or just `tth prompt` to auto-detect.

