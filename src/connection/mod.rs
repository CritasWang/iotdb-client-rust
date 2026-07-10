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

//! Low-level Thrift connection to an IoTDB node.
//!
//! Mirrors `src/connection/Connection.ts` (Node.js) and the Thrift client setup
//! in the C# SDK: TCP → TFramedTransport → TBinaryProtocol → IClientRPCService client.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use thrift::protocol::{TBinaryInputProtocol, TBinaryOutputProtocol};
use thrift::transport::{
    ReadHalf, TFramedReadTransport, TFramedWriteTransport, TIoChannel, TTcpChannel, WriteHalf,
};

use crate::error::{Error, Result};
use crate::protocol::client::IClientRPCServiceSyncClient;

/// Default IoTDB DataNode RPC port.
pub const DEFAULT_PORT: u16 = 6667;

/// The concrete generated RPC client over framed transport + strict binary protocol.
pub type RpcClient = IClientRPCServiceSyncClient<
    TBinaryInputProtocol<TFramedReadTransport<ReadHalf<TTcpChannel>>>,
    TBinaryOutputProtocol<TFramedWriteTransport<WriteHalf<TTcpChannel>>>,
>;

/// A single endpoint `host:port` of an IoTDB DataNode (default port 6667).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

impl Endpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// Parse a `"host:port"` node-url string.
    ///
    /// Splits on the **last** `:` so IPv6 literals work; surrounding `[]`
    /// brackets on the host part are stripped (e.g. `"[::1]:6667"` → host `::1`).
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        let idx = s
            .rfind(':')
            .ok_or_else(|| Error::Client(format!("invalid node url '{s}': expected host:port")))?;
        let (host_part, port_part) = (&s[..idx], &s[idx + 1..]);
        let port: u16 = port_part.parse().map_err(|_| {
            Error::Client(format!("invalid node url '{s}': bad port '{port_part}'"))
        })?;
        let host = host_part
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host_part);
        if host.is_empty() {
            return Err(Error::Client(format!("invalid node url '{s}': empty host")));
        }
        Ok(Self::new(host, port))
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// Low-level connection wrapper. Owns the Thrift transport/protocol pair
/// and the generated `IClientRPCService` client.
pub struct Connection {
    endpoint: Endpoint,
    client: RpcClient,
}

impl Connection {
    /// Establish a TCP connection to `endpoint` (bounded by `connect_timeout`)
    /// and wrap it in framed transport + strict binary protocol.
    pub fn open(endpoint: Endpoint, connect_timeout: Duration) -> Result<Self> {
        let stream = connect_stream(&endpoint, connect_timeout)?;
        stream.set_nodelay(true).map_err(thrift::Error::from)?;

        let channel = TTcpChannel::with_stream(stream);
        let (read_half, write_half) = channel.split()?;
        let read_transport = TFramedReadTransport::new(read_half);
        let write_transport = TFramedWriteTransport::new(write_half);
        let input_protocol = TBinaryInputProtocol::new(read_transport, true);
        let output_protocol = TBinaryOutputProtocol::new(write_transport, true);
        let client = IClientRPCServiceSyncClient::new(input_protocol, output_protocol);

        Ok(Self { endpoint, client })
    }

    /// Mutable access to the generated RPC client for issuing calls.
    pub fn client_mut(&mut self) -> &mut RpcClient {
        &mut self.client
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

/// Resolve the endpoint and try each resolved address with the connect timeout.
fn connect_stream(endpoint: &Endpoint, connect_timeout: Duration) -> Result<TcpStream> {
    let addrs = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()
        .map_err(thrift::Error::from)?;
    let mut last_err: Option<std::io::Error> = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, connect_timeout) {
            Ok(stream) => return Ok(stream),
            Err(e) => last_err = Some(e),
        }
    }
    Err(match last_err {
        Some(e) => Error::Thrift(thrift::Error::from(e)),
        None => Error::Client(format!("could not resolve endpoint {endpoint}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipv4() {
        let ep = Endpoint::parse("127.0.0.1:6667").unwrap();
        assert_eq!(ep, Endpoint::new("127.0.0.1", 6667));
    }

    #[test]
    fn parse_hostname() {
        let ep = Endpoint::parse("iotdb.example.com:1234").unwrap();
        assert_eq!(ep, Endpoint::new("iotdb.example.com", 1234));
    }

    #[test]
    fn parse_ipv6_bracketed() {
        let ep = Endpoint::parse("[::1]:6667").unwrap();
        assert_eq!(ep, Endpoint::new("::1", 6667));

        let ep = Endpoint::parse("[2001:db8::1]:6668").unwrap();
        assert_eq!(ep, Endpoint::new("2001:db8::1", 6668));
    }

    #[test]
    fn parse_trims_whitespace() {
        let ep = Endpoint::parse("  localhost:6667 ").unwrap();
        assert_eq!(ep, Endpoint::new("localhost", 6667));
    }

    #[test]
    fn parse_no_port_is_error() {
        assert!(Endpoint::parse("localhost").is_err());
    }

    #[test]
    fn parse_bad_port_is_error() {
        assert!(Endpoint::parse("localhost:abc").is_err());
        assert!(Endpoint::parse("localhost:99999").is_err());
        assert!(Endpoint::parse("localhost:").is_err());
    }

    #[test]
    fn parse_empty_host_is_error() {
        assert!(Endpoint::parse(":6667").is_err());
        assert!(Endpoint::parse("[]:6667").is_err());
    }

    #[test]
    fn display_roundtrip() {
        assert_eq!(
            Endpoint::new("localhost", 6667).to_string(),
            "localhost:6667"
        );
        assert_eq!(Endpoint::new("::1", 6667).to_string(), "[::1]:6667");
        assert_eq!(
            Endpoint::parse(&Endpoint::new("::1", 6667).to_string()).unwrap(),
            Endpoint::new("::1", 6667)
        );
    }
}
