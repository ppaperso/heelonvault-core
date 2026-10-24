#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if [[ "$(uname -s)" != Linux ]]; then
    echo "Documentation capture currently requires Linux/GTK; this is not native Windows/macOS validation." >&2
    exit 1
fi
WORK="$(mktemp -d -t heelonvault-docs.XXXXXXXX)"
trap 'rm -rf -- "$WORK"' EXIT
mkdir -p "$WORK/config" "$WORK/data" "$WORK/state" "$WORK/images"
touch "$WORK/config/heelonvault-documentation-fixture"
export XDG_CONFIG_HOME="$WORK/config"
export XDG_DATA_HOME="$WORK/data"
export XDG_STATE_HOME="$WORK/state"
export HEELONVAULT_DOC_OUTPUT="$WORK/images"
export GDK_SCALE=1 GDK_DPI_SCALE=1 GSK_RENDERER=cairo
export LC_ALL=C.UTF-8 TZ=UTC
unset HEELONVAULT_UI_SNAPSHOT_DIR

for language in fr en; do
    HEELONVAULT_DOC_LANGUAGE="$language" cargo test --locked -p heelonvault-app \
        --bin heelonvault \
        ui::windows::main_window::documentation_tests::documentation_screenshots \
        -- --ignored --exact --nocapture
done

names=(profile-community profile-professional security-center updates-available updates-consent
    profile-community-data profile-professional-data
    dashboard-grid dashboard-list search editor-password editor-api editor-ssh editor-document)
for language in fr en; do
    for name in "${names[@]}"; do
        test -s "$WORK/images/$language/$name.png"
    done
done
for language in fr en; do
    mkdir -p "docs/images/user-guide/$language"
    for name in "${names[@]}"; do
        cp "$WORK/images/$language/$name.png" "docs/images/user-guide/$language/$name.png"
    done
done

version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/heelonvault-app/Cargo.toml | head -1)"
files=()
for language in fr en; do
    for name in "${names[@]}"; do
        files+=("docs/images/user-guide/$language/$name.png")
    done
done
sha256sum "${files[@]}" | jq -Rn \
    --arg commit "$(git rev-parse HEAD)" \
    --arg version "$version" \
    --arg generated "$(date -u +%FT%TZ)" \
    --arg gtk "$(pkg-config --modversion gtk4)" \
    --arg adwaita "$(pkg-config --modversion libadwaita-1)" \
    --arg premium "$(git -C ../heelonvault-premium rev-parse HEAD)" \
    --arg dirty "$(test -z "$(git status --porcelain -- crates scripts)" && echo false || echo true)" \
    '{source_commit:$commit, source_dirty:($dirty == "true"), app_version:$version,
      generated_utc:$generated, platform:"Linux", renderer:"cairo", scale:1,
      gtk:$gtk, libadwaita:$adwaita, premium_commit:$premium, font:"DejaVu Sans 10",
      theme:"light", fixture:"synthetic in-memory SQLite; simulated releases; no real secrets",
      images:[inputs | capture("^(?<sha256>[a-f0-9]+)  (?<path>.+)$")]}' \
    > docs/images/user-guide/manifest.json
echo "Generated bilingual documentation captures. Review every image before committing."
