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
| GTK 4 | ≥ 4.6 | LGPL-2.1-or-later | https://gitlab.gnome.org/GNOME/gtk |
| libadwaita | ≥ 1.2 | LGPL-2.1-or-later | https://gitlab.gnome.org/GNOME/libadwaita |
| GLib / GObject / GIO | ≥ 2.66 | LGPL-2.1-or-later | https://gitlab.gnome.org/GNOME/glib |
| Pango | current | LGPL-2.1-or-later | https://gitlab.gnome.org/GNOME/pango |
| Cairo | current | LGPL-2.1-or-later | https://gitlab.freedesktop.org/cairo/cairo |
| SQLite | ≥ 3.35 | Public Domain | https://www.sqlite.org |

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
| aes-gcm | 0.11.1 | Apache-2.0 OR MIT | AEAD encryption (AES-256-GCM) |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | Error handling |
| argon2 | 0.6.0 | MIT OR Apache-2.0 | Password hashing (Argon2id) |
| chrono | 0.4.45 | MIT OR Apache-2.0 | Date and time |
| gtk4 | 0.11.4 | MIT | GTK4 Rust bindings |
| image | 0.25.10 | MIT OR Apache-2.0 | Image processing |
| libadwaita | 0.9.2 | MIT | libadwaita Rust bindings |
| qrcode | 0.14.1 | MIT OR Apache-2.0 | QR code generation (TOTP setup) |
| ring | 0.17.14 | Apache-2.0 AND ISC | Cryptographic primitives |
| rustls | 0.23.44 | Apache-2.0 OR ISC OR MIT | TLS 1.3 |
| secrecy | 0.10.3 | Apache-2.0 OR MIT | Secret value zeroization |
| serde | 1.0.229 | MIT OR Apache-2.0 | Serialization framework |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | JSON serialization |
| thiserror | 2.0.20 | MIT OR Apache-2.0 | Error type derivation |
| tokio | 1.53.1 | MIT | Async runtime |
| totp-rs | 6.0.0 | MIT | TOTP / 2FA authentication |
| tracing | 0.1.44 | MIT | Structured logging |
| tracing-subscriber | 0.3.23 | MIT | Log subscriber/appender |
| uuid | 1.26.1 | Apache-2.0 OR MIT | UUID generation |
| zxcvbn | 3.1.1 | MIT | Password strength estimation |

### 2.2 Full Transitive Dependency Table

<!-- markdownlint-disable MD013 -->

| Crate | Version | License |
| ----- | ------- | ------- |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 |
| aead | 0.6.1 | MIT OR Apache-2.0 |
| aes | 0.9.3 | MIT OR Apache-2.0 |
| aes-gcm | 0.11.1 | Apache-2.0 OR MIT |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| alloca | 0.4.0 | MIT |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 |
| android_system_properties | 0.1.6 | MIT OR Apache-2.0 |
| anes | 0.1.6 | MIT OR Apache-2.0 |
| anstyle | 1.0.14 | MIT OR Apache-2.0 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| argon2 | 0.6.0 | MIT OR Apache-2.0 |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 |
| atoi | 2.0.0 | MIT |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| aws-lc-rs | 1.18.1 | ISC AND (Apache-2.0 OR ISC) |
| aws-lc-sys | 0.45.0 | ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) |
| base32 | 0.5.1 | MIT OR Apache-2.0 |
| base64 | 0.22.1 | MIT OR Apache-2.0 |
| base64 | 0.23.1 | MIT OR Apache-2.0 |
| base64ct | 1.8.3 | Apache-2.0 OR MIT |
| bip39 | 2.2.2 | CC0-1.0 |
| bit-set | 0.8.0 | Apache-2.0 OR MIT |
| bit-vec | 0.8.0 | Apache-2.0 OR MIT |
| bitcoin_hashes | 0.14.101 | CC0-1.0 |
| bitflags | 2.13.2 | MIT OR Apache-2.0 |
| blake2 | 0.11.0 | MIT OR Apache-2.0 |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 |
| bstr | 1.13.1 | MIT OR Apache-2.0 |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| byteorder | 1.5.0 | Unlicense OR MIT |
| byteorder-lite | 0.1.0 | Unlicense OR MIT |
| bytes | 1.12.1 | MIT |
| cairo-rs | 0.22.9 | MIT |
| cairo-sys-rs | 0.22.9 | MIT |
| cast | 0.3.0 | MIT OR Apache-2.0 |
| cc | 1.4.5 | MIT OR Apache-2.0 |
| cfg-expr | 0.20.9 | MIT OR Apache-2.0 |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| cfg_aliases | 0.2.2 | MIT |
| chacha20 | 0.10.2 | MIT OR Apache-2.0 |
| chrono | 0.4.45 | MIT OR Apache-2.0 |
| ciborium | 0.2.2 | Apache-2.0 |
| ciborium-io | 0.2.2 | Apache-2.0 |
| ciborium-ll | 0.2.2 | Apache-2.0 |
| cipher | 0.5.2 | MIT OR Apache-2.0 |
| clap | 4.6.6 | MIT OR Apache-2.0 |
| clap_builder | 4.6.6 | MIT OR Apache-2.0 |
| clap_lex | 1.1.0 | MIT OR Apache-2.0 |
| cmake | 0.1.58 | MIT OR Apache-2.0 |
| cmov | 0.5.4 | Apache-2.0 OR MIT |
| combine | 4.6.8 | MIT |
| const-oid | 0.10.2 | Apache-2.0 OR MIT |
| constant_time_eq | 0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| core-foundation | 0.10.1 | MIT OR Apache-2.0 |
| core-foundation-sys | 0.8.7 | MIT OR Apache-2.0 |
| core_detect | 1.0.0 | MIT/Apache-2.0 |
| cpubits | 0.1.1 | MIT OR Apache-2.0 |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 |
| crc | 3.4.0 | MIT OR Apache-2.0 |
| crc-catalog | 2.5.0 | MIT OR Apache-2.0 |
| crc32fast | 1.5.1 | MIT OR Apache-2.0 |
| criterion | 0.8.2 | Apache-2.0 OR MIT |
| criterion-plot | 0.8.2 | Apache-2.0 OR MIT |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 |
| crossbeam-queue | 0.3.14 | MIT OR Apache-2.0 |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 |
| crunchy | 0.2.4 | MIT |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 |
| csv | 1.4.0 | Unlicense/MIT |
| csv-core | 0.1.13 | Unlicense/MIT |
| ctr | 0.10.1 | MIT OR Apache-2.0 |
| ctutils | 0.4.2 | Apache-2.0 OR MIT |
| curve25519-dalek | 5.0.0 | BSD-3-Clause |
| curve25519-dalek-derive | 0.1.1 | MIT/Apache-2.0 |
| darling | 0.20.11 | MIT |
| darling_core | 0.20.11 | MIT |
| darling_macro | 0.20.11 | MIT |
| deranged | 0.5.8 | MIT OR Apache-2.0 |
| derive_builder | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_core | 0.20.2 | MIT OR Apache-2.0 |
| derive_builder_macro | 0.20.2 | MIT OR Apache-2.0 |
| digest | 0.10.7 | MIT OR Apache-2.0 |
| digest | 0.11.3 | MIT OR Apache-2.0 |
| directories | 6.0.0 | MIT OR Apache-2.0 |
| dirs-sys | 0.5.0 | MIT OR Apache-2.0 |
| dispatch2 | 0.3.1 | Zlib OR Apache-2.0 OR MIT |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 |
| dotenvy | 0.15.7 | MIT |
| dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| ed25519 | 3.0.0 | Apache-2.0 OR MIT |
| ed25519-dalek | 3.0.0 | BSD-3-Clause |
| either | 1.18.0 | MIT OR Apache-2.0 |
| encoding_rs | 0.8.41 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| errno | 0.3.14 | MIT OR Apache-2.0 |
| etcetera | 0.11.0 | MIT OR Apache-2.0 |
| event-listener | 5.4.2 | Apache-2.0 OR MIT |
| fancy-regex | 0.18.0 | MIT |
| fastrand | 2.5.0 | Apache-2.0 OR MIT |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 |
| fiat-crypto | 0.3.0 | MIT OR Apache-2.0 OR BSD-1-Clause |
| field-offset | 0.3.6 | MIT OR Apache-2.0 |
| find-msvc-tools | 0.1.12 | MIT OR Apache-2.0 |
| flate2 | 1.1.10 | MIT OR Apache-2.0 |
| fluent-bundle | 0.16.0 | Apache-2.0 OR MIT |
| fluent-langneg | 0.13.1 | Apache-2.0 OR MIT |
| fluent-syntax | 0.12.0 | Apache-2.0 OR MIT |
| fluent-template-macros | 0.15.1 | MIT/Apache-2.0 |
| fluent-templates | 0.15.1 | MIT/Apache-2.0 |
| flume | 0.11.1 | Apache-2.0/MIT |
| flume | 0.12.0 | Apache-2.0/MIT |
| fnv | 1.0.7 | Apache-2.0 / MIT |
| foldhash | 0.2.0 | Zlib |
| form_urlencoded | 1.2.2 | MIT OR Apache-2.0 |
| fs_extra | 1.3.0 | MIT |
| futures-channel | 0.3.34 | MIT OR Apache-2.0 |
| futures-core | 0.3.34 | MIT OR Apache-2.0 |
| futures-executor | 0.3.34 | MIT OR Apache-2.0 |
| futures-intrusive | 0.5.0 | MIT OR Apache-2.0 |
| futures-io | 0.3.34 | MIT OR Apache-2.0 |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 |
| futures-sink | 0.3.34 | MIT OR Apache-2.0 |
| futures-task | 0.3.34 | MIT OR Apache-2.0 |
| futures-timer | 3.0.4 | MIT/Apache-2.0 |
| futures-util | 0.3.34 | MIT OR Apache-2.0 |
| gdk-pixbuf | 0.22.0 | MIT |
| gdk-pixbuf-sys | 0.22.9 | MIT |
| gdk4 | 0.11.4 | MIT |
| gdk4-sys | 0.11.4 | MIT |
| generic-array | 0.14.7 | MIT |
| getrandom | 0.2.17 | MIT OR Apache-2.0 |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| getrandom | 0.4.3 | MIT OR Apache-2.0 |
| ghash | 0.6.0 | Apache-2.0 OR MIT |
| gio | 0.22.9 | MIT |
| gio-sys | 0.22.9 | MIT |
| glib | 0.22.9 | MIT |
| glib-build-tools | 0.22.8 | MIT |
| glib-macros | 0.22.9 | MIT |
| glib-sys | 0.22.9 | MIT |
| glob | 0.3.4 | MIT OR Apache-2.0 |
| globset | 0.4.20 | Unlicense OR MIT |
| gobject-sys | 0.22.9 | MIT |
| graphene-rs | 0.22.8 | MIT |
| graphene-sys | 0.22.9 | MIT |
| gsk4 | 0.11.4 | MIT |
| gsk4-sys | 0.11.4 | MIT |
| gtk4 | 0.11.4 | MIT |
| gtk4-macros | 0.11.4 | MIT |
| gtk4-sys | 0.11.4 | MIT |
| half | 2.7.1 | MIT OR Apache-2.0 |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| hashlink | 0.11.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| heelonvault-app | 1.2.0-rc.1 | Apache-2.0 |
| heelonvault-core | 1.2.0-rc.1 | Apache-2.0 |
| heelonvault-premium | 1.2.0-rc.1 | Apache-2.0 |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| hex-conservative | 0.2.3 | CC0-1.0 |
| hkdf | 0.13.0 | MIT OR Apache-2.0 |
| hmac | 0.13.0 | MIT OR Apache-2.0 |
| http | 1.5.0 | MIT OR Apache-2.0 |
| http-body | 1.1.0 | MIT |
| http-body-util | 0.1.5 | MIT |
| httparse | 1.10.1 | MIT OR Apache-2.0 |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 |
| hyper | 1.11.1 | MIT |
| hyper-rustls | 0.27.9 | Apache-2.0 OR ISC OR MIT |
| hyper-util | 0.1.20 | MIT |
| iana-time-zone | 0.1.65 | MIT OR Apache-2.0 |
| iana-time-zone-haiku | 0.1.2 | MIT OR Apache-2.0 |
| icu_collections | 2.3.0 | Unicode-3.0 |
| icu_locale_core | 2.3.0 | Unicode-3.0 |
| icu_normalizer | 2.3.0 | Unicode-3.0 |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 |
| icu_properties | 2.3.0 | Unicode-3.0 |
| icu_properties_data | 2.3.0 | Unicode-3.0 |
| icu_provider | 2.3.1 | Unicode-3.0 |
| ident_case | 1.0.1 | MIT/Apache-2.0 |
| idna | 1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | 1.2.2 | Apache-2.0 OR MIT |
| ignore | 0.4.33 | Unlicense OR MIT |
| image | 0.25.10 | MIT OR Apache-2.0 |
| indexmap | 2.14.2 | Apache-2.0 OR MIT |
| inout | 0.2.2 | MIT OR Apache-2.0 |
| intl-memoizer | 0.5.3 | Apache-2.0 OR MIT |
| intl_pluralrules | 7.0.2 | Apache-2.0/MIT |
| ipnet | 2.12.2 | MIT OR Apache-2.0 |
| itertools | 0.13.0 | MIT OR Apache-2.0 |
| itertools | 0.14.0 | MIT OR Apache-2.0 |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| jni | 0.22.4 | MIT OR Apache-2.0 |
| jni-macros | 0.22.4 | MIT OR Apache-2.0 |
| jni-sys | 0.4.1 | MIT OR Apache-2.0 |
| jni-sys-macros | 0.4.1 | MIT OR Apache-2.0 |
| jobserver | 0.1.35 | MIT OR Apache-2.0 |
| js-sys | 0.3.105 | MIT OR Apache-2.0 |
| lazy_static | 1.5.0 | MIT OR Apache-2.0 |
| libadwaita | 0.9.2 | MIT |
| libadwaita-sys | 0.9.2 | MIT |
| libc | 0.2.189 | MIT OR Apache-2.0 |
| libredox | 0.1.23 | MIT |
| libsqlite3-sys | 0.37.0 | MIT |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| litemap | 0.8.3 | Unicode-3.0 |
| lock_api | 0.4.14 | MIT OR Apache-2.0 |
| log | 0.4.34 | MIT OR Apache-2.0 |
| lru-slab | 0.1.2 | MIT OR Apache-2.0 OR Zlib |
| matchers | 0.2.0 | MIT |
| md-5 | 0.11.0 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| memoffset | 0.9.1 | MIT |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 |
| mio | 1.2.3 | MIT |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 |
| multiversion | 0.9.0 | MIT OR Apache-2.0 |
| multiversion-macros | 0.9.0 | MIT OR Apache-2.0 |
| multiversion_no_op | 1.0.0 | Apache-2.0 OR MIT |
| ndk-context | 0.1.1 | MIT OR Apache-2.0 |
| nu-ansi-term | 0.50.3 | MIT |
| num-conv | 0.2.2 | MIT OR Apache-2.0 |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| objc2 | 0.6.4 | MIT |
| objc2-app-kit | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-core-foundation | 0.3.2 | Zlib OR Apache-2.0 OR MIT |
| objc2-encode | 4.1.0 | MIT |
| objc2-foundation | 0.3.2 | MIT |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| oorandom | 11.1.5 | MIT |
| openssl-probe | 0.2.1 | MIT OR Apache-2.0 |
| option-ext | 0.2.0 | MPL-2.0 |
| page_size | 0.6.0 | MIT/Apache-2.0 |
| pango | 0.22.9 | MIT |
| pango-sys | 0.22.9 | MIT |
| parking | 2.2.1 | Apache-2.0 OR MIT |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 |
| password-hash | 0.6.1 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |
| phc | 0.6.1 | Apache-2.0 OR MIT |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 |
| plotters | 0.3.7 | MIT |
| plotters-backend | 0.3.7 | MIT |
| plotters-svg | 0.3.7 | MIT |
| png | 0.18.1 | MIT OR Apache-2.0 |
| polyval | 0.7.3 | Apache-2.0 OR MIT |
| potential_utf | 0.1.6 | Unicode-3.0 |
| powerfmt | 0.2.0 | MIT OR Apache-2.0 |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 |
| proc-macro-crate | 3.5.0 | MIT OR Apache-2.0 |
| proc-macro-hack | 0.5.20+deprecated | MIT OR Apache-2.0 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| proptest | 1.11.0 | MIT OR Apache-2.0 |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 |
| qrcode | 0.14.1 | MIT OR Apache-2.0 |
| quick-error | 1.2.3 | MIT/Apache-2.0 |
| quinn | 0.11.11 | MIT OR Apache-2.0 |
| quinn-proto | 0.11.17 | MIT OR Apache-2.0 |
| quinn-udp | 0.5.15 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| rand | 0.9.5 | MIT OR Apache-2.0 |
| rand | 0.10.2 | MIT OR Apache-2.0 |
| rand_chacha | 0.9.0 | MIT OR Apache-2.0 |
| rand_core | 0.9.5 | MIT OR Apache-2.0 |
| rand_core | 0.10.1 | MIT OR Apache-2.0 |
| rand_pcg | 0.10.2 | MIT OR Apache-2.0 |
| rand_xorshift | 0.4.0 | MIT OR Apache-2.0 |
| rayon | 1.12.0 | MIT OR Apache-2.0 |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 |
| redox_syscall | 0.5.18 | MIT |
| redox_users | 0.5.2 | MIT |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| relative-path | 1.9.3 | MIT OR Apache-2.0 |
| reqwest | 0.13.5 | MIT OR Apache-2.0 |
| ring | 0.17.14 | Apache-2.0 AND ISC |
| rstest | 0.27.0 | MIT OR Apache-2.0 |
| rstest_macros | 0.27.0 | MIT OR Apache-2.0 |
| rustc-hash | 2.1.3 | Apache-2.0 OR MIT |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 |
| rustix | 1.1.4 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | 0.23.44 | Apache-2.0 OR ISC OR MIT |
| rustls-native-certs | 0.8.4 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | 1.15.1 | MIT OR Apache-2.0 |
| rustls-platform-verifier | 0.7.0 | MIT OR Apache-2.0 |
| rustls-platform-verifier-android | 0.1.1 | MIT OR Apache-2.0 |
| rustls-webpki | 0.103.15 | ISC |
| rustversion | 1.0.23 | MIT OR Apache-2.0 |
| rusty-fork | 0.3.1 | MIT/Apache-2.0 |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 |
| same-file | 1.0.6 | Unlicense/MIT |
| schannel | 0.1.29 | MIT |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 |
| secrecy | 0.10.3 | Apache-2.0 OR MIT |
| security-framework | 3.7.0 | MIT OR Apache-2.0 |
| security-framework-sys | 2.17.0 | MIT OR Apache-2.0 |
| self_cell | 1.3.0 | Apache-2.0 OR GPL-2.0-only |
| semver | 1.0.28 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| serde_urlencoded | 0.7.1 | MIT/Apache-2.0 |
| sha1 | 0.11.0 | MIT OR Apache-2.0 |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| sha2 | 0.11.0 | MIT OR Apache-2.0 |
| sharded-slab | 0.1.7 | MIT |
| shlex | 2.0.1 | MIT OR Apache-2.0 |
| signature | 3.0.0 | Apache-2.0 OR MIT |
| simd-adler32 | 0.3.10 | MIT |
| simd_cesu8 | 1.2.0 | Apache-2.0 OR MIT |
| simdutf8 | 0.1.5 | MIT OR Apache-2.0 |
| slab | 0.4.12 | MIT |
| smallvec | 1.16.0 | MIT OR Apache-2.0 |
| socket2 | 0.6.5 | MIT OR Apache-2.0 |
| spin | 0.9.9 | MIT |
| sqlx-core | 0.9.0 | MIT OR Apache-2.0 |
| sqlx-macros | 0.9.0 | MIT OR Apache-2.0 |
| sqlx-macros-core | 0.9.0 | MIT OR Apache-2.0 |
| sqlx-mysql | 0.9.0 | MIT OR Apache-2.0 |
| sqlx-postgres | 0.9.0 | MIT OR Apache-2.0 |
| sqlx-sqlite | 0.9.0 | MIT OR Apache-2.0 |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 |
| stringprep | 0.1.5 | MIT/Apache-2.0 |
| strsim | 0.11.1 | MIT |
| subtle | 2.6.1 | BSD-3-Clause |
| symlink | 0.1.0 | MIT/Apache-2.0 |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.5 | MIT OR Apache-2.0 |
| sync_wrapper | 1.0.2 | Apache-2.0 |
| synstructure | 0.13.2 | MIT |
| system-deps | 7.0.8 | MIT OR Apache-2.0 |
| system-deps | 9.0.0 | MIT OR Apache-2.0 |
| target-lexicon | 0.13.5 | Apache-2.0 WITH LLVM-exception |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| thiserror | 2.0.20 | MIT OR Apache-2.0 |
| thiserror-impl | 2.0.20 | MIT OR Apache-2.0 |
| thread_local | 1.1.10 | MIT OR Apache-2.0 |
| time | 0.3.55 | MIT OR Apache-2.0 |
| time-core | 0.1.9 | MIT OR Apache-2.0 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 |
| tinystr | 0.8.4 | Unicode-3.0 |
| tinytemplate | 1.2.1 | Apache-2.0 OR MIT |
| tinyvec | 1.13.2 | Zlib OR Apache-2.0 OR MIT |
| tinyvec_macros | 0.1.1 | MIT OR Apache-2.0 OR Zlib |
| tokio | 1.53.1 | MIT |
| tokio-macros | 2.7.2 | MIT |
| tokio-rustls | 0.26.5 | MIT OR Apache-2.0 |
| tokio-stream | 0.1.19 | MIT |
| toml | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_datetime | 1.1.1+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_edit | 0.25.13+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_parser | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 |
| toml_writer | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 |
| totp-rs | 6.0.0 | MIT |
| tower | 0.5.3 | MIT |
| tower-http | 0.6.11 | MIT |
| tower-layer | 0.3.3 | MIT |
| tower-service | 0.3.3 | MIT |
| tracing | 0.1.44 | MIT |
| tracing-appender | 0.2.5 | MIT |
| tracing-attributes | 0.1.31 | MIT |
| tracing-core | 0.1.36 | MIT |
| tracing-log | 0.2.0 | MIT |
| tracing-serde | 0.2.0 | MIT |
| tracing-subscriber | 0.3.23 | MIT |
| trait-variant | 0.1.3 | MIT OR Apache-2.0 |
| try-lock | 0.2.5 | MIT |
| type-map | 0.5.1 | MIT/Apache-2.0 |
| typenum | 1.20.1 | MIT OR Apache-2.0 |
| unarray | 0.1.4 | MIT OR Apache-2.0 |
| unic-langid | 0.9.6 | MIT OR Apache-2.0 |
| unic-langid-impl | 0.9.6 | MIT OR Apache-2.0 |
| unic-langid-macros | 0.9.6 | MIT OR Apache-2.0 |
| unic-langid-macros-impl | 0.9.6 | MIT OR Apache-2.0 |
| unicode-bidi | 0.3.18 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-normalization | 0.1.25 | MIT OR Apache-2.0 |
| unicode-properties | 0.1.4 | MIT/Apache-2.0 |
| universal-hash | 0.6.1 | MIT OR Apache-2.0 |
| untrusted | 0.9.0 | ISC |
| url | 2.5.8 | MIT OR Apache-2.0 |
| urlencoding | 2.1.3 | MIT |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT |
| uuid | 1.26.1 | Apache-2.0 OR MIT |
| valuable | 0.1.1 | MIT |
| vcpkg | 0.2.15 | MIT/Apache-2.0 |
| version-compare | 0.2.1 | MIT |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| wait-timeout | 0.2.1 | MIT/Apache-2.0 |
| walkdir | 2.5.0 | Unlicense/MIT |
| want | 0.3.1 | MIT |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wasm-bindgen | 0.2.128 | MIT OR Apache-2.0 |
| wasm-bindgen-futures | 0.4.78 | MIT OR Apache-2.0 |
| wasm-bindgen-macro | 0.2.128 | MIT OR Apache-2.0 |
| wasm-bindgen-macro-support | 0.2.128 | MIT OR Apache-2.0 |
| wasm-bindgen-shared | 0.2.128 | MIT OR Apache-2.0 |
| web-sys | 0.3.105 | MIT OR Apache-2.0 |
| web-time | 1.1.0 | MIT OR Apache-2.0 |
| webbrowser | 1.2.4 | MIT OR Apache-2.0 |
| webpki-root-certs | 1.0.9 | CDLA-Permissive-2.0 |
| webpki-roots | 1.0.9 | CDLA-Permissive-2.0 |
| whoami | 2.1.3 | Apache-2.0 OR BSL-1.0 OR MIT |
| winapi | 0.3.9 | MIT/Apache-2.0 |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| winapi-util | 0.1.11 | Unlicense OR MIT |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 |
| windows-core | 0.62.2 | MIT OR Apache-2.0 |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 |
| windows-link | 0.2.1 | MIT OR Apache-2.0 |
| windows-result | 0.4.1 | MIT OR Apache-2.0 |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_aarch64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_i686_msvc | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnu | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_gnullvm | 0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 |
| winnow | 1.0.4 | MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| writeable | 0.6.4 | Unicode-3.0 |
| yoke | 0.8.3 | Unicode-3.0 |
| yoke-derive | 0.8.2 | Unicode-3.0 |
| zerocopy | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.57 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | 0.1.8 | Unicode-3.0 |
| zerofrom-derive | 0.1.7 | Unicode-3.0 |
| zeroize | 1.9.0 | Apache-2.0 OR MIT |
| zeroize_derive | 1.5.0 | Apache-2.0 OR MIT |
| zerotrie | 0.2.5 | Unicode-3.0 |
| zerovec | 0.11.8 | Unicode-3.0 |
| zerovec-derive | 0.11.6 | Unicode-3.0 |
| zlib-rs | 0.6.7 | Zlib |
| zmij | 1.0.23 | MIT |
| zxcvbn | 3.1.1 | MIT |

<!-- markdownlint-enable MD013 -->

---

## 3. License Summary

| License | Count | Notes |
| ------- | ----- | ----- |
| MIT OR Apache-2.0 (and variants) | ~229 | Fully permissive |
| MIT only | ~114 | Fully permissive |
| Unicode-3.0 | 19 | Permissive (Unicode data) |
| BSD-2-Clause / BSD-3-Clause | 7 | Permissive |
| Apache-2.0 WITH LLVM-exception | 6 | Permissive (LLVM exception) |
| Apache-2.0 AND ISC | 0 | `ring` — permissive |
| ISC | 8 | Permissive |
| CC0-1.0 | 5 | Public domain equivalent |
| Unlicense/MIT | 7 | Public domain / permissive |
| Zlib | 11 | Permissive |
| CDLA-Permissive-2.0 | 2 | `webpki-roots` data license |

> No copyleft licenses (GPL, LGPL, AGPL, EUPL) are present in the statically
> compiled Rust dependency tree. The only LGPL components are the system
> libraries listed in Section 1, which are dynamically linked.

---

*This file was generated using `cargo metadata`. Regenerate with:*

```bash
cargo metadata --format-version 1 | python3 scripts/gen-third-party-licenses.py
```

