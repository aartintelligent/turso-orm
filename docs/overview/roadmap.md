# Roadmap

This page says where the project stands and what comes next, so that you
can decide whether to build on it today.

## Status

`0.1` is usable for applications that need entities, CRUD, relations,
transactions and migrations against an in-memory or file database, an
embedded replica of Turso Cloud, or Turso Cloud over HTTP.
The API may still move where Turso's own features ask for a different
shape; such changes are breaking changes and are called out in the
changelogs.

## What is planned

| Area | Intent |
|---|---|
| Command-line tool | Run migrations and generate entity modules from an existing schema without writing a binary for it. |
| Search helpers | Typed query helpers over the vector and full-text functions that the SQL builder already renders. |
| Nested writes | Inserting a model together with its related rows in one call, on top of the relations the ORM already walks. |

## What is not planned

- A second database engine. The project exists because it targets one.
- A query language of its own. The builder renders SQL, and raw SQL stays
  one call away.

## How to influence it

Open a discussion or a change request on
[GitHub](https://github.com/aartintelligent/turso-orm/issues); the
[contributing page](../project/contributing.md) explains how a request
becomes a change.
