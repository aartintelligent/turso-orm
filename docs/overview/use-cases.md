# Use cases

Turso runs inside your process, so turso-orm fits wherever a program needs
a real SQL database without running a server next to it.

## Desktop and command-line tools

A CLI that keeps state, a desktop application with a local catalogue, a
background agent with a job queue: the database is a file next to the
binary, opened with one line, and the schema evolves through migrations
shipped with the program. There is no daemon to start and nothing for the
user to configure.

## Web services with a local database

A small service, an internal tool or a prototype often needs less than a
database server. The web examples show a JSON API on axum or actix-web
backed by a Turso file: migrations run at start-up, the pool lives in the
application state, and the service layer is tested against an in-memory
database without opening a port.

[:octicons-arrow-right-24: The web examples on GitHub](https://github.com/aartintelligent/turso-orm/tree/main/examples)

## Turso Cloud, with or without a disk

With the `sync` feature, a process opens an embedded replica of a Turso
Cloud database: reads are local and fast, writes are forwarded and synced
back. With the `serverless` feature, a process with no disk at all, such as
a function at the edge, talks to the same database over HTTP. The entities
and queries are the same in both cases; only the connection options
differ.

## Search-heavy applications

Turso adds vector distance functions and full-text indexes to SQLite. The
SQL builder exposes them, so a semantic search or a text search stays a
query on the same table as the rest of the data, inside the same
transaction.

## AI agents

An agent needs memory that outlives one prompt: the conversation so far,
the facts it extracted, the tools it called and what they returned, the
documents it may search. That is a relational problem with a vector
search on the side, and an embedded database answers it without a second
service to deploy next to the model.

- **Memory as entities.** Sessions, messages, extracted facts and tool
  calls are ordinary tables with relations between them, so "the last ten
  messages of this session with their tool results" is one
  `find_with_related` rather than a hand-written join.
- **Structured output straight into the database.** A model that answers
  in JSON is one `ActiveModel::from_json` away from a row; the attribute
  types decode the values, and a wrong one is an error rather than a
  corrupt record. A `Json` column keeps a tool's raw payload next to the
  typed fields.
- **Retrieval in the same query.** Embeddings live in a column and
  `vector_distance_cos` ranks them; a full-text index covers the rest. The
  retrieval step and the bookkeeping write run in one transaction, with no
  round trip to a separate vector store.
- **One database per agent, or shared.** A short-lived agent opens an
  in-memory database and throws it away; a long-lived one keeps a file; a
  fleet of agents shares a Turso Cloud database through embedded replicas,
  or over HTTP from the edge. The entity code does not change.

The repository is also set up for coding agents: `AGENTS.md` and the
skills under `.claude/skills` describe the architecture, the conventions
and the gates, so an agent extending your schema works from the same
instructions a contributor reads.

## Tests

Every test can open its own in-memory database in a few milliseconds, run
the migrations, and throw it away. The test suite of this repository works
that way, and so can yours.

[:octicons-arrow-right-24: Quickstart](../guide/quickstart.md){ .md-button .md-button--primary }
