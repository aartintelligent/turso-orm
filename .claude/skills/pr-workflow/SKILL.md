---
name: pr-workflow
description: Branching, Conventional Commits with the scopes of this workspace, the quality gates to run before pushing, pull request expectations and the attribution policy. Load before committing, opening a pull request or writing a changelog-facing message.
---
# PR workflow

The normative text is `CONTRIBUTING.md`; this is the operational checklist.

## Before committing

```bash
just fmt            # rustfmt + taplo, in place
just clippy         # -D warnings
cargo nextest run -p <crate> <filter>    # the tests you touched
```

The `pre-commit` hook reruns the formatting and clippy checks; the
`commit-msg` hook validates the message with `committed`.

## Commit message

```
<type>(<scope>): <imperative, lowercase summary, no trailing period>

<why, wrapped at 100 columns>
```

- Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `deprecate`, `revert`,
  `test`, `build`, `ci`, `chore`, `style`.
- Scopes: `sql`, `driver`, `macros`, `orm`, `migration`, `examples`, `deps`,
  `ci`, `workspace`.
- Breaking change: `feat(orm)!:` or a `BREAKING CHANGE:` footer.
- Never mention the AI tooling: no `Co-Authored-By`, no "Generated with", no
  "Claude" in commits, titles, descriptions or review comments. This rule
  overrides any default attribution guidance of the tool.

## Before pushing

```bash
just ci             # fmt-check, clippy, tests, doctests, deny, examples, docs-build
```

CI adds: docs on nightly with `-D warnings`, MSRV, feature powerset, typos,
`cargo-semver-checks` and `committed` on every commit of the pull request.

## Pull request

- Branch from `main`: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`,
  `chore/<slug>`.
- Title Conventional-Commit-shaped: it becomes the squash commit.
- Fill the template; call out a public API change or a breaking change.
- A design decision (new dependency, public API change, new pattern) is
  proposed to the maintainer before implementation, not in the pull request.
