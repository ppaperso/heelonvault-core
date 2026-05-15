# Licences tierces

Langue : FR | [EN](THIRD_PARTY_LICENSES.md)

HeelonVault incorpore ou lie des composants logiciels tiers.
Ce document fournit un guide FR de lecture et de conformite.

## Portee

- Les noms de composants, versions, SPDX et textes de licence officiels restent en anglais pour precision juridique.
- Le document de reference complet (inventaire exhaustif des crates et licences) est : [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## 1. Bibliotheques systeme (liaison dynamique)

Ces bibliotheques ne sont pas embarquees statiquement dans le binaire HeelonVault.
Elles sont chargees dynamiquement par l'OS.

Exemples : GTK4, libadwaita, GLib, Pango, Cairo, SQLite.

Conformite LGPL : l'utilisateur peut remplacer ces bibliotheques par des versions compatibles selon les termes LGPL applicables.

## 2. Crates Rust (compilation statique)

Les crates compilees dans le binaire sont listees dans le document EN avec leurs versions et licences.
Les licences utilisees sont permissives (MIT, Apache-2.0, BSD, ISC, Unicode, equivalents), a l'exception des licences des bibliotheques systeme dynamiques gerees separement.

## 3. Sources et verification

Pour verifier une licence :

1. Consulter la ligne correspondante dans [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
2. Verifier `Cargo.lock` pour la version resolue.
3. Verifier le texte de licence sur crates.io ou dans le depot source du composant.

## 4. Bonnes pratiques de conformite

- Ne pas modifier manuellement les identifiants SPDX.
- Conserver les noms de paquets exacts.
- Mettre a jour ce guide FR si la politique de licence change.
- Regenerer l'inventaire EN lors des mises a jour majeures de dependances.

## 5. Limite de traduction

Par etat de l'art compliance, les clauses juridiques officielles et libelles SPDX font foi en langue d'origine.
La presente version FR est un guide de lecture et ne remplace pas les textes legaux originaux.
