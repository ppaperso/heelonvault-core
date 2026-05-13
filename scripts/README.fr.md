# Scripts

Langue : FR | [EN](README.md)

Ce dossier contient des scripts shell operationnels utilises par le depot Rust.
Executer les scripts depuis la racine du depot.

## Scripts disponibles

- `scripts/backup-prod-before-tests.sh` : cree une archive de sauvegarde avant tests manuels.
- `scripts/fix-permissions.sh` : corrige les permissions et ACL sur les repertoires de donnees.

## Developpement Rust

Pour le developpement et les tests, utiliser les scripts racine et commandes Rust :

```bash
./scripts/run-dev.sh
cargo check
cargo test
```

Notes chemins de donnees :

- Donnees utilisateurs packagees : `~/.local/share/heelonvault`
- Donnees dev Rust : `data/`
- Donnees legacy Python : `/var/lib/heelonvault-shared` (ne pas modifier)
