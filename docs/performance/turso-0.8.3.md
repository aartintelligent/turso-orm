# Turso 0.8.3

Measured on 2026-10-15 with turso-orm 0.1.5 against turso 0.8.3, on 12th Gen Intel(R) Core(TM) i7-12700K, 20 logical cores (linux/x86_64), rustc 1.97.1 (8bab26f4f 2026-07-14). Each cell is the Criterion mean of one benchmark; the last column divides the ORM's time by the raw client's. The [Explain](explain.md) page says how to read these tables.

## `select`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `select/by_id` | 2.32 µs | 1.56 µs | 1.49× |
| `select/by_user` | 7.23 µs | 5.67 µs | 1.28× |
| `select/count` | 3.02 µs | 2.41 µs | 1.25× |
| `select/cursor` | 19.86 µs | 15.58 µs | 1.27× |
| `select/scan` (10000 rows) | 3.76 ms | 2.80 ms | 1.34× |

## `insert`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `insert/active_model` | 27.86 µs | 27.35 µs | 1.02× |
| `insert/exec` | 14.10 µs | 12.43 µs | 1.13× |
| `insert/many` (100 rows) | 347.41 µs | 310.28 µs | 1.12× |

## `update`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `update/active_model` | 15.49 µs | 13.99 µs | 1.11× |
| `update/many` | 2.64 µs | 2.19 µs | 1.20× |

## `delete`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `delete/insert_then_delete` | 20.69 µs | 19.69 µs | 1.05× |

## `relations`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `relations/find_also_related` | 13.17 µs | 8.99 µs | 1.47× |
| `relations/find_with_related` | 13.56 µs | 7.93 µs | 1.71× |
| `relations/linked` | 26.10 µs | 24.71 µs | 1.06× |
| `relations/load_many` | 34.21 µs | 28.76 µs | 1.19× |

## `render`

| Group | ORM | builder | Ratio |
|---|---|---|---|
| `render/create_table` | 228 ns | — | — |
| `render/insert_many` (100 rows) | — | 20.68 µs | — |
| `render/insert_one` | 136 ns | 117 ns | 1.16× |
| `render/select_by_id` | 297 ns | 287 ns | 1.04× |
| `render/select_join` | 549 ns | 507 ns | 1.08× |
