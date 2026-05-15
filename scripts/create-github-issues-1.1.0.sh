#!/usr/bin/env bash
# =============================================================================
# HeelonVault — Création des issues GitHub pour le milestone 1.1.0
# Prérequis : gh auth login (token avec scope repo)
# Usage     : bash scripts/create-github-issues-1.1.0.sh
# =============================================================================
set -euo pipefail

REPO="ppaperso/HeelonVault"

echo "==> Vérification authentification GitHub CLI"
gh auth status

# ---------------------------------------------------------------------------
# 1. Labels
# ---------------------------------------------------------------------------
echo ""
echo "==> Création des labels (ignore si déjà existants)"

gh label create "tech-debt"       --color "E4B429" --description "Dette technique identifiée" --repo "$REPO" 2>/dev/null || true
gh label create "security"        --color "DC3545" --description "Sécurité / comportement indéfini" --repo "$REPO" 2>/dev/null || true
gh label create "refactor"        --color "0075CA" --description "Refactoring et lisibilité" --repo "$REPO" 2>/dev/null || true
gh label create "testing"         --color "CFD3D7" --description "Couverture de tests" --repo "$REPO" 2>/dev/null || true
gh label create "documentation"   --color "0052CC" --description "Documentation API" --repo "$REPO" 2>/dev/null || true
gh label create "epic"            --color "6F42C1" --description "Epic / tracking parent" --repo "$REPO" 2>/dev/null || true
gh label create "priority:critical" --color "B60205" --description "Priorité critique" --repo "$REPO" 2>/dev/null || true
gh label create "priority:high"   --color "E99695" --description "Priorité haute" --repo "$REPO" 2>/dev/null || true
gh label create "priority:low"    --color "C5DEF5" --description "Priorité basse / optionnel" --repo "$REPO" 2>/dev/null || true

# ---------------------------------------------------------------------------
# 2. Milestone 1.1.0
# ---------------------------------------------------------------------------
echo ""
echo "==> Création du milestone 1.1.0"
gh api repos/"$REPO"/milestones \
  --method POST \
  --field title="1.1.0" \
  --field description="Résorption de la dette technique identifiée en v1.0.4 (analyse copilot 14/04/2026)" \
  --field state="open" \
  2>/dev/null || echo "    (milestone déjà existant — ignoré)"

MILESTONE_NUMBER=$(gh api repos/"$REPO"/milestones --jq '.[] | select(.title=="1.1.0") | .number')
echo "    milestone number = $MILESTONE_NUMBER"

# ---------------------------------------------------------------------------
# 3. Issue #4 — EPIC
# ---------------------------------------------------------------------------
echo ""
echo "==> Création de l'issue EPIC"
EPIC_URL=$(gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] chore(tech-debt): résorber la dette technique v1.0.4 (7 points)" \
  --label "epic,tech-debt" \
  --milestone "1.1.0" \
  --body "## Périmètre

Tracking épic pour les **7 points de dette technique** détectés lors de l'analyse statique du 14/04/2026 sur \`v1.0.4\`.

| # | Ticket | Impact | Est. |
|---|--------|--------|------|
| 1 | #5 — \`env::set_var\` après runtime multi-thread | ⚠️ Critique | 30 min |
| 2 | #6 — Bypass Clippy global dans \`import_service.rs\` | ⚠️ Critique | 1 h |
| 3 | #7 — Décomposer \`main()\` (362 lignes) | 🔧 Important | 2 h |
| 4 | #8 — Factoriser la construction des services | 🔧 Important | 1 h |
| 5 | #9 — Décomposer \`share_vault_with_team\` / \`rotate_vault_key\` | 🔧 Important | 3 h |
| 6 | #10 — Documentation \`///\` manquante sur les traits publics | ✨ Optionnel | 2 h |
| 7 | #11 — Absence de tests unitaires dans \`repositories/\` | ✨ Optionnel | 4 h |

## Définition of Done
- [ ] Tous les tickets enfants fermés
- [ ] \`cargo clippy -- -D warnings\` passe sans avertissement
- [ ] \`cargo test --workspace\` vert
- [ ] CHANGELOG mis à jour pour 1.1.0
")
echo "    EPIC créée : $EPIC_URL"

# ---------------------------------------------------------------------------
# 4. Tickets enfants
# ---------------------------------------------------------------------------

echo ""
echo "==> Issue #5 — env::set_var (CRITIQUE)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] fix(safety): déplacer env::set_var avant la construction du runtime Tokio" \
  --label "security,priority:critical,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

\`env::set_var(\"GSK_RENDERER\", \"gl\")\` est appelé à \`src/main.rs:248\`, **après** la construction du runtime Tokio multi-thread (\`Builder::new_multi_thread()\`).

Depuis Rust 1.81, \`env::set_var\` en contexte multi-thread est marqué \`deprecated\` et peut produire un comportement indéfini (data race avec \`getenv\` / \`localtime_r\` dans d'autres threads).

Référence analyse : \`src/main.rs:248\`
Impact : comportement indéfini (UB) sur Linux, avertissement de compilation Rust ≥ 1.81

## Solution

Appeler \`env::set_var\` **avant** \`Builder::new_multi_thread().build()\`.

\`\`\`rust
// Avant (src/main.rs ~ligne 246)
let runtime = Arc::new(runtime); // runtime déjà démarré
// ...
env::set_var(\"GSK_RENDERER\", \"gl\");  // ← UB

// Après
env::set_var(\"GSK_RENDERER\", \"gl\");  // ← avant tout thread
let runtime = Builder::new_multi_thread().enable_all().build()?;
let runtime = Arc::new(runtime);
\`\`\`

## Checklist
- [ ] Déplacer \`env::set_var\` avant la construction du runtime
- [ ] Vérifier qu'aucun autre \`set_var\` n'est appelé après le démarrage du runtime en dehors des tests
- [ ] \`cargo clippy -- -D warnings\` passe
- [ ] \`cargo test\` passe
- [ ] Tester le rendu GTK (pas de régression visuelle)

**Estimation : 30 min**
Closes #4 (partiel)"

echo ""
echo "==> Issue #6 — Bypass Clippy import_service.rs (CRITIQUE)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] fix(clippy): supprimer le bypass global disallowed_methods dans import_service.rs" \
  --label "security,priority:critical,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

\`src/services/import_service.rs:1\` contient \`#![allow(clippy::disallowed_methods)]\` au niveau **module entier** (hors \`#[cfg(test)]\`).

Cette directive annule la politique Clippy définie dans \`clippy.toml\` (interdiction de \`unwrap\`/\`expect\`) pour tout le fichier, y compris le code de production. Résultat : des \`unwrap_or_default()\` sur des index CSV (lignes 78–90) ne sont pas détectés, et une colonne manquante retourne silencieusement \`\"\"\` au lieu d'une erreur explicite.

Référence analyse : \`src/services/import_service.rs:1\`, lignes 78–90
Impact : import silencieux de données corrompues (champs vides non signalés)

## Solution

1. Supprimer la directive \`#![allow]\` de niveau module.
2. Remplacer les accès \`record.get(idx).unwrap_or_default()\` par des accès vérifiés retournant \`AppError::Validation\` si l'index est absent.
3. Si des \`unwrap\` **de test** sont nécessaires, les déplacer dans un bloc \`#[cfg(test)]\` avec \`#[allow(clippy::disallowed_methods)]\` scoped.

\`\`\`rust
// Avant (import_service.rs:78)
let name = record.get(name_idx).unwrap_or_default().trim().to_string();

// Après
let name = record
    .get(name_idx)
    .ok_or_else(|| AppError::Validation(format!(\"colonne 'name' manquante (index {name_idx})\")))?
    .trim()
    .to_string();
\`\`\`

## Checklist
- [ ] Supprimer \`#![allow(clippy::disallowed_methods)]\` ligne 1
- [ ] Remplacer chaque \`unwrap_or_default\` sur index CSV par \`ok_or_else(|| AppError::Validation(...))\`
- [ ] \`cargo clippy -- -D warnings\` passe sur \`import_service.rs\`
- [ ] Tests d'import existants passent
- [ ] Ajouter un test : import CSV avec colonne manquante → erreur explicite

**Estimation : 1 h**
Closes #4 (partiel)"

echo ""
echo "==> Issue #7 — Décomposer main() (IMPORTANT)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] refactor(main): décomposer main() de 362 lignes en sous-fonctions ciblées" \
  --label "refactor,priority:high,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

\`src/main.rs:209–570\` : la fonction \`main()\` fait **362 lignes** et gère à la fois :
- Parsing des arguments CLI (\`--version\`, \`--startup-check\`)
- Lancement du runtime Tokio
- Construction du contexte applicatif
- Restauration de backup (staging, promote, restart)
- Initialisation et affichage de la fenêtre GTK
- Gestion des callbacks de login/bootstrap

Complexité cyclomatique estimée > 25. Toute modification risque des régressions.

Référence analyse : \`src/main.rs:209–570\`
Impact : maintenabilité, risque de régression lors de toute modification

## Solution

Extraire trois fonctions distinctes :

\`\`\`rust
// Avant
fn main() -> Result<()> { /* 362 lignes */ }

// Après
fn main() -> Result<()> {
    handle_cli_flags()?;                        // --version, --startup-check
    let ctx = Arc::new(runtime.block_on(initialize_app_context())?);
    run_gui_application(ctx)                    // GTK + callbacks
}

fn handle_cli_flags() -> Result<bool> { ... }  // ~20 lignes
fn run_gui_application(ctx: Arc<AppContext>) -> Result<()> { ... }  // GTK init + connect_*
\`\`\`

## Checklist
- [ ] Extraire \`handle_cli_flags()\`
- [ ] Extraire \`run_gui_application()\` (GTK + callbacks login/backup/restore)
- [ ] \`main()\` ≤ 30 lignes après refactoring
- [ ] \`cargo clippy -- -D warnings\` passe
- [ ] \`cargo test --workspace\` passe
- [ ] Smoke test manuel : \`--version\`, \`--startup-check\`, lancement GUI

**Estimation : 2 h**
Closes #4 (partiel)"

echo ""
echo "==> Issue #8 — Factoriser construction services (IMPORTANT)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] refactor(init): factoriser les 7 instanciations répétées dans initialize_app_context()" \
  --label "refactor,priority:high,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

\`src/main.rs:712–870\` (\`initialize_app_context\`) instancie manuellement :
- **7×** \`SqlxUserRepository::new(pool.clone())\`
- **7×** \`CryptoServiceImpl::default()\`

Chaque service reçoit sa propre copie alors qu'un seul \`Arc\` partagé suffirait ou qu'une closure factory éliminerait la répétition.

Référence analyse : \`src/main.rs:760–840\`
Impact : lisibilité, risque d'oubli de mise à jour lors d'ajout de service

## Solution

\`\`\`rust
// Avant
AdminServiceImpl::new(SqlxUserRepository::new(pool.clone()), Arc::clone(&auth_service), ...)
TeamServiceImpl::new(SqlxTeamRepository::new(pool.clone()), SqlxUserRepository::new(pool.clone()), ...)

// Après
let user_repo = || SqlxUserRepository::new(pool.clone());
let crypto    = || CryptoServiceImpl::default();

AdminServiceImpl::new(user_repo(), Arc::clone(&auth_service), ...)
TeamServiceImpl::new(SqlxTeamRepository::new(pool.clone()), user_repo(), ...)
\`\`\`

## Checklist
- [ ] Introduire \`let user_repo = || SqlxUserRepository::new(pool.clone());\`
- [ ] Introduire \`let crypto = || CryptoServiceImpl::default();\`
- [ ] Remplacer toutes les instanciations répétées
- [ ] \`cargo clippy -- -D warnings\` passe
- [ ] \`cargo test --workspace\` passe

**Estimation : 1 h**
Closes #4 (partiel)"

echo ""
echo "==> Issue #9 — Décomposer share_vault_with_team / rotate_vault_key (IMPORTANT)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] refactor(team-service): décomposer share_vault_with_team() et rotate_vault_key() (130+ lignes)" \
  --label "refactor,priority:high,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

Deux fonctions dans \`src/services/team_service.rs\` ont une complexité excessive :
- \`share_vault_with_team()\` : **135 lignes** (lignes 469–603) — mélange RBAC, crypto, persistance DB et audit
- \`rotate_vault_key()\` : **147 lignes** (lignes 604–750) — même problème

Ces fonctions sont des points de défaillance silencieuse au moindre changement de logique métier.

Référence analyse : \`src/services/team_service.rs:469–603\`, \`604–750\`
Impact : maintenabilité, risque de bug lors d'évolution RBAC ou crypto

## Solution

Extraire des fonctions privées par responsabilité :

\`\`\`rust
// share_vault_with_team → 3 sous-fonctions
async fn validate_share_prerequisites(&self, ...) -> Result<(), AppError>
async fn encrypt_key_for_members(&self, ...) -> Result<Vec<KeyShare>, AppError>
async fn persist_key_shares(&self, ...) -> Result<(), AppError>

// rotate_vault_key → 3 sous-fonctions
async fn validate_rotation_permissions(&self, ...) -> Result<(), AppError>
async fn generate_and_distribute_new_key(&self, ...) -> Result<KeyRotationResult, AppError>
async fn audit_key_rotation(&self, ...) -> Result<(), AppError>
\`\`\`

## Checklist
- [ ] Extraire les sous-fonctions de \`share_vault_with_team\`
- [ ] Extraire les sous-fonctions de \`rotate_vault_key\`
- [ ] Chaque fonction résultante ≤ 40 lignes
- [ ] Tests existants (\`team_service.rs:1152–1202\`) passent sans modification
- [ ] \`cargo clippy -- -D warnings\` passe

**Estimation : 3 h**
Closes #4 (partiel)"

echo ""
echo "==> Issue #10 — Documentation API manquante (OPTIONNEL)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] docs(api): ajouter la documentation /// sur les traits de service et repository publics" \
  --label "documentation,priority:low,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

Les traits et méthodes publics des couches service et repository ne sont pas documentés :
- \`src/services/vault_service.rs\` : **0 commentaire \`///\`** (986 lignes)
- \`src/services/secret_service.rs\` : 3 lignes de doc (1084 lignes)
- \`src/repositories/\` total : 12 lignes pour ~3 000 lignes de code

Les invariants de sécurité (ex. : quand \`master_key\` doit être zéroïsé par l'appelant) sont implicites.

Référence analyse : \`src/services/vault_service.rs\`, \`src/repositories/*.rs\`
Impact : onboarding, maintenabilité, risque de mauvaise utilisation des API sensibles

## Solution

Documenter a minima :
1. Chaque méthode de trait \`VaultService\`, \`SecretService\`, \`TeamService\`
2. Les invariants de sécurité : durée de vie des clés, responsabilité de zéroïsation
3. Les méthodes \`pub\` des repositories (pré/post-conditions SQL)

\`\`\`rust
// Avant
async fn open_vault(&self, vault_id: Uuid, master_key: SecretBox<Vec<u8>>) -> Result<SecretBox<Vec<u8>>, AppError>;

// Après
/// Ouvre un coffre et retourne la clé de coffre déchiffrée.
///
/// `master_key` est la clé racine de l'utilisateur ; elle n'est pas stockée.
/// Le résultat doit être zéroïsé par l'appelant après usage.
///
/// # Erreurs
/// - [`AppError::NotFound`] si `vault_id` est inconnu.
/// - [`AppError::Crypto`] si le déchiffrement échoue.
async fn open_vault(&self, vault_id: Uuid, master_key: SecretBox<Vec<u8>>) -> Result<SecretBox<Vec<u8>>, AppError>;
\`\`\`

## Checklist
- [ ] Documenter toutes les méthodes de \`VaultService\`
- [ ] Documenter toutes les méthodes de \`SecretService\`
- [ ] Documenter toutes les méthodes de \`TeamService\`
- [ ] Documenter les méthodes \`pub\` des 5 repositories
- [ ] \`cargo doc --no-deps\` compile sans avertissement

**Estimation : 2 h**
Closes #4 (partiel)"

echo ""
echo "==> Issue #11 — Tests unitaires repositories manquants (OPTIONNEL)"
gh issue create \
  --repo "$REPO" \
  --title "[1.1.0] test(repositories): ajouter les tests unitaires pour vault_repository, team_repository et audit_log_repository" \
  --label "testing,priority:low,tech-debt" \
  --milestone "1.1.0" \
  --body "## Problème

Aucun test unitaire dans \`src/repositories/\`. Les cinq fichiers (3 500 lignes cumulées) ne sont couverts que par quelques tests d'intégration dans \`tests/\` qui ne testent pas les chemins d'erreur SQL.

Fichiers non couverts :
- \`src/repositories/vault_repository.rs\` (692 lignes)
- \`src/repositories/team_repository.rs\` (505 lignes)
- \`src/repositories/audit_log_repository.rs\` (non listé explicitement mais présent)

Référence analyse : \`src/repositories/*.rs\`
Impact : régressions silencieuses lors des migrations SQL, pas de test des chemins d'erreur

## Solution

Ajouter des modules \`#[cfg(test)]\` avec une base SQLite en mémoire (\`SqlitePool::connect(\":memory:\")\`) :

\`\`\`rust
#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect(\":memory:\").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn vault_not_found_returns_error() {
        let pool = setup_pool().await;
        let repo = SqlxVaultRepository::new(pool);
        let result = repo.get_vault(Uuid::new_v4()).await;
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }
}
\`\`\`

## Checklist
- [ ] Tests unitaires pour \`vault_repository\` (create, get, not_found, list)
- [ ] Tests unitaires pour \`team_repository\` (create, add_member, conflict)
- [ ] Tests unitaires pour \`audit_log_repository\` (append, list, pagination)
- [ ] Couverture des chemins d'erreur SQL (violation de contrainte, not found)
- [ ] \`cargo test --workspace\` passe

**Estimation : 4 h**
Closes #4 (partiel)"

# ---------------------------------------------------------------------------
# 5. Rattacher les tickets à l'EPIC via commentaire de liaison
# ---------------------------------------------------------------------------
echo ""
echo "==> Ajout commentaire de liaison sur l'EPIC"
EPIC_NUMBER=$(gh issue list --repo "$REPO" --search "[1.1.0] chore(tech-debt)" --limit 1 --json number --jq '.[0].number')
echo "    EPIC numéro = $EPIC_NUMBER"

echo ""
echo "============================================================"
echo "  DONE — 8 issues créées (1 epic + 7 tickets enfants)"
echo "  Consulter : https://github.com/$REPO/issues?milestone=1.1.0"
echo "============================================================"
