# Turso 0.8.3

Measured on 2026-10-15 with turso-orm 0.1.7 against turso 0.8.3, on INTEL(R) XEON(R) PLATINUM 8573C, 4 logical cores (linux/x86_64), rustc 1.97.1 (8bab26f4f 2026-07-14). Each cell is the Criterion mean of one benchmark; the last column divides the ORM's time by the raw client's. The [Explain](explain.md) page says how to read these tables.

## `select`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `select/by_id` | 3.92 µs | 2.58 µs | 1.52× |
| `select/by_user` | 12.30 µs | 8.88 µs | 1.39× |
| `select/count` | 5.09 µs | 4.02 µs | 1.27× |
| `select/cursor` | 34.33 µs | 24.91 µs | 1.38× |
| `select/scan` (10000 rows) | 5.76 ms | 4.65 ms | 1.24× |

## `insert`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `insert/active_model` | 66.93 µs | 83.57 µs | 0.80× |
| `insert/exec` | 19.88 µs | 19.02 µs | 1.05× |
| `insert/many` (100 rows) | 514.44 µs | 442.48 µs | 1.16× |

## `update`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `update/active_model` | 37.08 µs | 75.61 µs | 0.49× |
| `update/many` | 4.74 µs | 4.02 µs | 1.18× |

## `delete`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `delete/insert_then_delete` | 35.00 µs | 30.45 µs | 1.15× |

## `relations`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `relations/find_also_related` | 18.48 µs | 13.06 µs | 1.41× |
| `relations/find_with_related` | 19.84 µs | 12.90 µs | 1.54× |
| `relations/linked` | 46.01 µs | 39.06 µs | 1.18× |
| `relations/load_many` | 86.63 µs | 63.01 µs | 1.37× |

## `render`

| Group | ORM | builder | Ratio |
|---|---|---|---|
| `render/create_table` | 359 ns | — | — |
| `render/insert_many` (100 rows) | — | 32.96 µs | — |
| `render/insert_one` | 235 ns | 204 ns | 1.15× |
| `render/select_by_id` | 472 ns | 467 ns | 1.01× |
| `render/select_join` | 879 ns | 747 ns | 1.18× |
