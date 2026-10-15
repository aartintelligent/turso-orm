# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.6...turso-orm-v0.1.7) - 2026-10-15

### Added

- *(driver)* add ReplicaSync and wait for a replica's write lock like a local file ([#62](https://github.com/aartintelligent/turso-orm/pull/62))
- *(driver)* trace every statement and transaction, and keep the example replica in sync ([#60](https://github.com/aartintelligent/turso-orm/pull/60))

## [0.1.6](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.5...turso-orm-v0.1.6) - 2026-10-15

### Documentation

- *(workspace)* describe the moving decoders and drop the version from the bug template ([#45](https://github.com/aartintelligent/turso-orm/pull/45))

### Performance

- *(orm)* count a plain selection in place instead of through a subquery ([#53](https://github.com/aartintelligent/turso-orm/pull/53))
- *(workspace)* move values out of owned rows and skip the pool timer when a permit is free ([#42](https://github.com/aartintelligent/turso-orm/pull/42))
- *(workspace)* cut the per-row and per-query allocations of the hot paths ([#40](https://github.com/aartintelligent/turso-orm/pull/40))

## [0.1.5](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.4...turso-orm-v0.1.5) - 2026-10-14

### Fixed

- *(driver)* require turso 0.8.3 for its mvcc correctness fixes ([#38](https://github.com/aartintelligent/turso-orm/pull/38))

## [0.1.4](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.3...turso-orm-v0.1.4) - 2026-10-03

### Added

- *(migration)* validate the declared migrations against the bookkeeping table ([#31](https://github.com/aartintelligent/turso-orm/pull/31))

### Documentation

- *(orm)* state when model hooks run and what an after hook error means ([#29](https://github.com/aartintelligent/turso-orm/pull/29))

### Fixed

- *(orm)* keep unchanged values when resetting an active value ([#25](https://github.com/aartintelligent/turso-orm/pull/25))
- *(driver)* match mvcc conflicts by their exact phrases ([#28](https://github.com/aartintelligent/turso-orm/pull/28))
- *(driver)* let only the innermost open transaction act ([#26](https://github.com/aartintelligent/turso-orm/pull/26))
- *(migration)* check each migration again under the write lock ([#27](https://github.com/aartintelligent/turso-orm/pull/27))

## [0.1.3](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.2...turso-orm-v0.1.3) - 2026-10-02

### Documentation

- plain README header and clearer crate README footers ([#17](https://github.com/aartintelligent/turso-orm/pull/17))

## [0.1.2](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.1...turso-orm-v0.1.2) - 2026-10-02

No user-facing change: the crates were republished after the workspace moved to this single
changelog.

## [0.1.1](https://github.com/aartintelligent/turso-orm/compare/turso-orm-v0.1.0...turso-orm-v0.1.1) - 2026-10-02

### Fixed

- build the documentation on docs.rs without the fts feature ([#8](https://github.com/aartintelligent/turso-orm/pull/8))

## [0.1.0](https://github.com/aartintelligent/turso-orm/releases/tag/turso-orm-v0.1.0) - 2026-10-02

### Added

- *(workspace)* add the turso-orm workspace

### Documentation

- rewrite the README around the product rather than the code ([#7](https://github.com/aartintelligent/turso-orm/pull/7))
