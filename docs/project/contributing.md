# Contributing

Contributions are welcome: bug reports, change requests, documentation and
pull requests. The repository's
[`CONTRIBUTING.md`](https://github.com/aartintelligent/turso-orm/blob/main/CONTRIBUTING.md)
is the complete reference for tooling, quality gates, branches and commit
conventions; this page is the short version.

## Bug reports

Open an [issue](https://github.com/aartintelligent/turso-orm/issues/new/choose)
with the bug report template. The most useful report contains the entity
and the query that misbehave, the SQL you expected, the error or the
result you got, and the versions of turso-orm and the engine. A failing
test is even better: the suite runs against an in-memory database, so a
reproduction is a few lines.

## Change requests

For a new capability or an API change, open a change request before
writing code. Say what you are trying to do rather than how you would
implement it; the design is discussed in the issue, then the pull request
follows it. The [roadmap](../overview/roadmap.md) lists what is already
planned.

## Documentation

This site and the rustdoc are part of the project. A typo, a confusing
sentence or a missing explanation is a valid pull request. Code blocks on
the site are extracted from the examples, so a documentation change that
needs new code goes through the examples.

## Pull requests

1. Branch from `main`, keep the change focused.
2. Run `just ci` locally; it mirrors what CI runs.
3. Write commits in the Conventional Commits format, since the changelogs
   and the version numbers are generated from them.
4. Open the pull request with the template filled in and wait for a review.

## Security vulnerabilities

Do not open a public issue for a security problem. Use GitHub's private
vulnerability reporting on the repository (*Security* tab, *Report a
vulnerability*). You will get an acknowledgement within 72 hours and a fix
or a mitigation plan within 14 days for confirmed issues. Vulnerabilities in
the Turso engine itself belong to
[Turso's security process](https://github.com/tursodatabase/turso/security).
