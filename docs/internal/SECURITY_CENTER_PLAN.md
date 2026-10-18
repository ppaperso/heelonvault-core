# Security center implementation and validation

## Scope and sequence

Approved product scope: local diagnostics first, native GTK integration second, per-check
consent for DNS/TLS third. Reputation is conditional on provider feasibility, licensing,
privacy and operating cost; it must never be represented by a simulated green result.

1. Synchronize clean Core/Premium `main`; create `feature/2.1.0-security-center`.
2. Add semver-additive Core diagnostics and a shared stored-password estimator.
3. Integrate the native GTK page, correction navigation, local URL feedback and lifecycle.
4. Isolate DNS/TLS in an unpublished workspace crate; require consent for each check.
5. Align Core/App/Premium versions and both lockfiles; update FR/EN documentation.
6. Run targeted regressions, native visual smoke test, performance checks and release gates.

## Implemented UI

- Existing sidebar and root stack, not a separate modal.
- Mint/turquoise hero, shield, coverage and five filtering cards.
- All accessible vaults or one vault, grouped reuse, prioritized expandable findings.
- Progressive rendering in batches of 50, including within large reuse groups.
- Permission recheck before entering the existing editor; return to the center and rescan.
- Neutral/attention/error URL syntax diagnostics and an explicit per-check DNS/TLS dialog.
- No global safety score, compromise claim or automatic network scan.

The native GTK smoke test uses synthetic display data, not a real account/database.
It exercises card filtering and cleanup and can render previews without desktop capture:

```bash
GSK_RENDERER=cairo cargo test -p heelonvault-app --bin heelonvault \
  native_security_center_smoke -- --ignored --nocapture
```

Set `HEELONVAULT_UI_SNAPSHOT_DIR` to an existing directory to save wide/compact PNGs.
This test requires a graphical display and is explicitly selected rather than included
in headless unit-test runs. Linux CI runs it separately under Xvfb. Automated
diagnostics/network tests do not require a display.

## Privacy decisions

Core has no network dependency. The UI passes only an origin to the checker. No HTTP
request or page fetch is made, so paths, queries, fragments and remote page existence
are out of scope. DNS/SNI and client IP remain observable; consent explicitly says so.
Bundled trusted roots, hostname/date/chain validation; no custom CA or TLS bypass.
Revocation is not checked through online OCSP/CRL.

The production checker rejects internal/special addresses, validates every DNS answer and
pins the selected IP. Total timeout: 8 seconds; at most 16 DNS results and 2 connection
attempts. Cancellation abandons the result and connection; a started OS DNS lookup may
finish independently. Fixture keys/certificates are public test material only.

### Reputation feasibility outcome

No provider is enabled in this implementation. Google Safe Browsing's documented
non-commercial restriction is incompatible with assuming unrestricted commercial product
use. Web Risk requires a separate contractual/cost/privacy decision; its URL lookup
discloses the URL, while its update protocol requires canonicalization, list maintenance
and match confirmation. No third-party URL submissions were made.

References: [Safe Browsing](https://developers.google.com/safe-browsing),
[Web Risk](https://cloud.google.com/web-risk/docs/overview).

The UI and user/security docs explicitly say reputation is not checked. Enabling a provider
is a separate approved integration, not an ignored security advisory or a hidden fallback.

## Regression coverage

- Shared strength scoring, local URL cases and exact expiration boundaries.
- SQLite-backed cross-vault reuse; tokens excluded from password checks; corrupt
  ciphertext/metadata/dates; unavailable vaults; shared access and revocation.
- Pre-cancelled scans and worker-local zeroizing fingerprints.
- Controlled local TLS fixtures: valid handshake, hostname/root mismatch, non-TLS,
  timeout, cancellation, no application bytes and production loopback rejection.
- Privacy graph/source guards, bilingual catalogs and release consistency.
- Performance fixture sizes: 100 / 1,000 / 10,000 repeated-password items, with an explicit
  30-second per-scan regression budget. This measures repeated-password batch throughput,
  not worst-case unique-password estimation or peak RSS.

## Delivery boundary

No database migration, encryption/backup change or signed-report change. No release
publication or GitHub CI result is implied by local checks. Platform packaging and
accessibility/manual QA on Linux, Fedora, macOS and Windows remain release gates.
Validation results are recorded below after running the commands.

## Local validation results

Validated on the Linux development machine, Rust 1.99.1:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `cargo check --locked` | Passed |
| `cargo test --locked --all-targets --no-run` | Passed |
| `cargo test --locked --all-targets --all-features` | 358 passed; graphical smoke test selected separately |
| Native GTK smoke test | Passed: filters, selection retention, shared strength including whitespace, cleanup, 10,000-item model with bounded rendering |
| Community assembly/check and all-target Clippy | Passed with `--no-default-features` |
| Standalone Premium tests | 18 passed with all features |
| Rust 1.98 MSRV | All targets/features compiled in both repositories |
| `cargo audit` / `cargo deny check advisories` | Both lockfiles clean, no warning/violation |
| Full workspace `cargo deny check` | Advisories, bans, licenses and sources passed |
| `cargo semver-checks -p heelonvault-core` | 196 checks passed against the latest published baseline |
| `cargo publish --dry-run --allow-dirty -p heelonvault-core --locked` | Standalone package verified; nothing uploaded |
| Binary build / `--version` | Passed; reports the prepared product version |
| Workflow YAML / patch whitespace | Passed |

Repeated-password analysis measurements (debug build, same machine; not portable SLAs):
100 items: 53 ms; 1,000: 67 ms; 10,000: 841 ms. Distinct-password workloads and peak RSS
still require dedicated profiling before making performance claims beyond this fixture.

The binary SBOM and third-party inventory were regenerated. Core's new dependency on
zxcvbn also changes Premium's standalone lock graph; no existing dependency was upgraded.
Community-only compilation exposed a pre-existing misplaced feature gate around the
UI-only `LicenseDisplay` module. The gate was removed; premium-specific internals retain
their own gates, and both assemblies pass strict Clippy.

Linux CI now checks Community compilation, formatting of the new crate and the native
smoke test under Xvfb. These edited workflows have not run remotely yet. Merge Premium's
version/lock alignment before running Core CI against Premium `main`, as required by the
existing sibling-checkout release process. No commit, PR, tag or publication is part of
this local implementation step.
