# Basic example

A console program walking through the entity API against an in-memory
Turso database: schema generation, inserts, updates, deletes, filters,
pagination, streaming, relations in both directions and the batch loaders.

No setup is needed. Run it with:

```sh
cargo run
```

The output prints each model as it is read back, so the program doubles as
a reading guide for `src/query.rs` and `src/mutation.rs`.
