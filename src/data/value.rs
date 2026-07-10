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

//! A dynamically-typed IoTDB cell value.

use super::TSDataType;

/// One cell of an IoTDB row: a typed scalar or `Null`.
///
/// `Date` carries an `i32` in `yyyyMMdd` form (e.g. 2026-07-10 →
/// `20260710`), matching the C#/Java wire encoding. `Timestamp` is epoch
/// milliseconds.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Float(f32),
    Double(f64),
    Text(String),
    Timestamp(i64),
    /// Date as i32 `yyyyMMdd` (e.g. `20260710`).
    Date(i32),
    Blob(Vec<u8>),
    String(String),
    Null,
}

impl Value {
    /// The [`TSDataType`] this value carries, or `None` for [`Value::Null`].
    pub fn data_type(&self) -> Option<TSDataType> {
        Some(match self {
            Value::Boolean(_) => TSDataType::Boolean,
            Value::Int32(_) => TSDataType::Int32,
            Value::Int64(_) => TSDataType::Int64,
            Value::Float(_) => TSDataType::Float,
            Value::Double(_) => TSDataType::Double,
            Value::Text(_) => TSDataType::Text,
            Value::Timestamp(_) => TSDataType::Timestamp,
            Value::Date(_) => TSDataType::Date,
            Value::Blob(_) => TSDataType::Blob,
            Value::String(_) => TSDataType::String,
            Value::Null => return None,
        })
    }

    /// The wire type code (§8 of the protocol spec), or `None` for `Null`.
    pub fn type_code(&self) -> Option<i32> {
        self.data_type().map(TSDataType::code)
    }

    /// True iff this is [`Value::Null`].
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_codes() {
        assert_eq!(Value::Boolean(true).type_code(), Some(0));
        assert_eq!(Value::Int32(1).type_code(), Some(1));
        assert_eq!(Value::Int64(1).type_code(), Some(2));
        assert_eq!(Value::Float(1.0).type_code(), Some(3));
        assert_eq!(Value::Double(1.0).type_code(), Some(4));
        assert_eq!(Value::Text("t".into()).type_code(), Some(5));
        assert_eq!(Value::Timestamp(0).type_code(), Some(8));
        assert_eq!(Value::Date(20260710).type_code(), Some(9));
        assert_eq!(Value::Blob(vec![0]).type_code(), Some(10));
        assert_eq!(Value::String("s".into()).type_code(), Some(11));
        assert_eq!(Value::Null.type_code(), None);
    }

    #[test]
    fn data_type_of_null_is_none() {
        assert_eq!(Value::Null.data_type(), None);
        assert!(Value::Null.is_null());
        assert!(!Value::Int32(7).is_null());
    }
}
