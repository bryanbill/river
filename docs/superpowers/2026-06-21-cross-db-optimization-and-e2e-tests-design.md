# Cross-DB Join Optimization & E2E Test Suite

## Problem

1. Cross-database joins currently fetch both sides fully into memory and join locally. For large datasets or cartesian products, this hangs or OOMs.
2. No comprehensive E2E tests validate query correctness against real seeded databases.

## Solution Overview

### Part 1: Cross-DB Join Optimization (Chunked Pushdown)

Use a "planner-driven pushdown" strategy: the smaller side is fetched fully, its join keys are extracted, and the larger side is queried with batched `WHERE key IN (...)` filters pushed down to the remote database.

Cross-DB cross joins without a LIMIT clause are rejected at plan time.

### Part 2: E2E Integration Tests

One test file per documentation topic (docs 01–09), validating query correctness using exact-match assertions (seed-formula-based) and cross-database consistency checks.

---

## Part 1: Detailed Design

### 1.1 New AST Node

Add to `Expression` enum in `src/lang/ast.rs`:

```rust
Expression::In {
    expr: Box<Expression>,
    values: Vec<Expression>,
}
```

### 1.2 New PlanNode Variant

Add to `PlanNode` enum in `src/engine/planner.rs`:

```rust
PlanNode::SemiJoinFetch {
    build: Box<PlanNode>,
    probe_source: Source,
    probe_database: (String, DatabaseKind),
    build_key: Expression,
    probe_key: Expression,
    join_kind: JoinKind,
    condition: Expression,
}
```

### 1.3 Planner Decision Tree

When the planner creates a cross-DB join:

```
Cross-DB join detected?
+-- No -> emit normal PlanNode::Join (pushdown to single DB)
+-- Yes
    +-- Has equi-condition?
    |   +-- Yes -> emit SemiJoinFetch
    +-- Has LIMIT?
    |   +-- Yes -> emit normal PlanNode::Join (bounded, acceptable)
    +-- Neither -> return error
```

**Build side heuristic:** If one side has a LIMIT or filter, prefer that as the build side. Otherwise, default to left.

### 1.4 Executor: SemiJoinFetch Execution

```
1. Execute build side -> build_result
2. Extract distinct join keys from build_result[build_key_column]
3. Chunk keys into batches of CROSS_DB_BATCH_SIZE (1000)
4. For each batch:
   - Build query: probe_source scan + WHERE probe_key IN (batch_keys)
   - Translate to native dialect
   - Execute against probe_database
5. Concatenate all batch results -> probe_result
6. hash_join(build_result, probe_result, condition, join_kind)
```

- Batches execute sequentially to avoid overwhelming the remote DB.
- Empty build result -> skip probe, return empty result.
- Fallback: if equi-key extraction fails at runtime, fall back to full-fetch hash join with a warning log.

### 1.5 Translator: IN Clause

| Dialect  | Output                                       |
|----------|----------------------------------------------|
| Postgres | `"user_id" IN (1, 2, 3, ...)`               |
| MySQL    | `` `user_id` IN (1, 2, 3, ...) ``           |
| SQLite   | `"user_id" IN (1, 2, 3, ...)`               |
| MSSQL    | `[user_id] IN (1, 2, 3, ...)`               |
| MongoDB  | `{ "user_id": { "$in": [1, 2, 3, ...] } }`  |

Value serialization:
- `Value::Int(i)` -> `i` (no quotes)
- `Value::String(s)` -> `'escaped_s'`
- `Value::Float(f)` -> `f`
- `Value::Null` -> skipped

### 1.6 Cross-Join Guard

Cross-DB joins that are cross joins (no condition or non-equi condition) without a LIMIT are rejected at plan time:

```
"Cross-database cross joins require a LIMIT clause to prevent unbounded result sets.
Add 'limit N' to your query."
```

Same-DB cross joins are unaffected (pushed down natively).

### 1.7 Constants

```rust
const CROSS_DB_BATCH_SIZE: usize = 1000;
```

---

## Part 2: E2E Integration Tests

### 2.1 Directory Structure

```
tests/integrations/
+-- mod.rs                    # shared test helpers
+-- 01_basic_queries.rs       # doc 01: find, select columns, limit/offset
+-- 02_filtering.rs           # doc 02: comparisons, logical ops, NULL, LIKE, BETWEEN, IN, CASE
+-- 03_joins.rs               # doc 03: inner, left, right, full, cross, self-join, multi-join
+-- 04_aggregation.rs         # doc 04: count/sum/avg/min/max, GROUP BY, HAVING
+-- 05_window_functions.rs    # doc 05: row_number, rank, dense_rank, lag, lead
+-- 06_advanced_queries.rs    # doc 06: subqueries, CTEs, UNION, DISTINCT, CASE, CAST
+-- 07_cross_database.rs      # doc 07: cross-DB joins, CTEs, connection refs
+-- 08_data_modification.rs   # doc 08: INSERT, UPDATE, DELETE
+-- 09_meta_commands.rs       # doc 09: DESCRIBE, SHOW TABLES
```

### 2.2 Test Infrastructure (mod.rs)

- `TestContext` — connects to all 4 databases from `infra/river.yaml`
- `execute_river(ctx, query) -> QueryResult` — parses, plans, executes
- Assertion helpers:
  - `assert_exact_rows(result, expected_rows)` — full row comparison
  - `assert_row_count(result, n)` — count check
  - `assert_contains_row(result, row)` — spot-check a specific row
  - `assert_column_values(result, col, predicate)` — all values satisfy condition
  - `assert_cross_db_consistency(ctx, query_template, connections)` — same query on each DB, assert identical results

### 2.3 Validation Strategy

**Single-DB queries:** Exact-match assertions based on seed data formulas.

The seed data is deterministic — `user_row(i)`, `product_row(i)`, `order_row(i)`, `order_item_row(i)` produce known values for any index. Tests compute expected values inline.

Example:
```rust
// user_row(1): name = "Jack Clark", dept = "Operations", salary = 35017
// So: find users where id = 1 -> expect exactly this row
```

**Cross-DB queries:** Cross-database consistency checks. Run the same logical query against pg, mysql, sqlite, mongo independently, verify River produces identical results.

### 2.4 Test Isolation (Data Modification Tests)

Mutation tests (INSERT/UPDATE/DELETE) use one of:
- Transaction wrapping with rollback after each test
- Re-seed affected tables after each test

Tests are marked `#[serial]` to avoid conflicts between concurrent mutation tests.

### 2.5 Meta Command Tests

`DESCRIBE table` and `SHOW TABLES` return structured results. Tests verify:
- Table names are present in SHOW TABLES output
- Column names and types match expected schema from seed DDL

---

## Files Modified

- `src/lang/ast.rs` — Add `Expression::In { expr, values }`
- `src/engine/planner.rs` — Add `SemiJoinFetch` node, cross-join guard, decision tree
- `src/engine/executor.rs` — Handle `SemiJoinFetch` execution
- `src/engine/translator.rs` — Translate `IN (...)` for SQL dialects
- `src/adapters/mongodb.rs` (or mongo translator) — Translate `$in`

## New Files

- `tests/integrations/mod.rs`
- `tests/integrations/01_basic_queries.rs`
- `tests/integrations/02_filtering.rs`
- `tests/integrations/03_joins.rs`
- `tests/integrations/04_aggregation.rs`
- `tests/integrations/05_window_functions.rs`
- `tests/integrations/06_advanced_queries.rs`
- `tests/integrations/07_cross_database.rs`
- `tests/integrations/08_data_modification.rs`
- `tests/integrations/09_meta_commands.rs`

## No Breaking Changes

- Same-DB queries completely unaffected
- Existing cross-DB equi-joins with LIMIT still work (now faster)
- Cross-DB cross joins that previously hung now get a clear error (unless LIMIT added)
