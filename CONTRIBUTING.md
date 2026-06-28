# Contributing

First of all, thank you so much for taking the time to contribute to THOTH! 𓁟

## Guidelines

The following is a set of guidelines for contributing to this repository. They are
guidelines, not strict rules, so use your best judgment, and feel free to propose changes
to this document in a pull request.

## Development setup

Thoth is a Rust project. You will need a [Rust toolchain](https://rustup.rs) (stable) and a
C compiler, since the bundled SQLite is built from source.

```sh
git clone https://github.com/JoseVelazcoH/Thoth.git
cd Thoth
cargo build            # build
cargo test             # run the test suite
cargo run -- --help    # run the tth binary
```

To dogfood your changes in a real shell, install the binary and enable the shell
integration:

```sh
cargo install --path .    # installs `tth` into ~/.cargo/bin
tth install               # adds the eval line to your shell rc
exec $SHELL               # reload your shell (or open a new terminal)
```

The codebase lives under `src/` (the library plus the `tth` binary), with integration
tests in `tests/`.

## Issues

Before opening an issue, please search the existing issues (open and closed) to make sure
the feature or bug you want to propose does not already exist.

### Bugs

A bug report must include the following:

1. The version you are running (`tth --version`).
2. Exact steps to reproduce the bug.
3. A proposed fix or a hypothesis about the cause.

### Features

Feature issues are split into two kinds: **UI** and **Code**.

#### UI

UI issues are specific improvements to the user experience. The proposal must include at
least:

1. The section you want to improve or add.
2. An image or mockup of the result you want to reach.

#### Code

Code issues are improvements to the codebase itself, whether for better component handling
or structure. The proposal must include at least:

1. The section you want to improve or add.
2. A proposed solution.

## Pull Requests

To contribute, take one of the open issues in the repository. Anything tagged `type:bug`,
`good-first-issue`, or `help-wanted` would be fantastic. To claim an issue, leave a comment
asking for it and a maintainer will assign it to you.

Open your pull request against the `develop` branch (not `main`), and link the issue it
resolves.

### Quality bar

Thoth follows test-driven development: add or update tests alongside your change. Before you
submit, make sure the checks below pass locally, since the same ones are enforced on review:

```sh
cargo test
cargo clippy --tests -- -D warnings
cargo fmt --check
```

Production code avoids `.unwrap()` and `.expect()`: use the `ThothError` type and propagate
errors with `?`. Those calls are fine in tests.

### Review cycle

To speed up the review cycle, you can allow maintainers to push directly to your branch.
This is only done for small fixes.

### Commits

Commits must follow the [commit convention](docs/commit-convention.md). If a pull request
does not follow it, it will be rejected and you will be asked to correct the commit history.

## AI

We are not at odds with the use of AI. On the contrary, we push for more people to use it
to speed up the production process. That said, using AI correctly matters to us: every
change, and every issue and pull request description, must be tested and understood by a
human.
