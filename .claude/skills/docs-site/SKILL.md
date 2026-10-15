---
name: docs-site
description: How the documentation site (Zensical with the Material theme, docs/ and zensical.toml, published to GitHub Pages) is built, how pages include code from the examples through snippet markers, and how to add or move a page. Load before editing anything under docs/, zensical.toml, or a function that a page includes.
---
# Documentation site

The site is the guide; rustdoc is the API reference. Do not duplicate rustdoc
content in a page and do not paste code: include it.

## Commands

```bash
just docs-serve      # live preview, http://127.0.0.1:8000
just docs-build      # strict build: broken link or missing snippet = failure
```

Both go through `uv run --group docs zensical`, so nothing is installed
globally. The configuration is `zensical.toml`, Zensical's native TOML format
(a `[project]` table; `taplo` formats it like the manifests); keep configuration
examples in that format and never reintroduce a `mkdocs.yml`. `theme.variant`
is `modern`; `classic` restores the Material for MkDocs look if a style
regression ever needs it. Template
overrides are MiniJinja: filters and tests only, no Python function calls,
and the `pymdownx.emoji` extension must point at `zensical.extensions.emoji`.

## Layout

```
zensical.toml                  Zensical configuration: theme, extensions, nav (Home, Overview, Documentation, Performance, Project)
overrides/home.html            the whole landing page (the engine version from `extra.turso_version` in zensical.toml sits between the independence note and the buttons, plain text; `report` maintains the value): a full-viewport hero with the text above the code window in a column centred on the page, over a canvas, in the theme's `hero` block; links home.css and home.js itself; content and footer blocks are empty; without JavaScript the hero is the text and the code window on the plain background
docs/assets/javascripts/home.js  the hero animation: fourteen tables as clusters of rows on two wide ellipses in a turning 3D space that spans the screen, foreign keys as links, queries as pulses walking the relations, a field of distant rows behind; no labels; 2D canvas with additive compositing, no library; the knobs (camera, stretch, tilt, spin, field, query cadence, depth, cooling) are the constants at the top; honours `prefers-reduced-motion` with a single still frame
docs/assets/stylesheets/brand.css  the turso.tech palette (Aqua #4FF8D2, Dark Teal #183134, Bluewood #293945, Mirage #162129, Bunker #0D1318) mapped onto Material's variables; `primary`/`accent = "custom"` in zensical.toml hand the colors to it
docs/assets/stylesheets/home.css  loaded by home.html only (not in extra_css); styles the hero, its canvas and the centred column (a veil under the column on wide screens, a vertical one below 76.25em, keeps the text readable over the schema) and hides `.md-content`, never `.md-main`, which holds the mobile navigation drawer
docs/assets/stylesheets/extra.css layout fixes for every page on phones: table padding and code size, no heading permalink on touch screens, `.tx-diagram` wrapper that lets a wide Mermaid diagram scroll at its natural size
docs/assets/turso/             the illustrated Turso logomark from https://turso.tech/brand (site logo and favicon), the site name is "Turso ORM"; keep the independence note next to it
docs/index.md                  front matter only (template: home.html); the landing page has no Markdown body
docs/overview/                 presentation pages, no code: why, features, use-cases, design, compatibility, roadmap
docs/performance/              the Performance tab: `explain.md` (hand-written, how to read the tables) and one generated `turso-<version>.md` per engine version, written by `benches/src/bin/report.rs`, which also adds the nav entry under the `# report: versions` comment of zensical.toml; never edit the generated pages or that list by hand
docs/guide/quickstart.md       quickstart; includes `crates/turso-orm/examples/quickstart.rs` (the `agent` entity alone)
docs/guide/*.md                entities, queries (with error handling), relations, transactions, migrations; every block comes from `examples/common` (`entity/src/resource/*.rs`, `migration/src/resource/*.rs`) or `examples/basic` (`src/service/*.rs`, `src/dto/*.rs`, `src/error.rs`, `src/handler/agents.rs`, `tests/api.rs`)
docs/project/                  about, contributing, community, license
```

Every page in `nav` must exist and every page must be in `nav`; `strict`
reports both. The home page is the hero alone, like zensical.org: no
sections below it. The Overview and Project tabs are prose: they explain
and link, they do not show code. Code belongs under Documentation. The
examples are not documented on the site: pages link to the `examples/`
directory on GitHub and include code from it, nothing more. The site is deliberately small: fold a new topic into an
existing page before adding one, and never add a page that only links
elsewhere.

## Including code

In the Rust source, wrap the function (doc comment included) with markers:

```rust
// --8<-- [start:relations]
/// Walks relations in both directions.
async fn relations(db: &Database) -> Result<(), DbErr> { /* ... */ }
// --8<-- [end:relations]
```

In the page:

````markdown
```rust
--8<-- "examples/basic/src/service/agents.rs:relations"
```
````

Rules:

- Markers wrap whole items so that the block compiles on its own in the
  reader's head. Indent the markers like the item (inside an `impl`, four
  spaces); rustfmt keeps them in place.
- A whole file is included without a marker: `--8<-- "path/to/file.rs"`.
- Line ranges (`file.rs:14:66`) are acceptable only for a file that is
  entirely an example, such as `crates/turso-orm/examples/quickstart.rs`.
- Markers are the single accepted exception to the "inline comments explain
  why" rule; do not add other tooling comments.
- When moving or renaming an included function, move its markers and run
  `just docs-build`.

## Turso branding

turso-orm is independent of Turso. Every place that shows the Turso logo or
name prominently (hero, the Turso card on the home page, README, footer
copyright) carries the sentence "not affiliated with or endorsed by Turso".
Keep it when editing those places. Logo files come from the brand kit and
are not modified; the palette in `brand.css` mirrors the brand page. The
site is dark only (a single `slate` palette, no toggle), like turso.tech;
do not reintroduce a light scheme without the maintainer's say.

## Writing style

Same as the rest of the repository: English, full sentences, code in
backticks. Use `!!! note` / `!!! warning` admonitions for Turso-specific
behaviour, tabs when several variants are shown, and a table for option
lists. Mermaid `erDiagram` blocks render
without a plugin.
