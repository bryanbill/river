# Integration Test Framework Design

## Goal

Create a full end-to-end integration test framework that verifies River's cross-database query capabilities against real database instances. Tests exercise the complete pipeline: RiverQL parsing → query planning → translation → execution against real databases → result verification.

## Prerequisites

- Docker Compose services running (`infra/docker-compose.yml`: Postgres, MySQL, MongoDB)
- Seed data loaded (`python3 infra/seed.py`: 10,000 deterministic rows per table across all DBs)
- SQLite file present at `river.db`

## Architecture

```
tests/
├── common/
│   └── mod.rs          # Shared setup: connect all DBs, helpers, assertions
├── adapter_tests.rs    # Each adapter connects & executes correctly
├── cross_db_tests.rs   # Cross-database joins (core feature)
├── query_pipeline.rs   # Full parse → plan → translate → execute pipeline
├── mutations.rs        # INSERT/UPDATE/DELETE across databases
└── edge_cases.rs       # NULLs, type coercion, large results, errors
```

## Shared Helpers (`tests/common/mod.rs`)

### Setup

- `setup()` → loads `infra/river.yaml`, creates adapters for all 4 databases (Postgres, MySQL, MongoDB, SQLite)
- Panics early with clear error messages if any DB is unreachable, telling the user to run `docker-compose up -d`
- Returns a `TestContext` struct holding the adapter map and connection configs

### Execution

- `execute_river(query: &str, ctx: &TestContext) -> Result<QueryResult, RiverError>`
  - Parses the RiverQL string
  - Plans the query (detecting single vs cross-DB)
  - Executes against the appropriate adapters
  - Returns the result or error

### Assertion Helpers

- `assert_columns(result, &["name", "total"])` — verify column names
- `assert_row_count(result, expected)` — verify exact row count
- `assert_row_count_gte(result, min)` — verify minimum rows
- `assert_contains_value(result, col_name, expected_value)` — check a value exists
- `assert_no_nulls(result, col_name)` — verify no NULLs in a column
- `assert_ordered(result, col_name, direction)` — verify sort order

## Test Categories

### 1. Adapter Tests (`adapter_tests.rs`)

Verify each adapter independently connects and returns correct data.

| Test | Query | Assertion |
|------|-------|-----------|
| `pg_select_by_id` | `find [name, email] from users@pg where id = 1` | Returns deterministic user #1 |
| `pg_select_with_filter` | `find [name] from users@pg where department = "Engineering"` | Non-empty result, all rows match |
| `mysql_select_by_id` | `find [name, email] from users@mysql where id = 1` | Same user #1 data as PG |
| `mysql_select_with_limit` | `find [name] from users@mysql limit 5` | Exactly 5 rows |
| `mongo_select_by_id` | `find [name, email] from users@mongo where _id = 1` | Same user #1 |
| `mongo_select_with_filter` | `find [name] from users@mongo where status = "active"` | Non-empty, all active |
| `sqlite_select_by_id` | `find [name, email] from users@sqlite where id = 1` | Same user #1 |
| `sqlite_select_all` | `find [name] from users@sqlite` | 10,000 rows |

### 2. Cross-Database Join Tests (`cross_db_tests.rs`)

The core feature — joining data across different database systems.

| Test | Description |
|------|-------------|
| `pg_mysql_inner_join` | Join `users@pg` with `orders@mysql` on `id = user_id` |
| `pg_mongo_inner_join` | Join `users@pg` with `orders@mongo` on `id = user_id` |
| `mysql_sqlite_inner_join` | Join `users@mysql` with `orders@sqlite` on `id = user_id` |
| `pg_mysql_left_join` | Left join ensuring unmatched rows have NULLs |
| `cross_db_with_filter` | Cross-DB join with WHERE clause on both sides |
| `cross_db_with_projection` | Cross-DB join selecting specific columns |
| `cross_db_with_limit` | Cross-DB join with LIMIT applied after join |
| `cross_db_with_order` | Cross-DB join with ORDER BY on merged result |
| `three_way_cte` | CTE pulling from PG + MySQL, then joining in-memory |
| `cross_db_same_table_different_dbs` | Join `users@pg` with `users@mysql` (same schema, different sources) |

### 3. Full Pipeline Tests (`query_pipeline.rs`)

End-to-end from query string through entire engine.

| Test | Description |
|------|-------------|
| `pipeline_simple_select` | Basic select parses and executes |
| `pipeline_projection` | `find [name, salary] from users@pg` returns only 2 columns |
| `pipeline_filter_pushdown` | Filters are pushed to DB (verify by checking row count) |
| `pipeline_aggregation` | `GROUP BY department` with `count(*)` |
| `pipeline_distinct` | `find distinct [status] from users@pg` |
| `pipeline_order_by` | Results come back sorted |
| `pipeline_limit_offset` | Pagination works correctly |
| `pipeline_between` | `where salary between 50000 and 100000` |
| `pipeline_like` | `where name like "A%"` |
| `pipeline_cross_db_pipeline` | Full cross-DB query through pipeline |

### 4. Mutation Tests (`mutations.rs`)

DML operations that modify data.

| Test | Description |
|------|-------------|
| `insert_pg` | INSERT a row into Postgres, verify with SELECT |
| `insert_mysql` | INSERT a row into MySQL, verify with SELECT |
| `update_pg` | UPDATE a row, verify change |
| `delete_pg` | DELETE a row, verify removal |
| `insert_verify_cross_db` | INSERT into one DB, verify it appears in cross-DB join |

Each mutation test should clean up after itself (DELETE the inserted row, restore the updated row) to keep the seed dataset stable for other tests.

### 5. Edge Cases (`edge_cases.rs`)

Error handling, boundary conditions, and unusual inputs.

| Test | Description |
|------|-------------|
| `null_join_key` | Join where some rows have NULL join keys — should not match |
| `type_coercion_int_float` | Join on integer column vs float column |
| `large_result_join` | Join that produces 1000+ rows — verify no crash/truncation |
| `empty_result` | Query with impossible filter returns 0 rows, not error |
| `invalid_connection` | `find from users@nonexistent` → meaningful error |
| `syntax_error` | `find from` (incomplete) → parser error |
| `invalid_table` | `find from nonexistent_table@pg` → DB error surfaced |
| `cross_db_mismatched_types` | Join columns with different types across DBs |
| `concurrent_queries` | Execute 5 queries concurrently → all succeed |
| `special_characters_in_data` | Query/filter with quotes, unicode |

## Visibility Changes

Create `src/lib.rs` to expose internal modules for integration test consumption:

```rust
pub mod adapters;
pub mod connection;
pub mod engine;
pub mod error;
pub mod lang;
```

This is standard practice — `main.rs` handles the binary entry point, `lib.rs` exposes the library for tests and potential future reuse.

## Running Tests

```bash
# Prerequisites
docker compose -f infra/docker-compose.yml up -d --wait
python3 infra/seed.py

# Run all integration tests
cargo test --test '*'

# Run specific category
cargo test --test adapter_tests
cargo test --test cross_db_tests
cargo test --test query_pipeline
cargo test --test mutations
cargo test --test edge_cases

# Run with output for debugging
cargo test --test cross_db_tests -- --nocapture
```

## Seed Data Reference

Tests rely on deterministic data from `infra/seed.py`. Key properties:

- User #1: `name = "Kate Martin"`, `email = "kate.martin1@example.com"`, `department = "Support"`, `salary = 35017`, `status = "pending"`, `is_verified = false`
- 10,000 rows per table across all databases
- Same data in all SQL databases (Postgres, MySQL, SQLite)
- Same data in MongoDB (with `_id` instead of auto-increment `id`)
- Deterministic formulas mean row N always has the same values

## Success Criteria

- [ ] All 4 adapters connect and return correct data
- [ ] Cross-DB joins produce correct merged results (verified row counts and values)
- [ ] Full pipeline from RiverQL string to result works end-to-end
- [ ] Mutations (INSERT/UPDATE/DELETE) work and are verifiable
- [ ] Edge cases produce meaningful errors (not panics)
- [ ] Tests are independent (can run in any order)
- [ ] Test output is clear about what failed and why
- [ ] `cargo test --test '*'` passes cleanly with docker-compose running
