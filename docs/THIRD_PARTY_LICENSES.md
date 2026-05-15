# Third-Party Licenses

Language: EN | [FR](THIRD_PARTY_LICENSES.fr.md)

HeelonVault incorporates or links against third-party software. This document
lists those components and their license terms.

> **Machine-readable inventory:** The full SBOM (Software Bill of Materials) in
> CycloneDX 1.4 JSON format is available at [`sbom.cyclonedx.json`](sbom.cyclonedx.json).
> It is regenerated automatically on every release and can be ingested by tools
> such as OWASP Dependency-Track, Grype, or Trivy.

## 1. System Libraries (dynamically linked at runtime)

These libraries are **not embedded** in the HeelonVault binary. They are loaded
from the operating system at runtime via the dynamic linker. No LGPL code is
statically compiled into the HeelonVault executable.

| Library | Version | License | Source |
| ------- | ------- | ------- | ------ |
| GTK 4 | ≥ 4.6 | LGPL-2.1-or-later | <https://gitlab.gnome.org/GNOME/gtk> |
| libadwaita | ≥ 1.2 | LGPL-2.1-or-later | <https://gitlab.gnome.org/GNOME/libadwaita> |
| GLib / GObject / GIO | ≥ 2.66 | LGPL-2.1-or-later | <https://gitlab.gnome.org/GNOME/glib> |
| Pango | current | LGPL-2.1-or-later | <https://gitlab.gnome.org/GNOME/pango> |
| Cairo | current | LGPL-2.1-or-later | <https://gitlab.freedesktop.org/cairo/cairo> |
| SQLite | ≥ 3.35 | Public Domain | <https://www.sqlite.org> |

> In compliance with LGPL-2.1, users may replace these libraries with compatible
> versions. Copies of the LGPL-2.1 text can be found at
> <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>

---

## 2. Rust Crates (statically compiled into the binary)

The following crates are compiled into the HeelonVault binary. All are
permissive open-source licenses (MIT, Apache-2.0, BSD, ISC, Unicode, or
equivalent). The full license texts are available via
`https://crates.io/crates/<name>` or in the project's `Cargo.lock`.

### 2.1 Key Application Dependencies

| Crate | Version | License | Purpose |
| ----- | ------- | ------- | ------- |
| tokio | 1.50 | MIT | Async runtime |
| sqlx | 0.8.6 | MIT OR Apache-2.0 | SQLite async ORM / migrations |
| gtk4 | 0.9.7 | MIT | GTK4 Rust bindings |
| libadwaita | 0.7.2 | MIT | libadwaita Rust bindings |
| argon2 | 0.5.3 | MIT OR Apache-2.0 | Password hashing (Argon2id) |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT | AEAD encryption (AES-256-GCM) |
| totp-rs | 5.7.1 | MIT | TOTP / 2FA authentication |
| ring | 0.17.14 | Apache-2.0 AND ISC | Cryptographic primitives |
| rustls | 0.23.37 | Apache-2.0 OR ISC OR MIT | TLS 1.3 |
| zxcvbn | 2.2.2 | MIT | Password strength estimation |
| uuid | 1.22.0 | Apache-2.0 OR MIT | UUID generation |
| serde | 1.0.228 | MIT OR Apache-2.0 | Serialization framework |
| serde_json | 1.0.149 | MIT OR Apache-2.0 | JSON serialization |
| anyhow | 1.0.102 | MIT OR Apache-2.0 | Error handling |
| thiserror | 2.0.18 | MIT OR Apache-2.0 | Error type derivation |
| tracing | 0.1.44 | MIT | Structured logging |
| tracing-subscriber | 0.3.23 | MIT | Log subscriber/appender |
| chrono | 0.4.44 | MIT OR Apache-2.0 | Date and time |
| image | 0.25.10 | MIT OR Apache-2.0 | Image processing |
| qrcode | 0.14.1 | MIT OR Apache-2.0 | QR code generation (TOTP setup) |
| secrecy | 0.10.3 | Apache-2.0 OR MIT | Secret value zeroization |

### 2.2 Full Transitive Dependency Table

<!-- markdownlint-disable MD013 -->

| Crate | Version | License |
| ----- | ------- | ------- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aead | 0.5.2 | MIT OR Apache-2.0 |
| aes | 0.8.4 | MIT OR Apache-2.0 |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT |
| aho-corasick | 1.1.4 | Unlicense OR MIT |
| aligned | 0.4.3 | MIT OR Apache-2.0 |
| aligned-vec | 0.6.4 | MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| android_system_properties | 0.1.5 | MIT/Apache-2.0 |
| anes | 0.1.6 | MIT OR Apache-2.0 |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anyhow | 1.0.102 | MIT OR Apache-2.0 |
| arbitrary | 1.4.2 | MIT OR Apache-2.0 |
| arg_enum_proc_macro | 0.3.4 | MIT |
| argon2 | 0.5.3 | MIT OR Apache-2.0 |
| arrayvec | 0.7.6 | MIT OR Apache-2.0 |
| as-slice | 0.2.1 | MIT OR Apache-2.0 |
| atoi | 2.0.0 | MIT |
| autocfg | 1.5.0 | Apache-2.0 OR MIT |
| av-scenechange | 0.14.1 | MIT |
| av1-grain | 0.2.5 | BSD-2-Clause |
| avif-serialize | 0.8.8 | BSD-3-Clause |
| base32 | 0.5.1 | MIT OR Apache-2.0 |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| base64ct | 1.8.3 | Apache-2.0 OR MIT |
| bip39 | 2.2.2 | CC0-1.0 |
| bit-set | 0.5.3 | MIT/Apache-2.0 |
| bit-set | 0.8.0 | Apache-2.0 OR MIT |
| bit-vec | 0.6.3 | MIT/Apache-2.0 |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT |
| bit_field | 0.10.3 | Apache-2.0/MIT |
| bitcoin_hashes | 0.14.1 | CC0-1.0 |
| bitflags | 2.11.0 | MIT OR Apache-2.0 |
| bitstream-io | 4.9.0 | MIT/Apache-2.0 |
| blake2 | 0.10.6 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| built | 0.8.0 | MIT |
| bumpalo | 3.20.2 | MIT OR Apache-2.0 |
| bytemuck | 1.25.0 | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | Unlicense OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| bytes | 1.11.1 | MIT |
| cairo-rs | 0.20.12 | MIT |
| cairo-sys-rs | 0.20.10 | MIT |
| cast | 0.3.0 | MIT OR Apache-2.0 |
| cc | 1.2.57 | MIT OR Apache-2.0 |
| cfg-expr | 0.20.7 | MIT OR Apache-2.0 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| chrono | 0.4.44 | MIT OR Apache-2.0 |
| ciborium | 0.2.2 | Apache-2.0 |
| ciborium-io | 0.2.2 | Apache-2.0 |
| ciborium-ll | 0.2.2 | Apache-2.0 |
| cipher | 0.4.4 | MIT OR Apache-2.0 |
| clap | 4.6.0 | MIT OR Apache-2.0 |
| clap_builder | 4.6.0 | MIT OR Apache-2.0 |
| clap_lex | 1.1.0 | MIT OR Apache-2.0 |
| color_quant | 1.1.0 | MIT |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT |
| const-oid | 0.9.6 | Apache-2.0 OR MIT |
| constant_time_eq | 0.3.1 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| core2 | 0.4.0 | Apache-2.0 OR MIT |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| crc | 3.4.0 | MIT OR Apache-2.0 |
| crc-catalog | 2.4.0 | MIT OR Apache-2.0 |
| crc32fast | 1.5.0 | MIT OR Apache-2.0 |
| criterion | 0.5.1 | Apache-2.0 OR MIT |
| criterion-plot | 0.5.0 | MIT/Apache-2.0 |
| crossbeam-channel | 0.5.15 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.6 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.18 | MIT OR Apache-2.0 |
| crossbeam-queue | 0.3.12 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.21 | MIT OR Apache-2.0 |
| crunchy | 0.2.4 | MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| csv | 1.4.0 | Unlicense/MIT |
| csv-core | 0.1.13 | Unlicense/MIT |
| ctr | 0.9.2 | MIT OR Apache-2.0 |
| darling | 0.14.4 | MIT |
| darling_core | 0.14.4 | MIT |
| darling_macro | 0.14.4 | MIT |
| der | 0.7.10 | Apache-2.0 OR MIT |
| deranged | 0.5.8 | MIT OR Apache-2.0 |
| derive_builder | 0.12.0 | MIT/Apache-2.0 |
| derive_builder_core | 0.12.0 | MIT/Apache-2.0 |
| derive_builder_macro | 0.12.0 | MIT/Apache-2.0 |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| displaydoc | 0.2.5 | MIT OR Apache-2.0 |
| dotenvy | 0.15.7 | MIT |
| either | 1.15.0 | MIT OR Apache-2.0 |
| equator | 0.4.2 | MIT |
| equator-macro | 0.4.2 | MIT |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| etcetera | 0.8.0 | MIT OR Apache-2.0 |
| event-listener | 5.4.1 | Apache-2.0 OR MIT |
| exr | 1.74.0 | BSD-3-Clause |
| fancy-regex | 0.11.0 | MIT |
| fastrand | 2.3.0 | Apache-2.0 OR MIT |
| fax | 0.2.6 | MIT |
| fax_derive | 0.2.0 | MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| field-offset | 0.3.6 | MIT OR Apache-2.0 |
| find-msvc-tools | 0.1.9 | MIT OR Apache-2.0 |
| flate2 | 1.1.9 | MIT OR Apache-2.0 |
| flume | 0.11.1 | Apache-2.0/MIT |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| foldhash | 0.1.5 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| futures-channel | 0.3.32 | MIT OR Apache-2.0 |
| futures-core | 0.3.32 | MIT OR Apache-2.0 |
| futures-executor | 0.3.32 | MIT OR Apache-2.0 |
| futures-intrusive | 0.5.0 | MIT OR Apache-2.0 |
| futures-io | 0.3.32 | MIT OR Apache-2.0 |
| futures-macro | 0.3.32 | MIT OR Apache-2.0 |
| futures-sink | 0.3.32 | MIT OR Apache-2.0 |
| futures-task | 0.3.32 | MIT OR Apache-2.0 |
| futures-timer | 3.0.3 | MIT/Apache-2.0 |
| futures-util | 0.3.32 | MIT OR Apache-2.0 |
| gdk-pixbuf | 0.20.10 | MIT |
| gdk-pixbuf-sys | 0.20.10 | MIT |
| gdk4 | 0.9.6 | MIT |
| gdk4-sys | 0.9.6 | MIT |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.2 | MIT OR Apache-2.0 |
| ghash | 0.5.1 | Apache-2.0 OR MIT |
| gif | 0.14.1 | MIT OR Apache-2.0 |
| gio | 0.20.12 | MIT |
| gio-sys | 0.20.10 | MIT |
| glib | 0.20.12 | MIT |
| glib-build-tools | 0.20.0 | MIT |
| glib-macros | 0.20.12 | MIT |
| glib-sys | 0.20.10 | MIT |
| glob | 0.3.3 | MIT OR Apache-2.0 |
| gobject-sys | 0.20.10 | MIT |
| graphene-rs | 0.20.10 | MIT |
| graphene-sys | 0.20.10 | MIT |
| gsk4 | 0.9.6 | MIT |
| gsk4-sys | 0.9.6 | MIT |
| gtk4 | 0.9.7 | MIT |
| gtk4-macros | 0.9.5 | MIT |
| gtk4-sys | 0.9.6 | MIT |
| half | 2.7.1 | MIT OR Apache-2.0 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashlink | 0.10.0 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| hermit-abi | 0.5.2 | MIT OR Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| hex-conservative | 0.2.2 | CC0-1.0 |
| hkdf | 0.12.4 | MIT OR Apache-2.0 |
| hmac | 0.12.1 | MIT OR Apache-2.0 |
| home | 0.5.12 | MIT OR Apache-2.0 |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 |
| iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 |
| icu_collections | 2.1.1 | Unicode-3.0 |
| icu_locale_core | 2.1.1 | Unicode-3.0 |
| icu_normalizer | 2.1.1 | Unicode-3.0 |
| icu_normalizer_data | 2.1.1 | Unicode-3.0 |
| icu_properties | 2.1.2 | Unicode-3.0 |
| icu_properties_data | 2.1.2 | Unicode-3.0 |
| icu_provider | 2.1.1 | Unicode-3.0 |
| id-arena | 2.3.0 | MIT/Apache-2.0 |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.1 | Apache-2.0 OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| image-webp | 0.2.4 | MIT OR Apache-2.0 |
| imgref | 1.12.0 | CC0-1.0 OR Apache-2.0 |
| indexmap | 2.13.0 | Apache-2.0 OR MIT |
| inout | 0.1.4 | MIT OR Apache-2.0 |
| interpolate_name | 0.2.4 | MIT |
| is-terminal | 0.4.17 | MIT |
| itertools | 0.10.5 | MIT/Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.17 | MIT OR Apache-2.0 |
| jobserver | 0.1.34 | MIT OR Apache-2.0 |
| js-sys | 0.3.91 | MIT OR Apache-2.0 |
| lazy_static | 1.5.0 | MIT OR Apache-2.0 |
| leb128fmt | 0.1.0 | MIT OR Apache-2.0 |
| lebe | 0.5.3 | BSD-3-Clause |
| libadwaita | 0.7.2 | MIT |
| libadwaita-sys | 0.7.2 | MIT |
| libc | 0.2.183 | MIT OR Apache-2.0 |
| libfuzzer-sys | 0.4.12 | (MIT OR Apache-2.0) AND NCSA |
| libm | 0.2.16 | MIT |
| libredox | 0.1.14 | MIT |
| libsqlite3-sys | 0.30.1 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.1 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.29 | MIT OR Apache-2.0 |
| loop9 | 0.1.5 | MIT |
| matchers | 0.2.0 | MIT |
| maybe-rayon | 0.1.1 | MIT |
| md-5 | 0.10.6 | MIT OR Apache-2.0 |
| memchr | 2.8.0 | Unlicense OR MIT |
| memoffset | 0.9.1 | MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.1.1 | MIT |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| new_debug_unreachable | 1.0.6 | MIT |
| nom | 8.0.0 | MIT |
| noop_proc_macro | 0.3.0 | MIT |
| nu-ansi-term | 0.50.3 | MIT |
| num-bigint | 0.4.6 | MIT OR Apache-2.0 |
| num-bigint-dig | 0.8.6 | MIT/Apache-2.0 |
| num-conv | 0.2.0 | MIT OR Apache-2.0 |
| num-derive | 0.4.2 | MIT OR Apache-2.0 |
| num-integer | 0.1.46 | MIT OR Apache-2.0 |
| num-iter | 0.1.45 | MIT OR Apache-2.0 |
| num-rational | 0.4.2 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| oorandom | 11.1.5 | MIT |
| opaque-debug | 0.3.1 | MIT OR Apache-2.0 |
| pango | 0.20.12 | MIT |
| pango-sys | 0.20.10 | MIT |
| parking | 2.2.1 | Apache-2.0 OR MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| password-hash | 0.5.0 | MIT OR Apache-2.0 |
| paste | 1.0.15 | MIT OR Apache-2.0 |
| pastey | 0.1.1 | MIT OR Apache-2.0 |
| pem-rfc7468 | 0.7.0 | Apache-2.0 OR MIT |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkcs1 | 0.7.5 | Apache-2.0 OR MIT |
| pkcs8 | 0.10.2 | Apache-2.0 OR MIT |
| pkg-config | 0.3.32 | MIT OR Apache-2.0 |
| plain | 0.2.3 | MIT/Apache-2.0 |
| plotters | 0.3.7 | MIT |
| plotters-backend | 0.3.7 | MIT |
| plotters-svg | 0.3.7 | MIT |
| png | 0.18.1 | MIT OR Apache-2.0 |
| polyval | 0.6.2 | Apache-2.0 OR MIT |
| potential_utf | 0.1.4 | Unicode-3.0 |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| prettyplease | 0.2.37 | MIT OR Apache-2.0 |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.106 | MIT OR Apache-2.0 |
| profiling | 1.0.17 | MIT OR Apache-2.0 |
| profiling-procmacros | 1.0.17 | MIT OR Apache-2.0 |
| proptest | 1.10.0 | MIT OR Apache-2.0 |
| pxfm | 0.1.28 | BSD-3-Clause OR Apache-2.0 |
| qoi | 0.4.1 | MIT/Apache-2.0 |
| qrcode | 0.14.1 | MIT OR Apache-2.0 |
| quick-error | 1.2.3 | MIT/Apache-2.0 |
| quick-error | 2.0.1 | MIT/Apache-2.0 |
| quote | 1.0.45 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.9.3 | MIT OR Apache-2.0 |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.6.4 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| rand_xorshift | 0.4.0 | MIT OR Apache-2.0 |
| rav1e | 0.8.1 | BSD-2-Clause |
| ravif | 0.13.0 | BSD-3-Clause |
| rayon | 1.11.0 | MIT OR Apache-2.0 |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| redox_syscall | 0.7.3 | MIT |
| regex | 1.12.3 | MIT OR Apache-2.0 |
| regex-automata | 0.4.14 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.10 | MIT OR Apache-2.0 |
| relative-path | 1.9.3 | MIT OR Apache-2.0 |
| rgb | 0.8.53 | MIT |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rsa | 0.9.10 | MIT OR Apache-2.0 |
| rstest | 0.24.0 | MIT OR Apache-2.0 |
| rstest_macros | 0.24.0 | MIT OR Apache-2.0 |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | 0.23.37 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.14.0 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.9 | ISC |
| rustversion | 1.0.22 | MIT OR Apache-2.0 |
| rusty-fork | 0.3.1 | MIT/Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| same-file | 1.0.6 | Unlicense/MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| secrecy | 0.10.3 | Apache-2.0 OR MIT |
| semver | 1.0.27 | MIT OR Apache-2.0 |
| serde | 1.0.228 | MIT OR Apache-2.0 |
| serde_core | 1.0.228 | MIT OR Apache-2.0 |
| serde_derive | 1.0.228 | MIT OR Apache-2.0 |
| serde_json | 1.0.149 | MIT OR Apache-2.0 |
| serde_spanned | 1.0.4 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 |
| sha1 | 0.10.6 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sharded-slab | 0.1.7 | MIT |
| shlex | 1.3.0 | MIT OR Apache-2.0 |
| signature | 2.2.0 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.8 | MIT |
| simd_helpers | 0.1.0 | MIT |
| slab | 0.4.12 | MIT |
| smallvec | 1.15.1 | MIT OR Apache-2.0 |
| socket2 | 0.6.3 | MIT OR Apache-2.0 |
| spin | 0.9.8 | MIT |
| spki | 0.7.3 | Apache-2.0 OR MIT |
| sqlx | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-core | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-macros | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-macros-core | 0.8.6 | MIT OR Apache-2.0 |
| sqlx-sqlite | 0.8.6 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| stringprep | 0.1.5 | MIT/Apache-2.0 |
| strsim | 0.10.0 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| syn | 1.0.109 | MIT OR Apache-2.0 |
| syn | 2.0.117 | MIT OR Apache-2.0 |
| synstructure | 0.13.2 | MIT |
| system-deps | 7.0.7 | MIT OR Apache-2.0 |
| target-lexicon | 0.13.3 | Apache-2.0 WITH LLVM-exception |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| thiserror | 2.0.18 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.18 | MIT OR Apache-2.0 |
| thread_local | 1.1.9 | MIT OR Apache-2.0 |
| tiff | 0.11.3 | MIT |
| time | 0.3.47 | MIT OR Apache-2.0 |
| time-core | 0.1.8 | MIT OR Apache-2.0 |
| time-macros | 0.2.27 | MIT OR Apache-2.0 |
| tinystr | 0.8.2 | Unicode-3.0 |
| tinytemplate | 1.2.1 | Apache-2.0 OR MIT |
| tinyvec | 1.11.0 | Zlib OR Apache-2.0 OR MIT |
| tinyvec_macros | 0.1.1 | MIT OR Apache-2.0 OR Zlib |
| tokio | 1.50.0 | MIT |
| tokio-macros | 2.6.1 | MIT |
| tokio-stream | 0.1.18 | MIT |
| toml | 0.9.12 | MIT OR Apache-2.0 |
| toml_datetime | 0.7.5 | MIT OR Apache-2.0 |
| toml_datetime | 1.0.0 | MIT OR Apache-2.0 |
| toml_edit | 0.25.4 | MIT OR Apache-2.0 |
| toml_parser | 1.0.9 | MIT OR Apache-2.0 |
| toml_writer | 1.0.6 | MIT OR Apache-2.0 |
| totp-rs | 5.7.1 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-appender | 0.2.4 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tracing-log | 0.2.0 | MIT |
| tracing-serde | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| typenum | 1.19.0 | MIT OR Apache-2.0 |
| unarray | 0.1.4 | MIT OR Apache-2.0 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 |
| unicode-properties | 0.1.4 | MIT/Apache-2.0 |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 |
| universal-hash | 0.5.1 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| url | 2.5.8 | MIT OR Apache-2.0 |
| urlencoding | 2.1.3 | MIT |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| uuid | 1.22.0 | Apache-2.0 OR MIT |
| v_frame | 0.3.9 | BSD-2-Clause |
| valuable | 0.1.1 | MIT |
| vcpkg | 0.2.15 | MIT/Apache-2.0 |
| version-compare | 0.2.1 | MIT |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| wait-timeout | 0.2.1 | MIT/Apache-2.0 |
| walkdir | 2.5.0 | Unlicense/MIT |
| wasi | 0.11.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.2 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip3 | 0.4.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasite | 0.1.0 | Apache-2.0 OR BSL-1.0 OR MIT |
| wasm-bindgen | 0.2.114 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.114 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.114 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.114 | MIT OR Apache-2.0 |
| wasm-encoder | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-metadata | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasmparser | 0.244.0 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| web-sys | 0.3.91 | MIT OR Apache-2.0 |
| webpki-roots | 0.26.11 | CDLA-Permissive-2.0 |
| webpki-roots | 1.0.6 | CDLA-Permissive-2.0 |
| weezl | 0.1.12 | MIT OR Apache-2.0 |
| zxcvbn | 2.2.2 | MIT |

<!-- markdownlint-enable MD013 -->

---

## 3. License Summary

| License | Count | Notes |
| ------- | ----- | ----- |
| MIT OR Apache-2.0 (and variants) | ~380 | Fully permissive |
| MIT only | ~89 | Fully permissive |
| Unicode-3.0 | 18 | Permissive (Unicode data) |
| BSD-2-Clause / BSD-3-Clause | 8 | Permissive |
| Apache-2.0 WITH LLVM-exception | 14 | Permissive (LLVM exception) |
| Apache-2.0 AND ISC | 1 | `ring` — permissive |
| ISC | 3 | Permissive |
| CC0-1.0 | 3 | Public domain equivalent |
| Unlicense/MIT | 5 | Public domain / permissive |
| Zlib | 1 | Permissive |
| CDLA-Permissive-2.0 | 2 | `webpki-roots` data license |

> No copyleft licenses (GPL, LGPL, AGPL, EUPL) are present in the statically
> compiled Rust dependency tree. The only LGPL components are the system
> libraries listed in Section 1, which are dynamically linked.

---

*This file was generated on 2026-03-23 using `cargo metadata`. Regenerate with:*

```bash
cargo metadata --format-version 1 | python3 scripts/gen-third-party-licenses.py
```
