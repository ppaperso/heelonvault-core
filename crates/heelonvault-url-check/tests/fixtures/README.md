# TEST-ONLY TLS fixtures (heelonvault-url-check)

The `.fixture` suffix marks these files as test data (it also keeps them
out of the repository's `*.pem` / `*.key` ignore rules). They exist only for
the local TLS tests in `src/tests.rs`. Never use them outside the test suite.

- `ca.pem.fixture`: self-signed test certificate authority, `CN=HeelonVault URL check test CA`.
  Its private key was deleted right after it signed the server certificate.
- `leaf.pem.fixture`: test server certificate for `fixture.test` (DNS name only, no IP),
  issued by the test CA above.
- `leaf.key.fixture`: **public, synthetic private key** for `leaf.pem.fixture`. It protects nothing.
  It is committed on purpose and is **not a secret**.

These keys are **never used in production**. Production code only trusts the
`webpki-roots` trust store, and these files are not compiled into
non-test builds (they are only loaded via `include_bytes!` inside `#[cfg(test)]`).
`fixture.test` uses the reserved `.test` top-level domain (RFC 6761).

## Generation (OpenSSL, ECDSA P-256)

```sh
openssl ecparam -name prime256v1 -genkey -noout | openssl pkcs8 -topk8 -nocrypt -out ca.key
openssl req -x509 -new -key ca.key -sha256 -days 36500 \
  -subj "/CN=HeelonVault URL check test CA" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,keyCertSign,cRLSign" -out ca.pem
openssl ecparam -name prime256v1 -genkey -noout | openssl pkcs8 -topk8 -nocrypt -out leaf.key
openssl req -new -key leaf.key -subj "/CN=fixture.test" -out leaf.csr
printf '%s\n' 'basicConstraints=critical,CA:FALSE' 'keyUsage=critical,digitalSignature' \
  'extendedKeyUsage=serverAuth' 'subjectAltName=DNS:fixture.test' > leaf.ext
openssl x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial \
  -sha256 -days 36500 -extfile leaf.ext -out leaf.pem
rm ca.key ca.srl leaf.csr leaf.ext
mv ca.pem ca.pem.fixture && mv leaf.pem leaf.pem.fixture && mv leaf.key leaf.key.fixture
```

To regenerate, run the commands above in this directory and re-run the
crate tests.
