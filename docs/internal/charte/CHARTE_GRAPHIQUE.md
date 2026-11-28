# Charte graphique de l'application HeelonVault

Langue : FR | [EN](CHARTE_GRAPHIQUE.en.md)

Cette charte décrit l'aspect de l'application de bureau (direction C du relooking 3.0, #105) :
surfaces blanches, gris neutres, et sarcelle Heelonys réservée aux actions. Elle s'appuie sur
le style libadwaita (Adwaita) au lieu de le remplacer.

La source de vérité est le code : les couleurs sont définies **uniquement** dans
`crates/heelonvault-app/assets/style/00-tokens.css`, et les autres feuilles de
`assets/style/` y font référence par leur nom. En cas d'écart entre ce document et les
tokens, ce sont les tokens qui font foi ; corrigez alors ce document.

Le site web Heelonys a sa propre charte (police Space Grotesk, dégradés, sections sombres).
Elle ne s'applique **pas** à l'application : seule la gamme de sarcelles du logo est commune.

## Principes

1. **Le blanc porte le contenu.** Les cartes, listes et formulaires sont blancs
   (`hv_surface`) sur un fond presque blanc (`hv_canvas`). Les barres de titre sont blanches.
2. **Le texte et la structure sont neutres.** Textes, bordures et séparateurs sont gris,
   sans teinte de marque.
3. **La sarcelle signale une action ou une sélection.** Bouton principal, élément
   sélectionné, focus, interrupteurs actifs, liens. Elle n'est jamais décorative et ne sert
   jamais d'avertissement.
4. **Les couleurs sémantiques ne servent qu'aux états.** Succès, avertissement, danger et
   information, chacun avec un fond clair (`*_bg`) et un texte foncé (`*_strong`).
5. **Aucun dégradé, un seul niveau d'ombre.** Pas de bandeau coloré, pas de liseré de
   couleur sur les cartes, pas d'effet de soulèvement au survol. Seule exception : le sceau
   de certification Heelonys (voir plus bas).
6. **Casse de phrase partout.** Libellés, badges et pastilles s'écrivent « Doublon »,
   « Mot de passe faible », jamais en capitales ni avec un émoji.

## Couleurs

### Marque (accent uniquement)

| Token | Valeur | Usage |
| --- | --- | --- |
| `hv_brand_900` | `#07393A` | État pressé du bouton principal, texte sur fond `brand_100` |
| `hv_brand_800` | `#0A5F5C` | Texte des boutons secondaires et des liens, icônes d'action, survol du bouton principal |
| `hv_brand_700` | `#0E706B` | Bouton principal, pastille sélectionnée, accent libadwaita, icône de type de secret |
| `hv_brand_600` | `#0D8F8A` | Anneau de focus, sélection d'une carte, jauges |
| `hv_brand_200` | `#A4DFCF` | Survol de l'élément sélectionné de la barre latérale |
| `hv_brand_100` | `#D6F1E9` | Fond d'un bouton bascule actif, survol d'une pastille de catégorie |
| `hv_brand_50` | `#EEF8F7` | Fond des pastilles de catégorie, des rôles, des cases cochées et des encarts d'information |

`hv_brand_500` (`#13A1A1`), `hv_brand_green` (`#1F8678`) et `hv_brand_300` (`#57B9B1`)
restent définis pour mémoire de la gamme du logo, mais aucune règle ne les utilise :
leur contraste sur blanc est insuffisant pour du texte.

### Neutres

| Token | Valeur | Usage |
| --- | --- | --- |
| `hv_text` | `#1F2526` | Texte principal (14:1 sur blanc) |
| `hv_text_muted` | `#586466` | Texte secondaire, aides, icônes d'information (5,6:1) |
| `hv_text_subtle` | `#667375` | Texte tertiaire (4,6:1) |
| `hv_surface` | `#FFFFFF` | Cartes, listes, champs, barres de titre |
| `hv_surface_subtle` | `#F3F5F5` | Pastilles neutres, lignes en lecture seule, panneau du générateur |
| `hv_canvas` | `#FAFAFB` | Fond des pages |
| `hv_border` | `#E4E7E8` | Filets des cartes et séparateurs |
| `hv_border_strong` | `#CDD3D5` | Bordure des champs et des boutons secondaires, survol des cartes |

### Sémantiques

| Famille | Base | Texte (`*_strong`) | Fond (`*_bg`) |
| --- | --- | --- | --- |
| Succès | `#178A64` | `#176C53` | `#DBF3E8` |
| Avertissement | `#B26A00` | `#805000` | `#FFF0D3` |
| Danger | `#C01C28` | `#A42625` | `#FFE5E3` |
| Information | `#2F6FDE` | `#1F4FA8` | `#E3EDFF` |

Un badge sémantique associe toujours son fond `*_bg` à son texte `*_strong`. Les variantes
`*_bright` sont réservées aux indicateurs graphiques (jauges, pastilles de couleur), pas au
texte.

### Sceau de certification Heelonys

Les tokens `hv_seal_*` (fond sombre, filet doré) appartiennent au sceau de certification
(écran de connexion et en-tête de la fenêtre principale). C'est la seule surface sombre et la seule
surface en dégradé de l'application, conservée volontairement comme marque ; ne réutilisez
ni ses tokens ni ses dégradés ailleurs.

### Accent libadwaita

`00-tokens.css` redéfinit `accent_bg_color`, `accent_fg_color` et `accent_color` sur la
marque. À partir de libadwaita 1.6, qui suit l'accent du système, `accent-modern.css`
redéfinit les mêmes valeurs en variables CSS : gardez les deux fichiers alignés.

## Typographie

- Police du système (Cantarell sous GNOME, Segoe UI sous Windows) : aucune police n'est
  embarquée.
- Hiérarchie par les classes libadwaita (`title-1` à `title-4`, `heading`, `caption`,
  `dim-label`) plutôt que par des tailles fixes.
- Libellé de champ : 600, `hv_text`, 0,88rem (`field-label`). Aide : `hv_text_muted`,
  0,86rem (`field-hint`). Erreur : `hv_danger`, 600 (`form-error`).
- Les chiffres des compteurs utilisent des chiffres tabulaires (`font-feature-settings: "tnum"`).
- Les données techniques (clés SSH, empreintes) sont en `monospace`.

## Formes et élévation

| Élément | Rayon | Bordure | Ombre |
| --- | --- | --- | --- |
| Carte, liste encadrée, champ, bouton | 12px | `hv_border` (champ : `hv_border_strong`) | `0 1px 3px hv_shadow_soft` (cartes et listes) |
| Ligne de liste compacte | 10px | `hv_border` | idem |
| Pastille, badge, puce | 999px | aucune | aucune |
| Survol d'une carte | inchangé | `hv_border_strong` | `0 2px 8px hv_shadow_key` |

Le survol ne change ni la géométrie ni la position d'un élément : seuls la couleur, la
bordure et l'ombre évoluent, en 150ms.

## Composants

- **Bouton principal** (`suggested-action`, `primary-pill`) : fond `hv_brand_700`, texte
  blanc, 700 ; survol `brand_800`, pressé `brand_900`. Un seul par groupe ou par vue.
- **Bouton secondaire** (`text-button`, `secondary-pill`) : fond blanc, bordure
  `hv_border_strong`, texte `hv_brand_800`.
- **Bouton plat** (`flat`) : transparent, sans bordure ; pour les actions rapides des cartes,
  survol `brand_50`.
- **Action destructive** (`destructive-action`) : fond `hv_danger_strong`, texte blanc ;
  survol `hv_danger`. Jamais sarcelle.
- **Bouton Retour** : toujours le widget `BackButton` (`ui/widgets/back_button.rs`), plat,
  icône `hv-back-symbolic` et libellé en `hv_brand_800`. Jamais de flèche dans le texte. Toute
  page ouverte depuis la barre latérale commence par un en-tête « Retour, puis titre aligné
  à gauche ».
- **Cartes de secret** : blanches, sans liseré ; le type est porté par la teinte de l'icône.
  Les emplacements identifiant et domaine sont toujours réservés pour aligner les cartes.
- **Barre latérale** : style `navigation-sidebar`, plate. Les compteurs d'audit et les
  pastilles d'état sont masqués lorsqu'ils valent zéro ou sont en cours de chargement.
- **Formulaires** : `adw::EntryRow`, `adw::PasswordEntryRow` et listes `boxed-list` ; une
  carte blanche par formulaire, sans cadres imbriqués.
- **Pastilles de choix** (type, coffre) : neutres au repos, `hv_brand_700` et texte blanc
  une fois sélectionnées. Les cases à cocher en pastille (familles du générateur) passent
  à une bordure `hv_brand_700` sur fond `hv_brand_50`.
- **États en ligne** (`inline-status-*`) : neutre pendant le chargement, puis paire
  sémantique (succès ou danger) ; jamais la sarcelle pour un succès.

## Accessibilité

- Contraste minimal WCAG 2.1 AA : 4,5:1 pour le texte et les icônes porteuses de sens,
  vérifié sur le fond réel par les tests natifs (`theme_tests.rs`, `assert_text_contrast`).
- Focus visible partout : anneau `0 0 0 3px alpha(hv_brand_600, 0.16)` et bordure
  `hv_brand_700` sur les champs.
- Une information ne repose jamais sur la couleur seule : chaque badge a un libellé, et
  chaque icône d'état a une infobulle ou un texte voisin.

## Ajouter ou modifier un style

1. Ajoutez la couleur manquante comme token dans `00-tokens.css`, jamais en dur dans une
   autre feuille.
2. Placez la règle dans la feuille du domaine concerné (`assets/style/NN-*.css`) ; une
   nouvelle feuille doit être importée dans `style.css` et listée dans `gresources.xml`.
3. Ciblez une classe propre au composant plutôt qu'une classe empruntée à un autre écran.
4. Lancez la suite native (`native_`) : elle échoue sur un avertissement GTK ou un contraste
   insuffisant.
