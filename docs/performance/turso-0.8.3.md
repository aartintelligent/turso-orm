# Turso 0.8.3

Measured on 2026-11-12 with turso-orm 0.1.7 against turso 0.8.3, on AMD EPYC 7763 64-Core Processor, 4 logical cores (linux/x86_64), rustc 1.97.1 (8bab26f4f 2026-07-14). Each cell is the Criterion mean of one benchmark; the last column divides the ORM's time by the raw client's. The [Explain](explain.md) page says how to read these tables.

## `select`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `select/by_id` | 3.25 µs | 1.96 µs | 1.66× |
| `select/by_user` | 10.44 µs | 7.76 µs | 1.34× |
| `select/count` | 4.11 µs | 3.08 µs | 1.33× |
| `select/cursor` | 26.79 µs | 19.83 µs | 1.35× |
| `select/scan` (10000 rows) | 4.36 ms | 3.35 ms | 1.30× |

## `insert`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `insert/active_model` | 127.21 µs | 135.29 µs | 0.94× |
| `insert/exec` | 19.24 µs | 17.14 µs | 1.12× |
| `insert/many` (100 rows) | 557.38 µs | 471.97 µs | 1.18× |

## `update`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `update/active_model` | 103.29 µs | 110.28 µs | 0.94× |
| `update/many` | 3.60 µs | 2.84 µs | 1.27× |

## `delete`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `delete/insert_then_delete` | 32.12 µs | 26.00 µs | 1.24× |

## `relations`

| Group | ORM | raw | Ratio |
|---|---|---|---|
| `relations/find_also_related` | 17.15 µs | 11.57 µs | 1.48× |
| `relations/find_with_related` | 17.78 µs | 11.02 µs | 1.61× |
| `relations/linked` | 34.72 µs | 29.25 µs | 1.19× |
| `relations/load_many` | 133.12 µs | 112.34 µs | 1.18× |

## `render`

| Group | ORM | builder | Ratio |
|---|---|---|---|
| `render/create_table` | 359 ns | — | — |
| `render/insert_many` (100 rows) | — | 27.21 µs | — |
| `render/insert_one` | 231 ns | 219 ns | 1.06× |
| `render/select_by_id` | 549 ns | 537 ns | 1.02× |
| `render/select_join` | 883 ns | 761 ns | 1.16× |
