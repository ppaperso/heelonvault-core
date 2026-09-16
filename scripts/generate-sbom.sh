#!/usr/bin/env bash
# scripts/generate-sbom.sh
# Génère le SBOM CycloneDX 1.4 JSON du binaire heelonvault réellement livré
# (toutes plateformes cibles) à la racine du projet.
#
# Prérequis :
#   - cargo install cargo-cyclonedx --version 0.5.9 --locked
#   - heelonvault-premium checké out en sibling (../heelonvault-premium) : le
#     binaire livré assemble core + app + premium (voir CLAUDE.md, modèle
#     Open Core) — un SBOM généré sans premium omettrait silencieusement
#     toutes ses dépendances propres (reqwest, hyper-rustls, ...) et ne
#     refléterait pas ce qui est réellement livré.
# Usage : ./scripts/generate-sbom.sh
#
# À exécuter localement après tout ajout ou mise à jour de dépendance,
# puis committer sbom.cyclonedx.json avant de pousser.
# Le CI (job check-sbom dans supply-chain.yml) échoue si le fichier commité
# est obsolète.

set -euo pipefail

SBOM_TOOL_VERSION="0.5.9"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

if [[ ! -d "../heelonvault-premium" ]]; then
    echo "[SBOM] [ERROR] ../heelonvault-premium not found — check it out as a sibling directory first."
    echo "[SBOM] [ERROR] A SBOM generated without it would omit every premium-only dependency."
    exit 1
fi

CURRENT_TOOL_VERSION=""
if command -v cargo-cyclonedx &>/dev/null; then
    CURRENT_TOOL_VERSION="$(cargo cyclonedx --version 2>/dev/null | awk '{print $2}')"
fi

if [[ "$CURRENT_TOOL_VERSION" != "$SBOM_TOOL_VERSION" ]]; then
    echo "[SBOM] syncing cargo-cyclonedx to ${SBOM_TOOL_VERSION}..."
    cargo install cargo-cyclonedx --version "$SBOM_TOOL_VERSION" --locked --force
fi

# --describe binaries : un seul SBOM pour le binaire réellement livré
#   (heelonvault-core et sqlx-shim sont des libs sans [[bin]], ignorées —
#   sinon cargo-cyclonedx émet 3 fichiers, un par membre du workspace).
# --target all : capture aussi les dépendances propres à une seule
#   plateforme (ex: embed-resource/vswhom/winreg, Windows uniquement) —
#   sans ça un SBOM généré sur Linux/macOS omettrait silencieusement les
#   dépendances Windows (et vice-versa).
# Incompatible avec --override-filename : on déplace le fichier ensuite.
GENERATED="crates/heelonvault-app/heelonvault_bin.cdx.json"
rm -f "$GENERATED"

echo "[SBOM] Generating sbom.cyclonedx.json (binary target, all platforms)..."
cargo cyclonedx \
    --format json \
    --spec-version 1.4 \
    --describe binaries \
    --target all

if [[ ! -f "$GENERATED" ]]; then
    echo "[SBOM] [ERROR] Expected output not found at $GENERATED"
    exit 1
fi

mv "$GENERATED" sbom.cyclonedx.json

# Normalise les file:// absolus (path+file:// des path-dependencies du
# workspace, download_url= des purl) vers un préfixe fixe : ils encodent le
# répertoire de checkout de la machine qui a généré le fichier
# (/home/you/heelonvault-core en local, /home/runner/work/... en CI), ce qui
# rendrait le SBOM commité différent à chaque environnement même à
# dépendances strictement identiques — cassant le check-sbom de
# supply-chain.yml de façon permanente. Seuls heelonvault-premium (patché,
# donc chemin canonicalisé) et heelonvault-core/crates/* sont concernés :
# les autres path-dependencies (sqlx-shim, heelonvault-app) restent déjà
# relatives telles que déclarées dans Cargo.toml.
NORMALIZE_JQ='
walk(
  if type == "string" then
    gsub("file:///[^\"#]*/(?<tail>heelonvault-core/crates/[a-zA-Z0-9_-]+|heelonvault-premium)(?<frag>#[^\"]*)?";
         "file:///NORMALIZED/\(.tail)\(.frag // "")")
  else . end
)
'
jq "$NORMALIZE_JQ" sbom.cyclonedx.json > sbom.cyclonedx.json.tmp
mv sbom.cyclonedx.json.tmp sbom.cyclonedx.json

COMPONENT_COUNT=$(jq '.components | length' sbom.cyclonedx.json)
echo "[SBOM] Done — ${COMPONENT_COUNT} components inventoried."
echo "[SBOM] Next step: commit sbom.cyclonedx.json if dependencies changed."
