## Summary

<!-- What does this change and why? Link the issue it closes, e.g. "Closes #12". -->

## Checklist

- [ ] `just ci` passes locally (fmt, clippy, tests, doctests, cargo-deny, examples)
- [ ] Behaviour changes come with tests
- [ ] Public API changes are documented (rustdoc, README and examples when user-facing) and called out above
- [ ] Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (see `CONTRIBUTING.md`)
- [ ] Breaking changes are marked with `!` in the commit type (e.g. `feat(orm)!:`)
