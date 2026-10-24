# User-guide screenshots

## Scope

The user guides use real production GTK widgets, styles and translations rendered
with synthetic data. They are not mockups, recordings of a personal account or
proof of a complete end-to-end user session. The security report and available
release are simulated; the release shown is not a publication announcement.
No recovery phrase, private key, TOTP seed or usable credential is captured.

The capture scenarios cover cards/list, search, all four editor types, the
security center, Community/Professional profiles and update discovery. Startup,
recovery and administrative operations remain documented as procedures rather
than illustrated with obsolete images. A shared native fixture also checks
ordinary-user restrictions; only the administrator illustrations are exported.

## Generate

On Linux with the repository toolchain, GTK development packages, DejaVu fonts,
`jq`, and the Premium checkout as a sibling:

```sh
bash scripts/generate-user-guide-screenshots.sh
bash scripts/check-user-guide-assets.sh
```

Use a display at least 1600 by 1500 pixels, or run in Xvfb:

```sh
dbus-run-session -- xvfb-run --auto-servernum \
  --server-args="-screen 0 1920x1600x24" \
  bash scripts/generate-user-guide-screenshots.sh
```

The manual `user-guide-screenshots` workflow uses Ubuntu 24.04 with Xvfb.
It uploads only images and their manifest for review, never build directories,
private sources or a database. It does not commit, merge or publish anything.
Download and inspect the artifact before replacing committed images. This
workflow is intentionally not triggered on ordinary PRs.

Generation runs an ignored documentation-only test in a separate process per
language. It isolates XDG configuration/data/state, uses in-memory SQLite and
mock release results, and removes its temporary directory on exit. It neither
opens a personal database nor calls GitHub. Cairo, light theme, scale 1 and
DejaVu Sans 10 are selected explicitly. Each process uses a fresh test fixture.

Generated files are copied into the guide only after both scenarios succeed and
all expected PNGs exist. The manifest records source/Premium commits, whether
capture sources were modified, product version, renderer, font, toolkit versions
and SHA-256 hashes. It does not claim pixel identity across GTK/font versions or
operating systems.

## Review and release gate

1. Commit capture tooling / UI changes before final generation, so provenance
   identifies committed sources. Generate the images in a second commit.
2. Open every image at its native size. Check translated labels, selected
   controls, text clipping, plausible demo data and absence of sensitive values.
3. Read both guides alongside the images; test instructions against the
   application. Keep stable task-based image names and relative links.
4. Run `bash scripts/check-user-guide-assets.sh --release` before tagging.
   Besides hash/PNG/language checks, it rejects a different product version,
   dirty capture sources, changed UI/assets/translations/dependencies since
   capture, or a different Premium revision. A failure means re-capture and
   review, not editing the provenance to make it pass.
5. Check rendered Markdown in the documentation site's preview when available.
   The site synchronizes the latest release, not arbitrary commits on `main`.
6. Native Windows/macOS QA remains necessary. Linux captures are illustrations,
   not evidence of native platform rendering.

Ordinary CI performs only the inexpensive asset integrity and bilingual-link
checks. It does not require graphical generation on every PR. Source freshness
is checked at release preparation; review any visible UI change earlier as well.

Do not add version numbers to the guide's prose or asset filenames. Version
history belongs in the changelog and capture provenance in the manifest.
