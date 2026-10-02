# Charte Graphique HEELONYS

Langue : FR | [EN](CHARTE_GRAPHIQUE.en.md)

## 🎨 Palette de Couleurs

### Couleurs Primaires

Basées sur les couleurs extraites du logo Logo_Heelonys.jpg

#### Turquoise Sarcelle (Principal)

- **Deep Teal** : `#07393A` - Titres, textes importants, navigation
- **Dark Teal** : `#0A5F5C` - Éléments d'interface, boutons secondaires

#### Turquoise (Accent)

- **Medium Teal** : `#1F8678` - Boutons principaux, liens actifs
- **Bright Teal** : `#13A1A1` - Accents, highlights, icônes

#### Turquoise Clair (Complémentaire)

- **Soft Teal** : `#57B9B1` - Backgrounds clairs, hover states
- **Light Teal** : `#6FD3B4` - Éléments subtils, badges
- **Very Light Teal** : `#A4DFCF` - Backgrounds cards, sections

### Couleurs Neutres

- **Off White** : `#F3F6F3` - Background principal
- **Pure White** : `#FFFFFF` - Cards, panneaux
- **Charcoal** : `#2C3E50` - Textes principaux
- **Gray** : `#7F8C9A` - Textes secondaires, descriptions

### Couleurs de Support

- **Success Green** : `#6FD3B4` - Messages de succès
- **Warning Teal** : `#13A1A1` - Informations importantes
- **Error Red** : `#E74C3C` - Erreurs (utiliser avec parcimonie)

---

## 📐 Typographie

### Police Principale

- **Family** : Space Grotesk
- **Weights** : 400 (Regular), 500 (Medium), 600 (Semi-Bold), 700 (Bold)
- **Usage** : Tous les textes du site

### Hiérarchie

- **H1** : 3.4rem / 54px - Bold (700)
- **H2** : 2rem / 32px - Semi-Bold (600)
- **H3** : 1.2rem / 19px - Semi-Bold (600)
- **Body** : 1rem / 16px - Regular (400)
- **Small** : 0.85rem / 14px - Regular (400)

### Line Height

- Titres : 1.2
- Paragraphes : 1.6

---

## 🎯 Principes d'Utilisation

### Couleur Primaire (#07393A)

- Titres de sections
- Logo text
- Navigation active
- Footer background

### Couleur Accent (#13A1A1 / #1F8678)

- Boutons Call-to-Action
- Liens interactifs
- Icônes importantes
- Underline des liens actifs

### Backgrounds

1. **Principal** : #F3F6F3 (très léger turquoise)
2. **Cards** : #FFFFFF
3. **Hero/Sections sombres** : Gradient #07393A → #0A5F5C
4. **Sections claires** : #A4DFCF

---

## 🖼️ Composants

### Boutons

#### Bouton Primaire

- Background : Gradient `linear-gradient(120deg, #13A1A1, #1F8678)`
- Texte : Blanc
- Border-radius : 999px (pill shape)
- Padding : 12px 20px
- Hover : Déplacement vers le haut (-2px) + ombre

#### Bouton Outline

- Background : Transparent
- Border : 1px solid #07393A
- Texte : #07393A
- Border-radius : 999px
- Hover : Background #F3F6F3

### Cards

- Background : Blanc
- Border : 1px solid #A4DFCF
- Border-radius : 20px
- Padding : 30px
- Box-shadow : 0 20px 40px rgba(7, 57, 58, 0.08)

### Badges/Pills

- Background : rgba(19, 161, 161, 0.12)
- Texte : #0A5F5C
- Border-radius : 999px
- Padding : 6px 14px

---

## 🎨 Dégradés

### Hero Section

```css
linear-gradient(135deg, #07393A, #0A5F5C)
```

### Boutons Primaires

```css
linear-gradient(120deg, #13A1A1, #1F8678)
```

### Cards avec effet

```css
linear-gradient(135deg, #FFFFFF, #F3F6F3)
```

---

## 💡 États Interactifs

### Hover

- Transformation : `translateY(-2px)`
- Ombre : Augmentation de l'intensité
- Couleur : Transition vers couleur plus foncée

### Focus

- Outline : 2px solid #13A1A1
- Offset : 2px

### Active

- Transformation : `translateY(0)`
- Couleur : Teinte plus sombre de 10%

---

## 🌐 Accessibilité

### Ratios de Contraste

- Texte principal sur fond clair : #2C3E50 sur #F3F6F3 (ratio > 7:1)
- Texte blanc sur fond sombre : #FFFFFF sur #07393A (ratio > 12:1)
- Liens : #0A5F5C avec underline visible

### Respect des Standards

- WCAG 2.1 Niveau AA minimum
- Support du mode réduit de mouvement
- Focus visible sur tous les éléments interactifs

---

## 📱 Responsive Design

### Breakpoints

- Mobile : < 600px
- Tablet : 600px - 960px
- Desktop : > 960px

### Grilles

- Gap : 20-40px selon le contexte
- Colonnes : `repeat(auto-fit, minmax(280px, 1fr))`

---

## ✨ Animations

### Transitions Standards

- Durée : 0.2s - 0.25s
- Timing : ease ou cubic-bezier(0.4, 0, 0.2, 1)

### Éléments Animés

- Boutons : hover transform + shadow
- Navigation : underline avec scaleX
- Cards : hover elevation

---

## 📋 Checklist d'Application

- ✅ Toujours utiliser les couleurs turquoise du logo
- ✅ Éviter les bleus purs (#00c6ff supprimé)
- ✅ Maintenir la cohérence avec le logo
- ✅ Utiliser les dégradés turquoise pour les sections importantes
- ✅ Préserver les contrastes pour l'accessibilité
- ✅ Tester sur différents écrans
