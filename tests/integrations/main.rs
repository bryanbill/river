//! Comprehensive E2E integration tests for all documented RiverQL features.
//!
//! These tests run against real seeded databases (Postgres, MySQL, SQLite, MongoDB)
//! and validate correctness using deterministic seed formulas and cross-DB consistency.
//!
//! Prerequisites: `docker-compose up -d` in infra/ and `python3 infra/seed.py` run.

mod helpers;
mod t01_basic_queries;
mod t02_filtering;
mod t03_joins;
mod t04_aggregation;
mod t05_window_functions;
mod t06_advanced_queries;
mod t07_cross_database;
mod t08_data_modification;
mod t09_meta_commands;
mod t10_schema;
mod t11_create_table;

#[path = "../common/mod.rs"]
mod common;
