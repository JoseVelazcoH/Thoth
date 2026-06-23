# Commit Convention

This project follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).

## Format

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

### Rules

- The description must be in lowercase and must not end with a period.
- The body wraps at 72 characters.
- Breaking changes are declared with `!` after the type/scope, or with a `BREAKING CHANGE:` footer.

---

## Types

| Type       | When to use                                              |
|------------|----------------------------------------------------------|
| `feat`     | A new feature visible to the user or CLI consumer        |
| `fix`      | A bug fix                                                |
| `docs`     | Documentation only changes                               |
| `style`    | Formatting, whitespace — no logic change                 |
| `refactor` | Code change that is neither a fix nor a feature          |
| `perf`     | Performance improvement                                  |
| `test`     | Adding or updating tests                                 |
| `build`    | Build system or external dependency changes              |
| `ci`       | CI/CD pipeline changes                                   |
| `chore`    | Maintenance tasks that don't touch production code       |
| `revert`   | Reverts a previous commit                                |

---

## Scopes

Scopes are optional and should match the module or layer being changed.

| Scope       | Area                                     |
|-------------|------------------------------------------|
| `db`        | Database layer (`tth/db.py`)             |
| `project`   | Project management (`tth/project.py`)    |
| `session`   | Session tracking (`tth/session.py`)      |
| `recorder`  | Activity recorder (`tth/recorder.py`)    |
| `search`    | FTS5 search (`tth/search.py`)            |
| `tags`      | Tag system (`tth/tags.py`)               |
| `suggest`   | Suggestions (`tth/suggest.py`)           |
| `installer` | Shell hook installer (`tth/installer.py`)|
| `exporter`  | Data export (`tth/exporter.py`)          |
| `cli`       | CLI entrypoint (`tth/main.py`)           |
| `shells`    | Shell hooks (`shells/`)                  |
| `deps`      | Dependency / packaging changes           |

---

## Examples

```
feat(cli): add `tth start` command with project flag

fix(db): prevent duplicate session entries on concurrent writes

test(search): add FTS5 ranking edge case coverage

docs: add commit convention guide

feat(session)!: remove legacy `--flat` flag

BREAKING CHANGE: `--flat` was deprecated in v0.3 and is now removed.
```

---

## Validation

Commits are validated automatically by the `commitlint` pre-commit hook (`.githooks/commit-msg`).
The hook rejects commits that do not conform to this format.
