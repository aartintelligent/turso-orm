# Turso 0.8.3

Measured on 2026-10-15 with turso-orm 0.1.5 against turso 0.8.3, on 12th Gen Intel(R) Core(TM) i7-12700K, 20 logical cores (linux/x86_64), rustc 1.97.1 (8bab26f4f 2026-07-14). Each cell is the Criterion mean of one benchmark; the last column divides the ORM's time by the raw client's. The [Explain](explain.md) page says how to read these tables.

## `select`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `select/by_id` | 2.47 µs | 1.72 µs | 1.43× |
| `select/by_user` | 7.79 µs | 5.68 µs | 1.37× |
| `select/count` | 575.88 µs | 2.64 µs | 218.49× |
| `select/cursor` | 21.76 µs | 16.34 µs | 1.33× |
| `select/scan` (10000 rows) | 4.11 ms | 2.97 ms | 1.38× |

## `insert`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `insert/active_model` | 28.34 µs | 26.29 µs | 1.08× |
| `insert/exec` | 13.05 µs | 12.86 µs | 1.01× |
| `insert/many` (100 rows) | 378.26 µs | 302.41 µs | 1.25× |

## `update`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `update/active_model` | 15.74 µs | 14.43 µs | 1.09× |
| `update/many` | 2.80 µs | 2.37 µs | 1.18× |

## `delete`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `delete/insert_then_delete` | 21.49 µs | 20.18 µs | 1.06× |

## `relations`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `relations/find_also_related` | 12.36 µs | 8.90 µs | 1.39× |
| `relations/find_with_related` | 13.12 µs | 8.01 µs | 1.64× |
| `relations/linked` | 33.54 µs | 29.54 µs | 1.14× |
| `relations/load_many` | 36.84 µs | 34.70 µs | 1.06× |

## `render`

| Group | ORM | builder | Ratio |
|---|---|---|---|
| `render/create_table` | 229 ns | — | — |
| `render/insert_many` (100 rows) | — | 21.62 µs | — |
| `render/insert_one` | 138 ns | 131 ns | 1.06× |
| `render/select_by_id` | 308 ns | 333 ns | 0.92× |
| `render/select_join` | 580 ns | 486 ns | 1.19× |
