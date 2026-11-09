# Qualification GTK et préparation à la sortie de X11

Langue : FR

## Décision de qualification

La certification graphique Linux bloquante est **Wayland avec
`G_DEBUG=fatal-warnings`**, sans filtre de messages ni retrait des assertions
compact-width. Les contrôles Rust, API, dépendances et la recette Windows restent
obligatoires ; macOS reste en pause.

**X11 non certifié — mono-utilisateur Wayland — à réévaluer à la première
externalisation de l'application.** La [known-issue #76](https://github.com/ppaperso/heelonvault-core/issues/76)
ne bloque plus la cible Wayland. Aucun job de certification X11/Xvfb n'est conservé.
Une éventuelle compatibilité future visera XWayland uniquement ; retirer X11 du
périmètre ne prouve pas cette compatibilité et ne désactive pas X11 dans GTK système.

Le socle local observé est Fedora 45, GTK 4.24.1, libadwaita 1.10.0.
La qualification CI headless est complémentaire à la recette GNOME/Wayland réelle,
pas une certification de toutes les versions de GTK ou de tous les compositeurs.

## Banc de test

```sh
dbus-run-session -- bash scripts/run-wayland-tests.sh \
  cargo test --locked --workspace --all-features native_ -- --ignored --nocapture
```

Le [runner](../../scripts/run-wayland-tests.sh) utilise labwc/wlroots headless avec
rendu Pixman et clients GTK Cairo. Il attend une connexion Wayland réussie et la
présence de `wl_seat`, puis configure une sortie 1920×1600. Un compositeur de type
kiosque qui impose le plein écran rendrait le test compact-width invalide ; labwc
conserve les fenêtres redimensionnables. Weston headless sans siège d'entrée n'est
pas retenu : l'ouverture d'un popover déclenche une erreur GDK avec GTK local.

Le runtime et la configuration du compositeur sont privés, les variables ne sont
pas importées dans l'environnement D-Bus/systemd de l'utilisateur, et le processus
est arrêté à la sortie. Un échec ou un timeout affiche explicitement le journal.
`GDK_BACKEND=wayland`, `G_DEBUG=fatal-warnings` et `GTK_IM_MODULE=simple` sont
imposés aux commandes de test seulement. IBus et les réglages de production ne
changent pas ; la recette de saisie réelle reste distincte.

Validation locale : la commande headless passe avec le bus de session existant.
Un second bus démarré avec `dbus-run-session` sur ce poste Fedora échoue à activer
le service d'accessibilité (`Permission denied`), sans que le runner désactive
l'accessibilité ni masque l'avertissement. La CI installe les services D-Bus et
AT-SPI et utilise un bus isolé ; sa validation hébergée reste à effectuer après push.
Sur un bureau disposant déjà d'un bus fonctionnel, omettre `dbus-run-session`.

Le contrôle complémentaire dans la session GNOME avec le moteur par défaut a aussi
reproduit une instabilité distincte : largeur réelle 760 après demande 560, largeur
minimale 412, fenêtre ni maximisée ni plein écran. Elle n'est pas attribuée à X11
et n'est pas corrigée en filtrant ou sautant le test. La certification automatisée
porte sur le compositeur headless reproductible ; la recette du paquet dans la
session GNOME doit examiner ce résultat avant toute revendication de validation
complète du bureau réel.

Les [captures documentaires](USER_GUIDE_SCREENSHOTS.md) utilisent le même runner :
elles n'introduisent pas une dépendance Xvfb cachée dans la préparation des releases.

## Audit des usages et plan de migration

Périmètre : sources Rust et fichiers `.inc`, manifests et lockfiles des deux dépôts,
build scripts, scripts de lancement/installation, packaging Linux/Windows/macOS,
workflows et configuration Cargo. Recherches : `gdk_x11`, `gdk/x11`, `gdkx.h`,
`gtk4-x11`, `GDK_WINDOWING_X11`, Xlib/XCB, `XOpenDisplay`, `xrandr`, `xdotool`,
`wmctrl`, `GDK_BACKEND`, Xvfb et XWayland ; inspection du graphe Cargo et de pkg-config.

| Surface | Constat vérifié | Traitement / migration |
| --- | --- | --- |
| [CI native](../../.github/workflows/ci.yml) | Ancien backend `x11` et Xvfb explicitement forcés. | **Remplacés** par Wayland headless strict ; aucune certification X11 bloquante ou silencieusement tolérée. |
| [Workflow de captures](../../.github/workflows/user-guide-screenshots.yml) | Ancien second chemin Xvfb/X11. | **Remplacé** par le même runner Wayland ; seuils de tests inchangés. |
| [Presse-papiers](../../crates/heelonvault-app/src/ui/sensitive_clipboard.rs) : `storable_formats` | Commentaire X11, mais méthode portable `GdkContentProvider`, sans appel Xlib. Liste vide volontaire pour limiter la persistance. | **Conserver l'API portable** et sa politique ; ne pas supprimer une protection parce que son commentaire cite X11. Requalifier le provider et l'expiration pour GTK 5. |
| Même module : provider Windows, possession et focus | Commentaire comparant Windows à X11 ; implémentations sélectionnées par `cfg(windows)`, pas par backend X11. `GdkClipboard::is_local` et `GtkWindow::is_active` sont portables. | **Conserver** les branches Windows/non-Windows. Ne pas remplacer le focus par Xlib, ni promettre l'effacement des copies détenues par d'autres processus. |
| [Manifest UI](../../crates/heelonvault-app/Cargo.toml) et graphe Cargo | `gtk4` et `libadwaita` génériques ; aucune crate `gdk4-x11`, `x11`, `x11rb` ou `xcb` imposée par le graphe Rust actif. | **Aucune feature `x11-compat` nécessaire aujourd'hui.** Garder les API GDK portables. |
| [Build UI](../../crates/heelonvault-app/build.rs) | Compilation GResource et ressource PE Windows uniquement ; aucun header X11 ou identifiant de fenêtre natif. | **Conserver**. Si un futur usage X11 est indispensable, isoler dans un module optionnel `x11-compat`, désactivé par défaut, validé uniquement sous XWayland. |
| Dépendances natives gtk-rs | Métadonnées de `gtk4-sys` et `gdk4-sys` demandent `pkg-config gtk4`, pas `gtk4-x11`. GTK local annonce `broadway wayland x11`. | **Pas d'exigence X11 côté application.** La bibliothèque GTK distribuée peut toutefois lier Xlib/XCB ; auditer les bibliothèques effectivement embarquées à chaque paquet, ne pas les retirer aveuglément. |
| [AppImage](../../.github/workflows/linux-appimage-rc.yml), [AppRun](../../linux/apprun.sh), installateurs et Premium | Pas de backend X11 forcé, de `.pc` X11 applicatif, ni de workaround X11 dans les sources examinées. | **Conserver** les réglages de production. Vérifier la disponibilité Wayland dans GTK embarqué et tester le paquet sur GNOME/Wayland. |
| Headers, XID, macros et commandes X11 | Aucun usage applicatif de `gdk_x11_surface_get_xid`, `GDK_WINDOWING_X11`, `gdk/x11/gdkx.h`, Xlib/XCB ou outil de contrôle X11 trouvé. | **Rien à remplacer ou supprimer**. Le XID de la backtrace appartient au module système IBus, pas à HeelonVault. |

Ce résultat est un audit de dépendance à X11, **pas** une certification de compilation
GTK 5 : gtk-rs/libadwaita et leurs éventuelles ruptures d'API devront être migrés et
testés lors de leur disponibilité. XWayland a lui aussi besoin du backend X11 de
GTK côté client ; il n'est donc pas un remplacement de Wayland pour une future
application GTK sans backend X11.

## Brouillon amont GNOME/gtk (non envoyé)

**Title:** X11 frame-timing assertions while resizing GTK windows with GTK 4.24.1

**Environment:** Fedora 45, GTK 4.24.1, libadwaita 1.10.0, Xvfb 21.1.25;
also reproduced on the desktop X11 backend. Application uses no frame-timing API.

**Reproduction observed in the application suite:**

```sh
G_DEBUG=fatal-warnings GTK_IM_MODULE=simple GSK_RENDERER=cairo GDK_BACKEND=x11 \
  xvfb-run --auto-servernum cargo test --locked --workspace --all-features \
  native_ -- --ignored --nocapture
```

This is **not yet a standalone minimal reproducer**. The scenario presents
windows, drains the GLib main context, then resizes wide/compact windows. Preserve
these operations when reducing it; do not claim that a reduced program has been
verified until it has actually reproduced the failure.

**Observed diagnostics (alternating):**

```text
gdk_frame_timings_throttling_hint:
  self->result != GDK_FRAME_PREPARING
gdk_frame_timings_submitted() called on submitted frame.
```

**Backtrace:**

```text
gdk_frame_timings_throttling_hint
gdk_frame_clock_start
gdk_surface_thaw_updates
gdk_x11_display_translate_event
...
g_main_context_iteration
native_security_center_scenario (event drain during resize)
```

**Control:** the same application suite passes on Wayland with fatal warnings.
The IBus crash is separate; `GTK_IM_MODULE=simple` isolates it from this reproduction.
The frame-clock rework is a hypothesis, not a proven causal commit.

**Not performed:** reduced C/Rust reproducer and bisect between GTK 4.22.5 and 4.24.1.
No upstream regression range is claimed. Further investigation is optional,
time-bounded, and does not gate the Wayland-only qualification scope.
