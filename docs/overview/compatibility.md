# Compatibility

## Turso

turso-orm builds on the [`turso`](https://crates.io/crates/turso) crate,
the Rust client of the Turso database, for in-memory and file databases
and embedded replicas, and on
[`turso_serverless`](https://crates.io/crates/turso_serverless) for Turso
Cloud over HTTP. Each release of turso-orm pins the versions it was
developed and tested against; the current ones are listed in the workspace
manifest. Both clients are pure Rust, so there is no C toolchain to install
on any platform.

| Where the data lives | Open it with | Feature |
|---|---|---|
| In memory | `ConnectOptions::in_memory()` | |
| A local file | `ConnectOptions::new(path)` | |
| Turso Cloud, with a local replica | `ConnectOptions::sync(path, url).auth_token(token)` | `sync` |
| Turso Cloud, over HTTP | `ConnectOptions::remote(url).auth_token(token)` | `serverless` |

The same entities, queries, transactions and migrations work against all
four; only the connection options change.

Behaviour of the engine that an application should know about, such as the
single active writer per connection or the pragmas behind `BEGIN
CONCURRENT`, is listed on the [design page](design.md).

## Rust

| | |
|---|---|
| Minimum supported Rust version | 1.94, edition 2024 |
| MSRV policy | Raising it is a minor version change while the crates are `0.x` |
| Tested platforms | Linux, macOS and Windows on every change |
| `unsafe` | Forbidden across the workspace |

## Feature flags

| Feature | Default | Enables |
|---|---|---|
| `macros` | on | The derive macros, re-exported through the prelude |
| `with-chrono` | on | `NaiveDate`, `NaiveTime`, `NaiveDateTime`, `DateTime<Utc>` and `DateTime<FixedOffset>` columns |
| `with-json` | on | `serde_json::Value` columns |
| `with-uuid` | on | `uuid::Uuid` columns |
| `with-rust_decimal` | off | `rust_decimal::Decimal` columns |
| `fts` | off | Full-text search functions of the engine |
| `sync` | off | Embedded replicas of Turso Cloud: a local file kept in sync with a cloud database |
| `serverless` | off | Turso Cloud over HTTP, with no local file, through the `turso_serverless` client |
| `mimalloc` | off | The engine's preferred global allocator |

Every feature combination compiles on its own; CI checks the power set.

## Versioning

The five crates share one version and are released together. Breaking
changes bump the minor version while the crates are `0.x` and are listed
in each crate's changelog, which the release tooling generates from the
commit history.
