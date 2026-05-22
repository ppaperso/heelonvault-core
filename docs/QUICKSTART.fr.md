# Demarrage rapide (Rust)

Langue : FR | [EN](QUICKSTART.md)

Version rapide documentee : `1.1.0`

## 1. Verification du build

```bash
cargo check --workspace
```

## 2. Lancement en developpement

Depuis la racine du depot :

```bash
./scripts/run-dev.sh
```

Chemin de la base de developpement :

- `data/heelonvault-rust-dev.db`

## 3. Lancement des tests

```bash
cargo test secret_repository:: -- --nocapture
cargo test secret_service:: -- --nocapture
cargo test --workspace --test login_history_integration
```

## 3 bis. Verifications UI recommandees

1. Ouvrir `Profil & Securite` depuis la barre laterale.
2. Fermer la fenetre principale avec la croix : l'ecran de login doit reapparaitre.
3. Se reconnecter immediatement : les cartes de secrets doivent etre visibles.
4. Activer l'affichage du mot de passe en edition, puis modifier un secret de type mot de passe.

## 4. Build de production

```bash
cargo build --release
```

L'installateur Linux package deploie :

- Binaire : `/opt/heelonvault/heelonvault`
- Lanceur shell : `/opt/heelonvault/run.sh`
- Entree desktop : `/usr/share/applications/com.heelonvault.rust.desktop`
- Entree desktop legacy : `/usr/share/applications/heelonvault.desktop`
- Base utilisateur : `~/.local/share/heelonvault/heelonvault-rust.db`
- Logs utilisateur : `~/.local/state/heelonvault/logs`

Verification post-installation (Ubuntu) :

```bash
test -x /opt/heelonvault/heelonvault
test -x /opt/heelonvault/run.sh
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop
desktop-file-validate /usr/share/applications/com.heelonvault.rust.desktop
gtk-launch com.heelonvault.rust
```

Note de migration legacy :

- D'anciens installateurs pouvaient stocker la base dans `/opt/heelonvault/data/heelonvault-rust-dev.db`. Le lanceur package copie ce fichier vers le dossier utilisateur au premier demarrage si necessaire.
