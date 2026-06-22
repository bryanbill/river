<div align="center">
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/river.png">
  <source media="(prefers-color-scheme: light)" srcset="assets/river.png">
  <img alt="River" src="assets/river.png" width="150">
</picture>

**A unified interface to query across multiple database instances.**

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
</div>


---

## What is River?

River gives you a single point of access to all your databases. Query across engines, join data that spans different systems, and browse everything from one terminal.

Data today lives everywhere — different servers, different architectures, different query languages. Federated query engines like Trino, Presto, and Databricks Unity Catalog solve this at the warehouse scale, but they demand infrastructure and setup. River brings the same idea to your terminal: lightweight, zero-config, cross-database queries without the overhead.

## Features

- **Multi-database support** — Postgres, MySQL/MariaDB, SQLite, MongoDB, SQL Server
- **RiverQL** — a familiar, SQL-inspired query language that abstracts away vendor differences
- **Cross-database joins** — combine data from different engines in a single query
- **CTEs, window functions, set operations** (UNION/INTERSECT/EXCEPT)
- **Terminal UI** — browse results, inspect schemas, and explore connections interactively
- **Zero config for ad-hoc queries** — just point it at a connection and start querying

## Install

### Quick install (recommended)

**Linux / macOS:**

```bash
curl -fsSL https://raw.githubusercontent.com/bryanbill/river/main/install.sh | bash
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/bryanbill/river/main/install.ps1 | iex
```

### Manual install

Download the latest binary from the [releases page](https://github.com/bryanbill/river/releases) for your platform.

### Build from source

```bash
git clone https://github.com/bryanbill/river.git
cd river
cargo build --release
./target/release/river --help
```

## Quick Start

1. Create a `river.yaml` file with your connections:

```yaml
- name: pg
  kind: postgres
  uri: "postgres://user:pass@localhost:5432/mydb"

- name: mongo
  kind: mongodb
  uri: "mongodb://localhost:27017/mydb"
```

2. Launch River:

```bash
river
```

3. Start querying with RiverQL:

```sql
find [name, email] from users where status = "active"
```

See the [full reference](docs.md) for a full overview of RiverQL syntax and features.

## Supported Databases

| Database | Kind      |
|----------|-----------|
| Postgres | `postgres`|
| MySQL    | `mysql`   |
| SQLite   | `sqlite`  |
| MongoDB  | `mongodb` |
| SQL Server | `mssql` |

## License

MIT
