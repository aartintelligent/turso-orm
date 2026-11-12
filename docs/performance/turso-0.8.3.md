# Turso 0.8.3

Measured on 2026-11-12 with turso-orm 0.1.8 against turso 0.8.3, on AMD EPYC 7763 64-Core Processor, 4 logical cores (linux/x86_64), rustc 1.97.1 (8bab26f4f 2026-07-14). Each cell is the Criterion mean of one benchmark; the last column divides the ORM's time by the raw client's. The [Explain](explain.md) page says how to read these tables.

## `select`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `select/by_id` | 3.20 µs | 1.94 µs | 1.65× |
| `select/by_user` | 10.34 µs | 7.67 µs | 1.35× |
| `select/count` | 4.02 µs | 2.99 µs | 1.34× |
| `select/cursor` | 27.06 µs | 19.14 µs | 1.41× |
| `select/scan` (10000 rows) | 4.65 ms | 3.29 ms | 1.42× |

## `insert`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `insert/active_model` | 134.28 µs | 134.59 µs | 1.00× |
| `insert/exec` | 19.74 µs | 18.92 µs | 1.04× |
| `insert/many` (100 rows) | 586.96 µs | 492.51 µs | 1.19× |

## `update`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `update/active_model` | 102.81 µs | 119.54 µs | 0.86× |
| `update/many` | 3.53 µs | 2.84 µs | 1.24× |

## `delete`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `delete/insert_then_delete` | 33.01 µs | 27.71 µs | 1.19× |

## `relations`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `relations/find_also_related` | 18.46 µs | 11.65 µs | 1.58× |
| `relations/find_with_related` | 18.21 µs | 11.53 µs | 1.58× |
| `relations/linked` | 37.09 µs | 34.73 µs | 1.07× |
| `relations/load_many` | 134.78 µs | 117.24 µs | 1.15× |

## `render`

| Group | ORM | builder | Ratio |
|---|---|---|---|
| `render/create_table` | 372 ns | — | — |
| `render/insert_many` (100 rows) | — | 27.83 µs | — |
| `render/insert_one` | 250 ns | 218 ns | 1.14× |
| `render/select_by_id` | 516 ns | 524 ns | 0.99× |
| `render/select_join` | 886 ns | 763 ns | 1.16× |
