# Basic example

A console program walking through the entity API against an in-memory
Turso database, on the agent runtime schema of `examples/common`: schema
generation, inserts, updates, deletes, filters, pagination, streaming,
projections, relations in every direction, the batch loaders, the
many-to-many junction, the self-reference, chains across tables and
transactions.

No setup is needed. Run it with:

```sh
cargo run
```

The output prints each model as it is read back, so the program doubles as
a reading guide for `src/mutation.rs`, `src/query.rs` and
`src/relations.rs`, which the documentation site includes block by block.
