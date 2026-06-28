const DOCS_FULL: &str = include_str!("../../docs.md");

pub fn full_reference() -> &'static str {
    DOCS_FULL
}

pub fn quickref() -> &'static str {
    let idx = DOCS_FULL.find("## Quick Reference").unwrap_or(DOCS_FULL.len());
    let start = DOCS_FULL[..idx]
        .rfind("---")
        .map(|p| p + 3)
        .unwrap_or(idx);
    &DOCS_FULL[start..]
}

pub fn keywords() -> &'static str {
    let start = DOCS_FULL.find("### Keywords").unwrap_or(DOCS_FULL.len());
    let end = DOCS_FULL[start..]
        .find("\n### Operators")
        .map(|p| start + p)
        .unwrap_or(DOCS_FULL.len());
    &DOCS_FULL[start..end]
}

pub fn topic(topic: &str) -> Option<&'static str> {
    match topic {
        "select" => {
            let s = section("## Query Basics", "## Expressions and Operators");
            Some(s)
        }
        "joins" => {
            let s = section("## Joins", "## Aggregation");
            Some(s)
        }
        "aggregation" => {
            let s = section("## Aggregation", "## Window Functions");
            Some(s)
        }
        "window" => {
            let s = section("## Window Functions", "## Advanced Queries");
            Some(s)
        }
        "modification" => {
            let s = section("## Data Modification", "## Meta Commands");
            Some(s)
        }
        "ddl" => {
            let idx = DOCS_FULL.find("### CREATE TABLE").unwrap_or(DOCS_FULL.len());
            let end = DOCS_FULL[idx..]
                .find("\n## Meta Commands")
                .map(|p| idx + p)
                .unwrap_or(DOCS_FULL.len());
            Some(&DOCS_FULL[idx..end])
        }
        "meta" => {
            let s = section("## Meta Commands", "## Quick Reference");
            Some(s)
        }
        "cross_db" => {
            let s = section("## Cross-Database Queries", "## Data Modification");
            Some(s)
        }
        "operators" => {
            let idx = DOCS_FULL.find("### Operators").unwrap_or(DOCS_FULL.len());
            let end = DOCS_FULL[idx..]
                .find("\n### Operator Precedence")
                .map(|p| idx + p)
                .unwrap_or(DOCS_FULL.len());
            Some(&DOCS_FULL[idx..end])
        }
        "functions" => {
            let idx = DOCS_FULL
                .find("### Built-in Functions")
                .unwrap_or(DOCS_FULL.len());
            let end = DOCS_FULL[idx..]
                .find("\n## Advanced Queries")
                .unwrap_or_else(|| DOCS_FULL[idx..].len());
            let mid = DOCS_FULL[idx..]
                .find("## Data Modification")
                .unwrap_or(usize::MAX);
            let end = end.min(mid + idx);
            if end > idx {
                Some(&DOCS_FULL[idx..end])
            } else {
                None
            }
        }
        "keywords" => {
            let s = section("### Keywords", "### Operators");
            Some(s)
        }
        _ => None,
    }
}

fn section(start_heading: &str, end_heading: &str) -> &'static str {
    let start = DOCS_FULL.find(start_heading).unwrap_or(DOCS_FULL.len());
    let end = DOCS_FULL[start..]
        .find(end_heading)
        .map(|p| start + p)
        .unwrap_or(DOCS_FULL.len());
    &DOCS_FULL[start..end]
}

const OVERVIEW: &str = "\
# RiverQL Overview

RiverQL is a universal query language for PostgreSQL, MySQL, SQLite, MongoDB, and SQL Server.

## Query Structure
```
find [columns] from table[@connection]
where condition
group by columns
having condition
order by column [asc|desc]
limit N offset M
```

## Key Syntax

**SELECT:** `find [col1, col2] from table`
**WHERE:** `find users where age > 21 and status = \"active\"`
**JOIN:** `find [u.name, o.total] from users as u join orders as o on u.id = o.user_id`
**GROUP BY:** `find [dept, count(*)] from employees group by dept`
**ORDER BY:** `find users order by name desc`
**LIMIT/OFFSET:** `find users limit 10 offset 20`

**INSERT:** `create users { name: \"Alice\", email: \"alice@example.com\" }`
**UPDATE:** `update users set status = \"active\" where id = 1`
**DELETE:** `remove users where status = \"banned\"`

**CREATE TABLE:** `create table products (name string, price float)`
**ALTER TABLE:** `alter table users add column bio string`
**DROP TABLE:** `drop table if exists temp_logs`
**CREATE DATABASE:** `create database analytics@pg`
**DROP DATABASE:** `drop database if exists old_data@mysql`

**Cross-DB:** `find * from users@prod-pg join orders@analytics-mysql on ...`

**Meta:** `describe table`, `show tables`, `explain find ...`

## Connection Reference: `@connection_name`

## Call `riverql_help` with a topic for detailed syntax:
- select, joins, aggregation, window, modification, ddl, meta, cross_db, operators, functions, keywords
";

pub fn overview() -> &'static str {
    OVERVIEW
}
