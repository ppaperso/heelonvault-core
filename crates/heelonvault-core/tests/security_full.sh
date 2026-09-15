#!/bin/bash
# security_full.sh - Execute all security tests for HeelonVault
# This script runs all security-related tests to verify database protection
# against hacking attacks (vol de DB, brute-force, etc.)

set -e

echo "=========================================="
echo "HeelonVault Security Tests Suite"
echo "=========================================="
echo ""

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track test results
PASSED=0
FAILED=0
TOTAL=0

# Function to run a test and track results
run_test() {
    local test_name=$1
    local test_path=$2
    
    TOTAL=$((TOTAL + 1))
    echo -n "Running ${test_path}... "
    
    if cargo test -p heelonvault-core --test ${test_path} -- --nocapture 2>&1; then
        echo -e "${GREEN}[PASS]${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}[FAIL]${NC}"
        FAILED=$((FAILED + 1))
    fi
    echo ""
}

# Function to run unit tests in a file (non-integration)
run_unit_tests() {
    local test_name=$1
    echo -n "Running ${test_name}... "
    
    TOTAL=$((TOTAL + 1))
    if cargo test -p heelonvault-core --lib ${test_name} -- --nocapture 2>&1; then
        echo -e "${GREEN}[PASS]${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}[FAIL]${NC}"
        FAILED=$((FAILED + 1))
    fi
    echo ""
}

echo "Starting security tests..."
echo ""

# ============================================================================
# Catégorie P1: Critique - Tests de chiffrement des données au repos
# ============================================================================

echo -e "${YELLOW}=== P1: CRITICAL - Encryption at Rest Tests ===${NC}"
echo ""

run_test "db_encryption_at_rest" "db_encryption_at_rest"

echo ""

# ============================================================================
# Catégorie P1: Critique - Tests de la chaîne de déchiffrement complète
# ============================================================================

echo -e "${YELLOW}=== P1: CRITICAL - Full Decryption Chain Tests ===${NC}"
echo ""

run_test "full_decryption_chain" "full_decryption_chain"

echo ""

# ============================================================================
# Catégorie P1: Critique - Tests de protection contre la copie de DB
# ============================================================================

echo -e "${YELLOW}=== P1: CRITICAL - DB Copy Protection Tests ===${NC}"
echo ""

run_test "db_copy_protection" "db_copy_protection"

echo ""

# ============================================================================
# Catégorie P2: Haut - Tests de résistance au brute-force
# ============================================================================

echo -e "${YELLOW}=== P2: HIGH - Brute-Force Resistance Tests ===${NC}"
echo ""

run_test "bruteforce_resistance" "bruteforce_resistance"

echo ""

# ============================================================================
# Catégorie P2: Haut - Tests d'intégrité des données
# ============================================================================

echo -e "${YELLOW}=== P2: HIGH - Data Integrity Tests ===${NC}"
echo ""

run_test "data_integrity" "data_integrity"

echo ""

# ============================================================================
# Catégorie P2: Haut - Tests de validation des entrées
# ============================================================================

echo -e "${YELLOW}=== P2: HIGH - Input Validation Tests ===${NC}"
echo ""

run_test "input_validation" "input_validation"

echo ""

# ============================================================================
# Catégorie P3: Moyen - Tests de sécurité des backups
# ============================================================================

echo -e "${YELLOW}=== P3: MEDIUM - Backup Security Tests ===${NC}"
echo ""

run_test "backup_security" "backup_security"

echo ""

# ============================================================================
# Catégorie P3: Moyen - Tests de sécurité mémoire
# ============================================================================

echo -e "${YELLOW}=== P3: MEDIUM - Memory Safety Tests ===${NC}"
echo ""

run_test "memory_safety" "memory_safety"

echo ""

# ============================================================================
# Catégorie P1: Critique - Confidentialité, RGPD, second facteur, permissions
# ============================================================================

echo -e "${YELLOW}=== P1: CRITICAL - Privacy, GDPR, TOTP, Access Control ===${NC}"
echo ""

run_test "privacy_no_secret_in_logs" "privacy_no_secret_in_logs"
run_test "privacy_local_only" "privacy_local_only"
run_test "gdpr_erasure_and_portability" "gdpr_erasure_and_portability"
run_test "totp_security" "totp_security"
run_test "access_control_matrix" "access_control_matrix"
run_test "robustness_properties" "robustness_properties"
run_test "account_rekey_integration" "account_rekey_integration"

echo ""

# ============================================================================
# Tests existants de sécurité
# ============================================================================

echo -e "${YELLOW}=== EXISTING - Existing Security Tests ===${NC}"
echo ""

run_test "security_crypto" "security_crypto"
run_test "security_auth" "security_auth"
run_test "backup_security_integration" "backup_security_integration"

echo ""

# ============================================================================
# Résumé
# ============================================================================

echo "=========================================="
echo "Security Tests Summary"
echo "=========================================="
echo "Total tests:  $TOTAL"
echo -e "${GREEN}Passed:      $PASSED${NC}"
echo -e "${RED}Failed:      $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All security tests passed!${NC}"
    echo ""
    echo "Database protection against hacking attacks is verified:"
    echo "  ✓ All data is encrypted at rest"
    echo "  ✓ Full decryption chain requires all components"
    echo "  ✓ DB copy protection is in place"
    echo "  ✓ Brute-force resistance parameters are sufficient"
    echo "  ✓ Data integrity is protected (AES-GCM authentication)"
    echo "  ✓ Input validation prevents malformed data"
    echo "  ✓ Backup security is enforced"
    echo "  ✓ Secrets never leak in Debug output, error messages, logs or reject reports"
    echo "  ✓ Core stays local-only (no network client, no telemetry)"
    echo "  ✓ Erasure removes personal data from disk; exports are owner-only"
    echo "  ✓ TOTP codes cannot be replayed; permission matrix is locked"
    exit 0
else
    echo -e "${RED}Some security tests failed!${NC}"
    echo ""
    echo "Please investigate the failures above."
    echo "Database protection may be compromised!"
    exit 1
fi
