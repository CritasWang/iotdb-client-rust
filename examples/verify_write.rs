// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Manual data-correctness check: write 10 rows covering every TSDataType
//! (one FIELD per type, plus 2 TAGs + 2 ATTRIBUTEs on the table model),
//! every row containing at least one null, every value distinct — then read
//! everything back and print it for eyeball comparison.
//!
//! Usage: cargo run --example verify_write -- <host> <port> <user> <password>

use iotdb_client::{
    ColumnCategory, Result, Session, SessionConfig, TSDataType, TableSession, Tablet, Value,
};

const TREE_DB: &str = "root.verify_rust";
const TABLE_DB: &str = "verify_rust";

/// FIELD columns: one per TSDataType (10 types).
const FIELD_DEFS: [(&str, TSDataType); 10] = [
    ("f_boolean", TSDataType::Boolean),
    ("f_int32", TSDataType::Int32),
    ("f_int64", TSDataType::Int64),
    ("f_float", TSDataType::Float),
    ("f_double", TSDataType::Double),
    ("f_text", TSDataType::Text),
    ("f_timestamp", TSDataType::Timestamp),
    ("f_date", TSDataType::Date),
    ("f_blob", TSDataType::Blob),
    ("f_string", TSDataType::String),
];

/// Value for field column `col` at row `r` (all distinct across rows), or
/// None on the anti-diagonal so every row has exactly one null field:
/// row r nulls field (10 - 1 - r).
fn field_value(col: usize, r: usize) -> Option<Value> {
    if col == 9 - r {
        return None;
    }
    let i = r as i32;
    Some(match col {
        0 => Value::Boolean(r % 2 == 0),
        1 => Value::Int32(100 + i),
        2 => Value::Int64(1_000_000_000_000 + i as i64),
        3 => Value::Float(1.5 + i as f32 * 0.25),
        4 => Value::Double(2.25 + i as f64 * 0.5),
        5 => Value::Text(format!("text-{r:02}")),
        6 => Value::Timestamp(1_720_000_000_000 + i as i64 * 1000),
        7 => Value::Date(20260701 + i), // 2026-07-01 .. 2026-07-10
        8 => Value::Blob(vec![0xB0 + r as u8, 0x00, 0xFF - r as u8]),
        9 => Value::String(format!("str-{r:02}")),
        _ => unreachable!(),
    })
}

fn print_resultset(mut ds: iotdb_client::SessionDataSet<'_>) -> Result<usize> {
    let headers = ds.columns().join(" | ");
    println!("  {headers}");
    let mut n = 0;
    while let Some(row) = ds.next_row()? {
        let ts = row
            .timestamp
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".into());
        let cells: Vec<String> = row
            .values
            .iter()
            .map(|v| match v {
                Value::Null => "null".to_string(),
                Value::Blob(b) => format!(
                    "0x{}",
                    b.iter().map(|x| format!("{x:02X}")).collect::<String>()
                ),
                other => format!("{other:?}"),
            })
            .collect();
        println!("  ts={ts} | {}", cells.join(" | "));
        n += 1;
    }
    Ok(n)
}

fn tree_model(host: &str, port: u16, user: &str, password: &str) -> Result<()> {
    println!("\n=== TREE MODEL ({TREE_DB}) ===");
    let mut session = Session::new(SessionConfig {
        endpoints: vec![iotdb_client::Endpoint::new(host, port)],
        username: user.into(),
        password: password.into(),
        ..Default::default()
    });
    session.open()?;
    let _ = session.execute_non_query(&format!("DELETE DATABASE {TREE_DB}"));
    session.execute_non_query(&format!("CREATE DATABASE {TREE_DB}"))?;
    for (name, ty) in FIELD_DEFS {
        session.execute_non_query(&format!(
            "CREATE TIMESERIES {TREE_DB}.d1.{name} WITH datatype={}",
            match ty {
                TSDataType::Boolean => "BOOLEAN",
                TSDataType::Int32 => "INT32",
                TSDataType::Int64 => "INT64",
                TSDataType::Float => "FLOAT",
                TSDataType::Double => "DOUBLE",
                TSDataType::Text => "TEXT",
                TSDataType::Timestamp => "TIMESTAMP",
                TSDataType::Date => "DATE",
                TSDataType::Blob => "BLOB",
                TSDataType::String => "STRING",
                _ => unreachable!(),
            }
        ))?;
    }

    let mut tablet = Tablet::new(
        format!("{TREE_DB}.d1"),
        FIELD_DEFS.iter().map(|(n, _)| n.to_string()).collect(),
        FIELD_DEFS.iter().map(|&(_, t)| t).collect(),
    )?;
    for r in 0..10 {
        tablet.add_row(
            1000 + r as i64,
            (0..10).map(|c| field_value(c, r)).collect(),
        )?;
    }
    session.insert_tablet(&tablet)?;

    println!("-- read back: SELECT * FROM {TREE_DB}.d1 --");
    let n = print_resultset(session.execute_query(&format!("SELECT * FROM {TREE_DB}.d1"))?)?;
    println!("-- {n} rows --");
    session.close()?;
    Ok(())
}

fn table_model(host: &str, port: u16, user: &str, password: &str) -> Result<()> {
    println!("\n=== TABLE MODEL (db {TABLE_DB}, table sensor_data) ===");
    let mut session = TableSession::builder()
        .node_urls(&[format!("{host}:{port}")])?
        .username(user)
        .password(password)
        .build()?;
    let _ = session.execute_non_query(&format!("DROP DATABASE {TABLE_DB}"));
    session.execute_non_query(&format!("CREATE DATABASE {TABLE_DB}"))?;
    session.execute_non_query(&format!("USE {TABLE_DB}"))?;

    let mut ddl = String::from(
        "CREATE TABLE sensor_data (region STRING TAG, device_id STRING TAG, \
         vendor STRING ATTRIBUTE, model STRING ATTRIBUTE",
    );
    for (name, ty) in FIELD_DEFS {
        ddl.push_str(&format!(
            ", {name} {} FIELD",
            match ty {
                TSDataType::Boolean => "BOOLEAN",
                TSDataType::Int32 => "INT32",
                TSDataType::Int64 => "INT64",
                TSDataType::Float => "FLOAT",
                TSDataType::Double => "DOUBLE",
                TSDataType::Text => "TEXT",
                TSDataType::Timestamp => "TIMESTAMP",
                TSDataType::Date => "DATE",
                TSDataType::Blob => "BLOB",
                TSDataType::String => "STRING",
                _ => unreachable!(),
            }
        ));
    }
    ddl.push(')');
    session.execute_non_query(&ddl)?;

    let mut names = vec![
        "region".to_string(),
        "device_id".to_string(),
        "vendor".to_string(),
        "model".to_string(),
    ];
    let mut types = vec![TSDataType::String; 4];
    let mut cats = vec![
        ColumnCategory::Tag,
        ColumnCategory::Tag,
        ColumnCategory::Attribute,
        ColumnCategory::Attribute,
    ];
    for (name, ty) in FIELD_DEFS {
        names.push(name.to_string());
        types.push(ty);
        cats.push(ColumnCategory::Field);
    }
    let mut tablet = Tablet::new_table("sensor_data", names, types, cats)?;
    for r in 0..10 {
        // Tags/attributes vary too: two regions, ten devices, two vendors.
        let mut row: Vec<Option<Value>> = vec![
            Some(Value::String(format!("region-{}", r % 2))),
            Some(Value::String(format!("dev-{r:02}"))),
            Some(Value::String(format!(
                "vendor-{}",
                if r < 5 { "A" } else { "B" }
            ))),
            Some(Value::String(format!("model-{}", 100 + r))),
        ];
        row.extend((0..10).map(|c| field_value(c, r)));
        tablet.add_row(1000 + r as i64, row)?;
    }
    session.insert(&tablet)?;

    println!("-- read back: SELECT * FROM sensor_data ORDER BY device_id --");
    let n =
        print_resultset(session.execute_query("SELECT * FROM sensor_data ORDER BY device_id")?)?;
    println!("-- {n} rows --");
    session.close()?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let host = args.first().map(String::as_str).unwrap_or("127.0.0.1");
    let port: u16 = args.get(1).and_then(|p| p.parse().ok()).unwrap_or(6667);
    let user = args.get(2).map(String::as_str).unwrap_or("root");
    let password = args.get(3).map(String::as_str).unwrap_or("root");

    tree_model(host, port, user, password)?;
    table_model(host, port, user, password)?;
    println!("\nDone. Databases {TREE_DB} (tree) and {TABLE_DB} (table) kept for inspection.");
    Ok(())
}
