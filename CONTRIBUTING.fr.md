# Guide de contribution (Rust)

Langue : FR | [EN](CONTRIBUTING.md)

Merci de contribuer a HeelonVault.

## Perimetre

- Le code principal est a la racine du depot.
- Le code Python historique a ete retire.
- Les contributions doivent rester Rust-first et security-first.

## Environnement de developpement

Prerequis :

- Linux
- Toolchain Rust (`cargo`, `rustc`)
- Paquets runtime GTK4/libadwaita pour votre distribution

Installation locale :

```bash
git clone <repo-url>
cd HeelonVault
cargo check
```

Lancement en mode developpement :

```bash
./scripts/run-dev.sh
```

Chemins de base :

- Dev : `data/heelonvault-rust-dev.db`
- Prod packagee : `~/.local/share/heelonvault/heelonvault-rust.db`
- Legacy a ne pas toucher : `/var/lib/heelonvault-shared`

## Standards de code

- Respecter le style et les conventions existantes.
- Privilegier les commits petits et focalises.
- Ajouter des tests pour les changements repository/service.
- Ne jamais commiter de secrets ou donnees privees.

## Commandes de test

Depuis la racine :

```bash
cargo check
cargo test
cargo test secret_repository:: -- --nocapture
cargo test secret_service:: -- --nocapture
```

## Checklist Pull Request

- `cargo check` passe.
- Les tests pertinents passent.
- Les changements sensibles cote securite sont justifies dans la PR.
- La documentation est mise a jour si le comportement change.

## Signalements de securite

Ne pas ouvrir d'issue publique pour les vulnerabilites.
Contact : `security@heelonys.fr`
