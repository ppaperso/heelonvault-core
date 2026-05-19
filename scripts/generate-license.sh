#!/usr/bin/env bash
# Generate a signed HeelonVault Professional license (.hvl) for a specific customer.
#
# Requirements:
# - bash
# - openssl (with Ed25519 support)
# - jq
# - xxd
# - uuidgen (optional; fallback included)

set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/generate-license.sh \
    --customer "Client X" \
    --expires "2027-12-31T23:59:59Z" \
    --private-key /secure/path/license-ed25519-private.pem \
    [--slots 5] \
    [--feature audit_log --feature certified_exports] \
    [--output ./license-client-x.hvl]

Options:
  --customer      Customer / organization name in the license payload (required)
  --expires       RFC3339 UTC expiration, ex: 2027-12-31T23:59:59Z (required)
  --private-key   Ed25519 private key in PEM format (required)
  --slots         Number of allowed slots/machines (default: 1)
  --feature       Feature flag; can be repeated (default: audit_log)
  --output        Output .hvl file path (default: ./license.hvl)
  --help          Show this help

Output format:
  JSON envelope with fields:
    - payload   (compact JSON string)
    - signature (hex Ed25519 signature)

Install paths on Linux:
  - Dev run:  ~/.config/heelonvault/license.hvl
  - Prod run: /etc/heelonvault/license.hvl
EOF
}

fail() {
  echo "[ERROR] $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

make_uuid() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen
  else
    # Fallback UUID-like value when uuidgen is unavailable.
    openssl rand -hex 16 | sed -E 's/(.{8})(.{4})(.{4})(.{4})(.{12})/\1-\2-\3-\4-\5/'
  fi
}

CUSTOMER=""
EXPIRES=""
PRIVATE_KEY=""
SLOTS="1"
OUTPUT="./license.hvl"
FEATURES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --customer)
      [[ $# -ge 2 ]] || fail "--customer requires a value"
      CUSTOMER="$2"
      shift 2
      ;;
    --expires)
      [[ $# -ge 2 ]] || fail "--expires requires a value"
      EXPIRES="$2"
      shift 2
      ;;
    --private-key)
      [[ $# -ge 2 ]] || fail "--private-key requires a value"
      PRIVATE_KEY="$2"
      shift 2
      ;;
    --slots)
      [[ $# -ge 2 ]] || fail "--slots requires a value"
      SLOTS="$2"
      shift 2
      ;;
    --feature)
      [[ $# -ge 2 ]] || fail "--feature requires a value"
      FEATURES+=("$2")
      shift 2
      ;;
    --output)
      [[ $# -ge 2 ]] || fail "--output requires a value"
      OUTPUT="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

require_cmd openssl
require_cmd jq
require_cmd xxd

[[ -n "$CUSTOMER" ]] || fail "--customer is required"
[[ -n "$EXPIRES" ]] || fail "--expires is required"
[[ -n "$PRIVATE_KEY" ]] || fail "--private-key is required"
[[ -f "$PRIVATE_KEY" ]] || fail "Private key file not found: $PRIVATE_KEY"
[[ "$SLOTS" =~ ^[0-9]+$ ]] || fail "--slots must be a positive integer"

# Basic RFC3339 UTC format validation: YYYY-MM-DDTHH:MM:SSZ
[[ "$EXPIRES" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] \
  || fail "--expires must match RFC3339 UTC format, e.g. 2027-12-31T23:59:59Z"

if [[ ${#FEATURES[@]} -eq 0 ]]; then
  FEATURES+=("audit_log")
fi

LICENSE_ID="$(make_uuid)"
FEATURES_JSON="$(printf '%s\n' "${FEATURES[@]}" | jq -R . | jq -s .)"

# Keep payload compact and deterministic before signing.
PAYLOAD_JSON="$(jq -cn \
  --arg id "$LICENSE_ID" \
  --arg customer_name "$CUSTOMER" \
  --arg expiration_date "$EXPIRES" \
  --argjson slots_count "$SLOTS" \
  --argjson features "$FEATURES_JSON" \
  '{
    id: $id,
    customer_name: $customer_name,
    slots_count: $slots_count,
    expiration_date: $expiration_date,
    features: $features,
    tier: "professional"
  }')"

# Sign raw payload bytes using Ed25519 private key.
# Expected by app verifier: hex signature (64 bytes => 128 hex chars).
TMP_PAYLOAD_FILE="$(mktemp)"
trap 'rm -f "$TMP_PAYLOAD_FILE"' EXIT
printf '%s' "$PAYLOAD_JSON" > "$TMP_PAYLOAD_FILE"

SIGNATURE_HEX="$(
  openssl pkeyutl -sign -rawin -inkey "$PRIVATE_KEY" -in "$TMP_PAYLOAD_FILE" \
    | xxd -p -c 9999
)"

mkdir -p "$(dirname "$OUTPUT")"

jq -cn \
  --arg payload "$PAYLOAD_JSON" \
  --arg signature "$SIGNATURE_HEX" \
  '{payload: $payload, signature: $signature}' > "$OUTPUT"

echo "[OK] License generated: $OUTPUT"
echo "[INFO] Customer: $CUSTOMER"
echo "[INFO] Slots: $SLOTS"
echo "[INFO] Expires: $EXPIRES"

echo "[NEXT] Install on Linux:"
echo "  Dev:  mkdir -p ~/.config/heelonvault && cp \"$OUTPUT\" ~/.config/heelonvault/license.hvl"
echo "  Prod: sudo mkdir -p /etc/heelonvault && sudo cp \"$OUTPUT\" /etc/heelonvault/license.hvl"
