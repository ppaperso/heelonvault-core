# Dossier de donnees de developpement (Rust)

Langue : FR | [EN](README.md)

Ce dossier est reserve aux donnees locales de developpement Rust.

## Chemins

- Base dev : `data/heelonvault-rust-dev.db`
- Base utilisateur (packagee) : `~/.local/share/heelonvault/heelonvault-rust.db`

## Protection des donnees legacy

Ne pas modifier ni supprimer `/var/lib/heelonvault-shared`.
Ce chemin correspond a d'anciennes donnees Python et doit rester intact.

## Reinitialiser les donnees dev locales

```bash
rm -f data/heelonvault-rust-dev.db
```

La base dev sera recreee au prochain lancement `./scripts/run-dev.sh`.
