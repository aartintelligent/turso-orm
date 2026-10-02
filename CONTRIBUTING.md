# Contributing

Thanks for contributing to `turso-orm`, an async ORM dedicated to the Turso
database. This guide covers how to set up the repository, the development
rules, the commit conventions and how a change is validated before it is
merged. It applies to both human contributors and AI coding agents.

> Architecture decisions, code conventions and their rationale live in
> [`AGENTS.md`](./AGENTS.md); the [design page](https://aartintelligent.github.io/turso-orm/design/) of the guide records the layering.
> Read them before writing code. This file is the source of truth for the
> workflow: tooling, gates, branches, commits and pull requests.

## Prerequisites

- **Rust**, managed with [rustup](https://rustup.rs). `rust-toolchain.toml`
  pins the development toolchain (with `clippy` and `rustfmt`); rustup
  provisions it on the first `cargo` invocation. The crates must also build
  on the MSRV declared as `rust-version` in `Cargo.toml`, which CI checks
  separately.
- **[just](https://just.systems)**: the `justfile` wraps every command below
  into short recipes; `just` alone lists them.
- **[lefthook](https://lefthook.dev)** (git hooks manager) and
  **[committed](https://github.com/crate-ci/committed)** (Conventional
  Commits linter): `cargo install committed`, lefthook via npm, brew or a
  binary release.
- **[uv](https://docs.astral.sh/uv/)** for the documentation site: it
  provisions Python and [Zensical](https://zensical.org) from
  `pyproject.toml` / `uv.lock` on the first `just docs-serve`, locally to
  the repository.
- For the full local CI run: `cargo-nextest`, `cargo-deny`, `cargo-hack`,
  `taplo-cli`, `typos-cli` and a nightly toolchain for the rustdoc build
  (`cargo install cargo-nextest cargo-deny cargo-hack taplo-cli typos-cli`,
  `rustup toolchain install nightly`).

`.agents/setup` installs all of the above that are missing and the git
hooks; cloud agents run it, humans may too.

No external service is needed: every test runs against a real in-memory
Turso database.

## Getting started

```sh
git clone https://github.com/aartintelligent/turso-orm && cd turso-orm
just setup                # installs the git hooks
cargo build --workspace   # first build provisions the pinned toolchain
just test
```

`just setup` is `lefthook install` and `uv sync --group docs`. Without
`just`, run them by hand; the hooks are not optional, since `commit-msg` is
what keeps the history conforming to the release tooling.

## Quality gates

Every gate below must stay green. `just ci` chains the local ones in order;
CI runs them all on every pull request and push to `main`, and `ci-ok` is the
single required status check.

| Gate | Recipe | What it guards |
|---|---|---|
| Format | `just fmt-check` | `cargo fmt` and `taplo fmt`, policies in `rustfmt.toml` and `.taplo.toml` |
| Lint | `just clippy` | Workspace lints and `clippy::pedantic`, warnings are errors |
| Tests | `just test`, `just doctest` | The suite on Linux, macOS and Windows, plus the doctests |
| Docs | `just doc` | Nightly rustdoc with the docs.rs flags, warnings and broken links are errors |
| Supply chain | `just deny` | Advisories, licences, bans and sources (`deny.toml`) |
| Features | `just hack` | Every feature pair compiles on its own |
| MSRV | `just msrv` | The workspace compiles on `rust-version` |
| Examples | `just examples` | Each example workspace is formatted, linted and tested; `basic` runs |
| Site | `just docs-build` | The documentation site builds in strict mode: no broken link, no missing snippet |
| Spelling | `typos` | Sources and docs (`typos.toml`) |
| API | CI only | `cargo-semver-checks` flags breaking changes on pull requests |
| Commits | CI only | `committed` validates every commit of a pull request |

While iterating, `cargo check --workspace` and a single test are the fast
inner loop:

```sh
cargo nextest run -p turso-orm --test entity relations_and_loaders
cargo test -p turso-orm --test entity -- relations_and_loaders
```

## Repository layout

```
crates/turso-sql/            SQL AST and builder for the Turso / SQLite dialect
crates/turso-orm-driver/     connection pool, transactions, typed rows over the `turso` crate
crates/turso-orm-macros/     derive macros (DeriveEntityModel, DeriveRelation, DeriveActiveEnum, ...)
crates/turso-orm/            entities, active models, queries, relations, chains, loaders, schema
crates/turso-orm-migration/  migrations (MigratorTrait, SchemaManager)
examples/                    standalone workspaces: basic, axum_example, actix_example
docs/                        the documentation site (Zensical), built from mkdocs.yml
```

Each crate depends only on the ones above it in that list. All crates share
the version, edition, MSRV, licence and lints declared in the root
`Cargo.toml` (`[workspace.package]`, `[workspace.lints]`) and are released
together. Every external dependency version and feature set lives once in
`[workspace.dependencies]`; a member crate never declares a version of its
own, and each entry carries a comment stating which crate needs it and why
its feature set is what it is.

The examples are separate workspaces, excluded from the root one so that
their web frameworks never enter the library's dependency graph. They depend
on the crates by `path`, which makes them a compile-time check of the public
API: run `just examples` after changing one. Their `.env` files are
committed on purpose; they hold a listen address and a database file path,
never a secret.

## Development rules

The workspace lints in the root `Cargo.toml` are the contract. Never silence
one locally without a `reason`, and never weaken a lint, `clippy.toml`,
`deny.toml` or a test to make something pass: surface the conflict instead.

- `unsafe_code = "forbid"`: no unsafe code anywhere.
- `missing_docs`: every public item carries English rustdoc and every file a
  `//!` header. Private items are documented too; see the comment
  conventions below.
- `clippy::pedantic` plus `unwrap_used`, `todo`, `dbg_macro` and `print_*`
  as warnings, with `-D warnings` in CI. Tests may `unwrap` and `expect`
  (`clippy.toml`).

Design rules:

- **Other ORMs are not a source.** Do not copy code from another project
  into this repository; write it for this engine.
- **Verify APIs against the pinned versions** (`turso` 0.8.1 above all)
  before using them; do not code from memory of other releases.
- **No abstraction without a demonstrated need**, and no second code path
  for a database this project does not target: there is one dialect.
- **Public API changes are deliberate.** They are flagged by
  `cargo-semver-checks`, described in the pull request and reflected in the
  README or the examples when user-facing.

## Comments and documentation

- Everything is written in English, in full sentences that end with a
  period, with code in backticks and the em dash for asides.
- **TOML, justfile, YAML and dotfiles** open with a header comment block (an
  80-column banner of hyphens, the file's role, a closing banner).
  Strategic comments sit on their own line above the key they describe,
  never inline. Values are aligned on `=`.
- **Rust files** contain no banner comments. Every file carries a `//!`
  header explaining what it owns and why; every item, public or private,
  carries `///` with a third-person summary; every fallible function lists
  its error variants under `# Errors`; panics are documented under
  `# Panics`. Error messages are lowercase without a trailing period.
- **Inline comments** explain the why or the hazard, on their own line above
  the block. Lint allowances carry a `reason`. No `TODO` or `FIXME`
  markers: unfinished work is an issue, not a comment.
- **Every test** has a one-line `///` stating the behaviour it pins.

## Testing

```sh
just test                                            # the whole suite (nextest)
cargo test -p <crate>                                # one crate
cargo test -p <crate> --test <file> -- <name>        # one test, substring match
cargo test -p <crate> --doc                          # the doctests of one crate
```

House rules:

- Unit tests sit in a `#[cfg(test)]` module at the end of the file they
  test; `turso-sql` asserts the exact SQL text its writer renders.
- Integration tests live under `crates/<name>/tests/` and open their own
  in-memory database (`ConnectOptions::in_memory()`), so they run in
  parallel and never share state. Nothing is `#[ignore]`d and nothing needs
  a service.
- Behaviour changes come with a test; bug fixes come with the test that
  would have caught them.
- The one test that needs a network, the serverless round trip in
  `crates/turso-orm-driver/tests/remote.rs`, reads `TURSO_DATABASE_URL`
  and `TURSO_AUTH_TOKEN` and skips when they are unset. Point it at a
  scratch database; CI does not run it.
- Documentation examples are compile-checked doctests wherever possible.
- Tests never mutate the process environment.

## Documentation

Two layers, with different jobs:

- **Rustdoc** is the API reference, published on docs.rs. `missing_docs` is
  enforced and CI builds it on nightly with `-D warnings`.
- **The site** under `docs/`, rendered by Zensical (the Material for MkDocs
  team's generator, reading `mkdocs.yml`) and published to GitHub Pages, is
  the guide: getting started, entities, queries,
  relations, transactions, migrations, the examples. It is kept small on
  purpose: fold a topic into an existing page before adding one. `just docs-serve`
  previews it, `just docs-build` runs the strict build CI runs.

Pages never paste code. They include it from the examples through snippet
markers (`// --8<-- [start:name]` / `[end:name]` in the source,
`--8<-- "path:name"` in the page), so that every block on the site is
compiled and tested by `just examples`. When you move or rename a function a
page includes, move its markers with it and run `just docs-build`.

## Git hooks

Managed by [lefthook](https://lefthook.dev) (`lefthook.yml`); `just setup`
installs them:

- **`pre-commit`**: `cargo fmt --all --check`, `taplo fmt --check` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  in parallel. A formatting drift or any clippy warning blocks the commit.
- **`commit-msg`**: `committed` validates the message against
  `committed.toml` (Conventional Commits, allowed types, 100-column caps).

The heavier gates (tests, docs, `cargo-deny`, examples) are deliberately not
in `pre-commit` to keep commits fast; run `just ci` before pushing. Do not
disable the hooks; `git commit --no-verify` is for genuine emergencies only.

## Branching model

- `main` is the default branch and is protected; changes land through pull
  requests.
- Branch off `main` for every change: `feat/<slug>`, `fix/<slug>`,
  `docs/<slug>`, `chore/<slug>`.
- Keep branches focused and short-lived; rebase on `main` rather than
  merging it back in.
- `release-plz` maintains its own `release-plz-*` branch and pull request;
  never edit it by hand.

## Commit conventions

Commits **must** follow
[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).
`release-plz` reads them to compute the next version and write the
changelogs, so a non-conforming history is expensive to repair. The rule
applies from the very first commit.

```
<type>(<optional scope>): <short summary>

<optional body, wrapped at 100 columns>

<optional footer / BREAKING CHANGE: ...>
```

| Type | Use for | Release effect |
|---|---|---|
| `feat` | A new user-facing capability | minor bump, "Added" |
| `fix` | A bug fix | patch bump, "Fixed" |
| `perf` | A performance improvement | patch bump, "Performance" |
| `refactor` | A change that neither fixes a bug nor adds a feature | "Changed", no bump |
| `docs` | Documentation only | "Documentation", no bump |
| `deprecate` | Marks an API for removal | "Deprecated", no bump |
| `revert` | Reverts a previous commit | "Reverted", no bump |
| `test` | Adding or fixing tests | not listed |
| `build` | Build system, dependencies, packaging | not listed |
| `ci` | CI configuration | not listed |
| `chore` | Maintenance that fits nowhere else | not listed |
| `style` | Formatting, whitespace, no change in meaning | not listed |

Rules:

- Subject in the imperative and lowercase, not empty, no trailing period.
- Header (`type(scope): subject`) at most **100 characters**; body and
  footer lines wrapped at 100 columns.
- Scopes are the crate short names (`sql`, `driver`, `macros`, `orm`,
  `migration`), `examples`, `deps`, `ci` or `workspace`.
- Breaking changes carry `!` after the type or scope (`feat(orm)!:`) **or**
  a `BREAKING CHANGE:` footer. While the crates are `0.x` they bump the
  minor version.

Enforcement is mechanical: the `commit-msg` hook rejects a non-conforming
message locally and CI validates every commit of a pull request. The rules
live in `committed.toml` and the changelog mapping in `release-plz.toml`;
keep both in sync with this table.

## Pull requests

1. Run `just ci` locally; everything green.
2. Push the branch and open a pull request against `main`. Keep the title
   Conventional-Commit-shaped: a squash-merge title becomes the commit
   message the release tooling reads.
3. Fill the template: what changes and why, the issue it closes, and any
   breaking change or new public API.
4. Get a review from a code owner, then merge; keep the history
   conventional-clean.

## Releasing

Releases are automated. Merging to `main` lets `release-plz` open or update
a release pull request with the bumped versions and changelogs. Merging that
pull request publishes all five crates to crates.io and creates the GitHub
release and tags. The `CARGO_REGISTRY_TOKEN` secret must be set on the
repository.

## Notes for AI agents

- Treat `AGENTS.md` (which `CLAUDE.md` links to) as the source of truth
  for architecture, conventions and their rationale;
  this file is the source of truth for workflow and commit rules. Deeper
  guides live in `.claude/skills/`, shared with Codex through `.codex/skills`.
- Never weaken a workspace lint, `clippy.toml`, `deny.toml` or a test to
  make something pass; surface the conflict instead.
- Working mode: routine changes that stay within the written conventions
  proceed on their own. A design decision, meaning a new dependency, a
  public API change, a new module layout, pattern or convention, is
  proposed to the maintainer and validated before implementation.
- Never copy code from another project; write it for this engine.
- `.env` files of the examples are committed; `.claude/settings.local.json`
  is local and git-ignored. Never commit a credential anywhere.
- Commit messages, pull request titles, descriptions and review comments
  never mention the AI tooling: no "Claude", "Claude Code", `Co-Authored-By`
  trailer or "Generated with" footer. Authorship is always and only the
  configured git user of the machine; the history reads as the maintainer's
  work, whatever produced it.
- Verify library APIs against the pinned versions before use; do not code
  from memory of other releases.

## License

By contributing you agree that your contributions are licensed under the
MIT OR Apache-2.0 dual licence of this project.
