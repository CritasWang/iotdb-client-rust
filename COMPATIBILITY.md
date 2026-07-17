<!--
Licensed to the Apache Software Foundation (ASF) under one
or more contributor license agreements.  See the NOTICE file
distributed with this work for additional information
regarding copyright ownership.  The ASF licenses this file
to you under the Apache License, Version 2.0 (the
"License"); you may not use this file except in compliance
with the License.  You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing,
software distributed under the License is distributed on an
"AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, either express or implied.  See the License for the
specific language governing permissions and limitations
under the License.
-->

# Compatibility & Release Policy

This document describes which IoTDB server versions this client targets, the
exact protocol toolchain each crate release is built from, and the versioning
rules the crate follows.

## IoTDB server compatibility matrix

The client speaks IoTDB service protocol **V3** over Apache Thrift RPC and
targets the IoTDB **2.0.x** line. There is no version-specific branching in
the client code.

| IoTDB version | Status | Notes |
|---|---|---|
| 2.0.1 | Expected to work | Untested |
| 2.0.2 | Expected to work | Untested |
| 2.0.3 | Expected to work | Untested |
| 2.0.4 | Expected to work | Untested |
| 2.0.5 | Expected to work | Untested |
| 2.0.6 | **Tested** | CI runs the full live test suite + 3 examples against `apache/iotdb:2.0.6-standalone` on every push; TLS verified end-to-end (`enable_thrift_ssl`, TLSv1.3, full certificate verification) |
| 2.0.7 | Expected to work | Untested |
| 2.0.8 | Expected to work | Untested |
| 2.0.10 | **Tested** | Full benchmark + data-correctness verification (all 10 data types, tree & table models, nulls) against a standalone deployment; RPC compression verified (`dn_rpc_thrift_compression_enable=true`) |
| master | Expected to work | The Thrift IDL is synced from `apache/iotdb` master (`iotdb-protocol/`); untested beyond IDL lockstep |
| 1.x | **Not supported** | The table model and the TIMESTAMP / DATE / BLOB / STRING data types assume 2.x; no plan to support 1.x |

"Expected to work" means the version speaks protocol V3 and the same IDL
surface, but the client has not been exercised against it. Reports of success
or failure on untested versions are welcome as issues.

## Protocol toolchain (per release)

Each crate release records the exact IDL source and Thrift toolchain used to
generate `src/protocol/`:

| Crate version | IoTDB IDL source | Thrift compiler | `thrift` crate |
|---|---|---|---|
| 0.1.0 (unreleased) | `apache/iotdb` master, `iotdb-protocol/` @ `2fedd8a395` (2026-06-30, last change to `client.thrift`/`common.thrift`) | 0.23.0 (as pinned by the IoTDB pom's `thrift.version`) | 0.23 |

The generation pipeline is documented in [`tools/generate-thrift.sh`](./tools/generate-thrift.sh):
the IDL files (`thrift/client.thrift`, `thrift/common.thrift`) are synced from
`apache/iotdb`'s `iotdb-protocol/` module, and the Thrift compiler binary is
taken from the IoTDB Maven build output, which guarantees the compiler version
matches the one the server project pins. The generated stubs are committed and
must never be hand-edited.

This table is updated as part of every release.

## Versioning policy (SemVer)

The crate follows [Cargo's SemVer rules](https://doc.rust-lang.org/cargo/reference/semver.html):

- **Pre-1.0 (`0.x.y`)**: a bump of `x` may contain breaking API changes; a
  bump of `y` is additive or a bug fix. Cargo treats `0.x` as the
  compatibility boundary, so `^0.1` will never auto-upgrade to `0.2`.
- **Post-1.0**: breaking changes bump the major version, new functionality
  bumps the minor version, bug fixes bump the patch version.
- A regeneration of the Thrift stubs against a newer IDL is treated as
  breaking only if it changes the public Rust API surface; pure wire-level
  additions ship as minor/patch releases.

## Deprecation policy

- APIs slated for removal are first marked `#[deprecated]` (with a message
  pointing at the replacement) and remain functional for at least one minor
  release before removal.
- Removals happen only in releases that SemVer already marks as breaking
  (major bumps, or `0.x` bumps pre-1.0).
- Deprecations and removals are listed in the release notes.

## Release process

- Every release documents, in its release notes: the IoTDB IDL commit, the
  Thrift compiler version, and the tested IoTDB server versions (the table
  above is updated in the same change).
- CI continuously tests against both the oldest tested and the latest stable
  IoTDB versions (currently 2.0.6 and 2.0.10; see
  [`.github/workflows/ci.yml`](./.github/workflows/ci.yml)).
