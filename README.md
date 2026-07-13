# Apache IoTDB Rust Client

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)

[English](./README.md) | [中文](./README_ZH.md)

Rust client SDK for [Apache IoTDB](https://iotdb.apache.org/), speaking Apache Thrift RPC (default port 6667). Supports both IoTDB data models, mirroring the architecture of the Node.js and C# SDKs:

- **Tree model** — `Session` / `SessionPool`: device/timeseries paths (`root.sg.d1.s1`)
- **Table model** — `TableSession` / `TableSessionPool`: relational SQL dialect

## Status

Working client: session management (with multi-node failover), tablet writes (`insertTablet`) for both models, TsBlock query decoding with paging iteration, and thread-safe session pools. Not yet published to crates.io.

## Requirements

- Rust 1.75+
- Apache IoTDB 2.x (examples and integration tests use `apache/iotdb:2.0.6-standalone`)

## Installation

Once published to crates.io:

```toml
[dependencies]
iotdb-client = "0.1"
```

Until then, use a git dependency:

```toml
[dependencies]
iotdb-client = { git = "https://github.com/apache/iotdb-client-rust" }
```

## Quick start

### Tree model

```rust
use iotdb_client::{Result, Session, SessionConfig, TSDataType, Tablet, Value};

fn main() -> Result<()> {
    let config = SessionConfig::default().with_node_urls(&["127.0.0.1:6667"])?;
    let mut session = Session::new(config);
    session.open()?;

    session.execute_non_query("CREATE DATABASE root.demo")?;
    session.execute_non_query(
        "CREATE TIMESERIES root.demo.d1.temperature WITH DATATYPE=DOUBLE, ENCODING=PLAIN",
    )?;

    // Batch write via a column-major tablet (nulls allowed).
    let mut tablet = Tablet::new(
        "root.demo.d1",
        vec!["temperature".into()],
        vec![TSDataType::Double],
    )?;
    tablet.add_row(1_720_000_000_000, vec![Some(Value::Double(21.5))])?;
    tablet.add_row(1_720_000_001_000, vec![None])?; // null cell
    session.insert_tablet(&tablet)?;
    // Multiple tablets in one RPC: insert_tablets(&[t1, t2], false)
    // (tree model only; insert_aligned_tablets for aligned devices).

    // Or write a single row via insertRecord (row-oriented; aligned variants
    // and multi-row insert_records / insert_records_of_one_device also exist).
    session.insert_record(
        "root.demo.d1",
        1_720_000_002_000,
        vec!["temperature".into()],
        &[Value::Double(22.0)],
        false, // is_aligned
    )?;

    // Query with row iteration; the dataset borrows the session until dropped.
    {
        let mut dataset = session.execute_query("SELECT temperature FROM root.demo.d1")?;
        while let Some(row) = dataset.next_row()? {
            println!("ts={:?} values={:?}", row.timestamp, row.values);
        }
    }

    session.execute_non_query("DELETE DATABASE root.demo")?;
    session.close()
}
```

### Table model

```rust
use iotdb_client::{ColumnCategory, Result, TSDataType, TableSession, Tablet, Value};

fn main() -> Result<()> {
    let mut session = TableSession::builder()
        .node_urls(&["127.0.0.1:6667"])?
        .username("root")
        .password("root")
        .build()?;

    session.execute_non_query("CREATE DATABASE IF NOT EXISTS demo")?;
    session.execute_non_query("USE demo")?;
    session.execute_non_query(
        "CREATE TABLE IF NOT EXISTS sensors (device_id STRING TAG, temperature DOUBLE FIELD)",
    )?;

    let mut tablet = Tablet::new_table(
        "sensors",
        vec!["device_id".into(), "temperature".into()],
        vec![TSDataType::String, TSDataType::Double],
        vec![ColumnCategory::Tag, ColumnCategory::Field],
    )?;
    tablet.add_row(
        1_720_000_000_000,
        vec![
            Some(Value::String("dev-1".into())),
            Some(Value::Double(21.5)),
        ],
    )?;
    session.insert(&tablet)?;

    {
        let mut dataset = session.execute_query("SELECT time, device_id, temperature FROM sensors")?;
        while let Some(row) = dataset.next_row()? {
            println!("{:?}", row.values);
        }
    }

    session.execute_non_query("DROP DATABASE demo")?;
    session.close()
}
```

### Session pool

```rust
use std::sync::Arc;
use iotdb_client::{Result, SessionPool, SessionPoolConfig};

fn main() -> Result<()> {
    let config = SessionPoolConfig {
        max_size: 4,
        ..SessionPoolConfig::default()
    }
    .with_node_urls(&["127.0.0.1:6667"])?;
    let pool = Arc::new(SessionPool::new(config)?);

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let pool = Arc::clone(&pool);
            std::thread::spawn(move || -> Result<()> {
                let mut session = pool.acquire()?; // RAII guard, released on drop
                session.execute_non_query("SHOW DATABASES")?;
                Ok(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread panicked")?;
    }

    pool.close();
    Ok(())
}
```

Full runnable versions live in [`examples/`](./examples):

```sh
cargo run --example tree_session
cargo run --example table_session
cargo run --example session_pool
```

## Thrift codegen

Generated stubs live in `src/protocol/` (`client.rs`, `common.rs`); never hand-edit them. The IDL sources in `thrift/` are synced from the IoTDB repo's `iotdb-protocol/` (`thrift-datanode/src/main/thrift/client.thrift`, `thrift-commons/src/main/thrift/common.thrift`).

Regenerate with:

```sh
./tools/generate-thrift.sh
```

The script picks the Thrift compiler in order of preference:

1. `$THRIFT_BIN` if set
2. the IoTDB repo's Maven build output (`$IOTDB_REPO`, default `../iotdb`): `iotdb-protocol/*/target/thrift/bin/thrift` — run `./mvnw generate-sources -pl iotdb-protocol/thrift-datanode -am` there first. This guarantees the exact Thrift version pinned by the IoTDB pom.
3. `thrift` on `PATH` (version must match the IoTDB pom's `thrift.version`)

When `$IOTDB_REPO` is present, the IDL files are re-synced from it before generation, and the Apache license headers are re-prepended to the generated files.

## Development

```sh
cargo build                              # build
cargo test                               # unit tests (live tests self-skip without a server)
cargo test test_name                     # single test
cargo fmt --check                        # format check
cargo clippy --all-targets -- -D warnings  # lint
./tools/check-license.sh                 # license header check
```

Integration tests need a running IoTDB; the live tests detect it on `127.0.0.1:6667` and skip gracefully when absent:

```sh
docker compose up -d   # standalone IoTDB (see docker-compose-1c1d.yml for a 1C1D cluster)
cargo test             # now includes the live-server tests
```

## Project layout

| Path | Contents |
| --- | --- |
| `src/client/` | `Session`, `TableSession`, `SessionPool`, `TableSessionPool`, `SessionDataSet` |
| `src/connection/` | Low-level Thrift transport (framed transport + binary protocol) |
| `src/data/` | `Tablet`, `Value`, `TSDataType` (official TSFile codes 0–11), TsBlock decoding, bitmaps |
| `src/protocol/` | Generated Thrift stubs (do not edit) |
| `thrift/` | Thrift IDL sources, synced from the IoTDB repo |
| `examples/` | Runnable examples for both models and the pools |
| `tools/` | Codegen and license-check scripts |

## License

[Apache License 2.0](./LICENSE)
