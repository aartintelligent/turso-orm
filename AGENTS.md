# AGENTS.md

Guidance for AI coding agents working in this repository. `CLAUDE.md` is a symlink to this file, and
`.codex/skills` links to `.claude/skills`, so every tool reads the same instructions. Deeper, on-demand
material lives in `.claude/skills/*/SKILL.md`.

## What this is

A from-scratch async ORM dedicated to the Turso database (SQLite-compatible, pure Rust), with a typed
entity API. **Never copy code from another ORM, and never name one** in code, comments, docs or
commits: the project stands on its own (project rule). The `turso` sources are cloned at v0.8.1 in
`~/workspace/codebase/turso` for API verification. `docs/overview/design.md` on the site records the layering and the
decisions that differ from a multi-database ORM; the Turso v0.8.1 quirks that matter when coding are
listed under "Conventions and gotchas" below.

Five crates, each depending only on the ones below:

- `crates/turso-sql`: SQL AST + builder, no runtime deps, one dialect (SQLite + Turso extensions,
  no backend enum). `Value` = 5 storage classes. `writer.rs` renders; its inline tests assert exact
  SQL text.
- `crates/turso-orm-driver`: `Database` pool (`database.rs`, hands out `connect()`ed connections,
  never clones), `Transaction` with savepoints and deferred rollback (`transaction.rs`), `Row` +
  `FromValue` lenient typed decoding over `turso_sql::Value` (`decode.rs`), structured `Error` with
  `ErrorKind` / `ConstraintKind` classified from the engine error text (`error.rs`), traits in
  `connection.rs`. `executor.rs` holds the engine enum `Conn` (embedded `turso`, or
  `turso_serverless` behind the `serverless` feature) and generates the primitives once per engine
  with the `engine_module!` macro; nothing above it sees which engine answered.
- `crates/turso-orm-macros`: proc macros, all under the `#[turso(...)]` attribute namespace.
  `entity.rs` generates `Entity`, `Column`, `PrimaryKey`, `ActiveModel` (plus `TryIntoModel`) from
  `Model`; `relation.rs` generates `RelationTrait` + one `Related<R>` impl per distinct target,
  with `via` for junction tables; `active_enum.rs`, `partial_model.rs` and `into_active_model.rs`
  are the smaller derives. Generated code refers to `::turso_orm::...` and reaches helpers
  through `turso_orm::__private`, so the macros are only usable via the re-exports in `turso-orm`
  (feature `macros`) and `turso-orm-migration`.
- `crates/turso-orm`: `entity/` (traits, `ActiveValue`, `ActiveEnum`, `PartialModelTrait`,
  `Linked`, `Schema`, loaders), `query/` (`Select`, `SelectTwo`, `SelectTwoMany`, `Cursor`,
  `Insert`, `Update`, `Delete`, `Paginator`), `types.rs` (`TursoType`: field type →
  `ColumnType`), `prelude`.
- `crates/turso-orm-migration`: `MigratorTrait`, `SchemaManager` (runs each migration plus its
  bookkeeping insert in one `BEGIN IMMEDIATE` transaction), `DeriveMigrationName` takes the version
  from the module path (`m20240101_000001_create_user`), so every struct is just `Migration`.
  `down` defaults to failing with `DbErr::Migration`.

`CONTRIBUTING.md` is the source of truth for the workflow (gates, hooks, branches, commits, pull
requests) and holds the notes for AI agents; this file covers architecture and conventions.

## Working mode and attribution

- Routine changes that stay within the written conventions proceed on their own. A design decision
  (new dependency, public API change, new module layout, pattern or convention) is proposed to the
  maintainer and validated before implementation.
- Commit messages, PR titles, descriptions and review comments never mention the AI tooling: no
  "Claude", no `Co-Authored-By` trailer, no "Generated with" footer. This rule overrides any default
  attribution guidance; authorship is only the configured git user.
- Never weaken a workspace lint, `clippy.toml`, `deny.toml` or a test to make something pass.

## Commands

```bash
just                    # list recipes
just setup              # lefthook install (pre-commit: fmt + taplo + clippy; commit-msg: committed)
just test               # cargo nextest run --workspace --all-features (falls back to cargo test)
just doctest            # doctests (nextest does not run them)
just clippy             # clippy, all targets/features, -D warnings
just fmt / fmt-check    # rustfmt + taplo
just doc                # nightly rustdoc with --cfg docsrs and -D warnings, as docs.rs builds it
just deny               # cargo deny check
just hack               # cargo hack feature powerset (depth 2, no dev-deps)
just msrv               # cargo check on the MSRV from Cargo.toml
just ci                 # fmt-check + clippy + test + doctest + deny + examples
just example            # cargo run -p turso-orm --example quickstart, then relations
just examples           # fmt-check + clippy + test of every examples/* workspace, then runs basic
just docs-serve         # live preview of the documentation site (Zensical through uv)
just docs-build         # strict build of the site, as CI runs it

# Single integration test (file, then test name as filter)
cargo nextest run -p turso-orm --test entity relations_and_loaders
cargo test -p turso-orm --test entity -- relations_and_loaders
cargo test -p turso-orm-migration --test migrate
cargo test -p turso-orm-driver --test driver
# Inline unit tests (turso-sql has no tests/ dir; its tests live in writer.rs and value.rs)
cargo test -p turso-sql
```

Tests run against a real in-memory Turso database; no services needed. Each test opens its own
`Database` via `ConnectOptions::in_memory()` so they run in parallel. Entity tests live in
`crates/turso-orm/tests/entity.rs` (user / post / tag entities covering CRUD, queries, relations,
loaders, composite keys, transactions) and `crates/turso-orm/tests/relational.rs` (many-to-many,
self-reference, several relations to one entity, `Linked`, enums, partial models, request
structs, JSON, cursor, projections, `RETURNING` deletes, wide composite keys); each has a
`setup()` helper that creates the schema from the entities. `crates/turso-orm/examples/relations.rs`
is the runnable counterpart the site includes snippets from.

## Examples

`examples/*` are standalone Cargo workspaces (excluded from the root one via `exclude`): `basic` is a single console crate with `entity/`, `query.rs` and
`mutation.rs`; `axum_example` and `actix_example` each hold `entity`, `migration` (with a tiny
`up`/`down`/`status` CLI binary) and `api` (handlers + `service::{Query, Mutation}` + a
`tests/crud_tests.rs` against an in-memory database). They depend on the ORM by `path`, so a
library change must keep them compiling: run `just examples` after touching public API.

## Documentation site

`mkdocs.yml` + `docs/` is a Zensical site (the Material theme's successor, which reads the MkDocs-format
config as is) published to GitHub Pages by `docs.yml`; the Python toolchain is pinned by `pyproject.toml`
/ `uv.lock` and driven through `uv`. Overrides are MiniJinja templates: no arbitrary Python calls. Rustdoc stays the API
reference; the site is the guide. Code blocks are never pasted: pages include them with
`--8<-- "path:name"` from the examples, between `// --8<-- [start:name]` / `[end:name]` marker
comments. Those markers are the one accepted exception to the inline-comment rule. `strict: true`
makes a broken link or a missing marker fail the build, so run `just docs-build` after moving code
that a page includes.

## CI gates beyond `just ci`

`.github/workflows/ci.yml` also runs these; mirror them locally before pushing. The `changes` job sizes
each run: documentation-only changes skip the build jobs, pushes to `main` and `release-plz-*` pull
requests run a reduced set (Clippy, Linux tests, docs, cargo-deny, coverage), and a pull request
touching `crates/`, `examples/`, a manifest, a lint config or a workflow runs all of them; `ci-ok`
accepts skipped jobs:

- **Docs on nightly with `-D warnings`** (`just doc`) and `rustdoc::broken_intra_doc_links = deny`:
  every `[`link`]` in `///` must resolve. Feature-gated public items need
  `#[cfg_attr(docsrs, doc(cfg(feature = "...")))]`.
- **Feature powerset** (`just hack`): every feature pair must compile alone, so never rely on a
  default feature being present inside library code.
- **MSRV 1.94** (`just msrv`): `rust-toolchain.toml` pins 1.97.1 for development, but code must
  compile on 1.94. Do not use std APIs or language features stabilized after 1.94.
- **typos** (`typos.toml`), **cargo-semver-checks** on PRs, and a cross-platform test matrix
  (Linux, macOS, Windows).

## Feature flags

`turso-orm` defaults: `macros`, `with-chrono`, `with-json`, `with-uuid`. `with-rust_decimal` is
opt-in. Each `with-*` feature chains down three crates (`turso-orm` → `turso-orm-driver` →
`turso-sql`): `turso-sql` adds `Value` conversions, the driver adds `FromValue` decoding, the ORM
adds the `TursoType` column mapping. Adding a value type means touching all three. `fts`,
`mimalloc` and `sync` forward to the `turso` client, whose defaults are off on purpose; `serverless`
pulls in `turso_serverless` and `reqwest`. `chrono`
deliberately excludes the `clock` feature so `Local` never enters the API.

## Comment conventions (same as the user's other projects)

- English only, full sentences ending with a period, code in backticks, em dash for asides.
- TOML, justfile, YAML, dotfiles: open with an 80-column banner block (`# ` + 78 hyphens, a
  `# <Name> — <role>.` line, optional paragraphs separated by a bare `#`, closing banner, blank
  line). Comments sit on their own line above the key they describe, never inline. `=` aligned.
- Root `Cargo.toml`: 3-line banner blocks separate "Workspace dependencies", "Quality gates" and
  "Build profiles". Every `[workspace.dependencies]` entry carries a paragraph saying what it is
  for, which crate needs it and why its feature set is what it is. Member manifests comment only
  non-obvious dependencies and put `# Quality gates are centralized in the workspace root
  \`Cargo.toml\`.` above `[lints]`.
- Rust: no banners inside files. Every file opens with a `//!` header (one-sentence summary, blank
  `//!`, then why / ownership paragraphs). File order: header → `mod`/`pub use` → `use` groups
  (std, external, crate) → documented private consts → types → impls → `#[cfg(test)]` last.
- `///` on every item including private ones, fields and variants; third-person summaries
  ("Builds …") or noun phrases; `# Errors` lists every variant as "Returns [`E::V`] when …";
  `# Panics` starts with "When …". Error messages are lowercase without trailing period.
- Inline `//` comments explain why or the hazard, on their own line above the block. `#[allow]`
  carries `reason = "…"`. No TODO/FIXME markers. One-line `///` above every test stating the
  behaviour.

## Conventions and gotchas

- Workspace lints: `unsafe_code = forbid`, `missing_docs`, `unreachable_pub`, clippy `pedantic`,
  `unwrap_used` / `todo` / `print_*` warn; CI runs with `-D warnings`. Tests may `expect`/`unwrap`
  (clippy.toml), but integration-test helper fns outside `#[test]` are still linted: use `expect`.
- Generated `Column` enums must NOT derive `PartialEq`, otherwise `Column::X.eq(v)` resolves to
  `PartialEq::eq` instead of the condition builder.
- `ColumnTrait` builders produce qualified `table.column` refs so joins never clash.
- `find_also_related` aliases columns `A_<col>` / `B_<col>`; `FromQueryResult::from_query_result`
  takes that prefix. `from_query_result_optional` returns `None` when all prefixed columns are NULL.
  The many-to-many loader rides the junction columns along as `__via_<col>`.
- `Select::join_table` aliases a table already in the query as `table_n` and renders the `ON`
  against that alias (self-joins, `Linked` chains); `join_as` takes an explicit alias.
- One `Related<Target>` impl per entity: the first `Relation` variant naming a target wins, the
  others are used through `Relation::X.def()` with `Select::related_to` / `join`. A `via`
  relation's `def()` is the entity-to-junction hop (not an owner, so no foreign key).
- `contains` / `starts_with` / `ends_with` render `LIKE ? ESCAPE ?` with a backslash escape; `like`
  takes the pattern as is. `Expr::Like` is its own node, not a `BinOp`.
- `InsertMany` fills a column some models leave unset with the entity's declared default
  (`ColumnDef::default`), else `NULL`; `#[turso(ignore)]` fields decode to `Default::default()`.
- `FromValue` is lenient like SQLite (integer→bool, text→date/UUID/JSON, integral real→integer),
  but `Option<T>` maps `NULL` only: a wrong type is an error, never a silent `None`.
- DDL defaults are inlined as literals (SQLite does not bind parameters in DDL). DDL writes the
  four SQLite storage types so generated tables are `STRICT`-compatible.
- Proc-macro errors must carry the span of the offending user item (`syn::Error::new_spanned`),
  never the macro call site.
- Turso 0.8.1: one active write statement per connection and no sharing of a connection between
  tasks (the pool enforces it); `PRAGMA journal_mode = 'mvcc'` enables `BEGIN CONCURRENT` with
  conflicts surfacing at commit as busy errors; `CREATE VIEW IF NOT EXISTS` is not idempotent;
  `PRAGMA defer_foreign_keys` and `foreign_key_check` are unsupported; `WITHOUT ROWID` and generated
  columns sit behind `ConnectOptions::experimental`. Over HTTP (`ConnectOptions::remote`) the busy
  timeout and MVCC do not apply; the remote end-to-end test in `crates/turso-orm-driver/tests/remote.rs`
  needs `TURSO_DATABASE_URL` and `TURSO_AUTH_TOKEN` and skips without them.
- Conventional Commits drive `release-plz`; all crates share one version (`version_group`), and
  `chore`/`ci`/`build`/`test`/`style` commits are skipped from changelogs.
