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

# TLS test fixtures

Self-signed certificate + PKCS#8 key used only by the loopback TLS tests
(`connection::tls_tests`, cargo feature `tls`). Not a secret — the key
never protects anything.

macOS SecureTransport imposes extra requirements even on explicitly
trusted roots: validity ≤ 825 days (error −67901) and an
`extendedKeyUsage=serverAuth` extension (error −67609). Current cert
expires **2028-10-10**; when the trusted-root test starts failing with a
validity/expiry error, regenerate:

```sh
cd tests/fixtures/tls
openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem \
  -days 820 -nodes -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" \
  -addext "extendedKeyUsage=serverAuth" \
  -addext "keyUsage=digitalSignature,keyEncipherment"
```
