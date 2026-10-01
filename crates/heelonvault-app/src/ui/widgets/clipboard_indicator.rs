//! Header bar indicator of what HeelonVault exposes through the clipboard.
//!
//! A neutral clipboard icon inside a ring that burns down like a fuse while a sensitive value
//! is served by the clipboard. Clicking it clears the clipboard at once.
//!
//! The tooltip only states what the application controls (see `sensitive_clipboard`): it never
//! claims that no secret is in memory — the open session keeps its master key, GTK keeps its
//! text buffers, and OS clipboard history is out of reach.

use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

use gtk4::glib;
use gtk4::prelude::*;
use heelonvault_core::i18n::I18nArg;

use crate::ui::sensitive_clipboard::{self, Exposure, SensitiveKind};

const RING_SIZE: i32 = 26;
const RING_WIDTH: f64 = 2.5;
/// Teal-grey track, the same muted tone as the header icons.
const TRACK_RGBA: (f64, f64, f64, f64) = (0.027, 0.224, 0.227, 0.16);
/// Amber "fuse" while a value is exposed.
const FUSE_RGB: (f64, f64, f64) = (0.851, 0.557, 0.016);
const EXPOSED_CLASS: &str = "exposed";

/// Share of the clearing delay still remaining, in `0.0..=1.0`.
fn remaining_fraction(now: Instant, copied_at: Instant, expires_at: Instant) -> f64 {
    let total = expires_at
        .saturating_duration_since(copied_at)
        .as_secs_f64();
    if total <= 0.0 {
        return 0.0;
    }
    let left = expires_at.saturating_duration_since(now).as_secs_f64();
    (left / total).clamp(0.0, 1.0)
}

/// Whole seconds left before the clipboard is cleared, rounded up (never shows "0 s" early).
fn remaining_seconds(now: Instant, expires_at: Instant) -> u64 {
    let left = expires_at.saturating_duration_since(now);
    left.as_secs() + u64::from(left.subsec_nanos() > 0)
}

fn tooltip_for(exposure: Exposure, now: Instant) -> String {
    match exposure {
        Exposure::Idle => heelonvault_core::tr!("clipboard-indicator-idle"),
        Exposure::Decrypting => heelonvault_core::tr!("clipboard-indicator-decrypting"),
        Exposure::InClipboard {
            kind, expires_at, ..
        } => {
            let key = match kind {
                SensitiveKind::Password => "clipboard-indicator-password",
                SensitiveKind::Login => "clipboard-indicator-login",
                SensitiveKind::RecoveryPhrase => "clipboard-indicator-recovery",
            };
            let seconds = i64::try_from(remaining_seconds(now, expires_at)).unwrap_or(i64::MAX);
            format!(
                "{}\n{}",
                heelonvault_core::i18n::tr_args(key, &[("seconds", I18nArg::Num(seconds))]),
                heelonvault_core::tr!("clipboard-indicator-clear-hint"),
            )
        }
    }
}

struct IndicatorState {
    exposure: Cell<Exposure>,
    /// A tick callback is animating the ring.
    ticking: Cell<bool>,
    /// Seconds shown in the tooltip, to rewrite it only when the countdown changes.
    shown_seconds: Cell<Option<u64>>,
}

/// The header widget. Cloning shares the same widget (GTK handles are refcounted).
#[derive(Clone)]
pub struct ClipboardIndicator {
    button: gtk4::Button,
    ring: gtk4::DrawingArea,
    state: Rc<IndicatorState>,
}

impl ClipboardIndicator {
    pub fn new() -> Self {
        let ring = gtk4::DrawingArea::builder()
            .content_width(RING_SIZE)
            .content_height(RING_SIZE)
            .build();

        let icon = gtk4::Image::from_icon_name("edit-paste-symbolic");
        icon.set_pixel_size(14);
        icon.set_halign(gtk4::Align::Center);
        icon.set_valign(gtk4::Align::Center);

        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&ring));
        overlay.add_overlay(&icon);

        let button = gtk4::Button::builder().child(&overlay).build();
        button.add_css_class("flat");
        button.add_css_class("header-clipboard-indicator");
        button.connect_clicked(|_| sensitive_clipboard::clear_now());

        let indicator = Self {
            button,
            ring,
            state: Rc::new(IndicatorState {
                exposure: Cell::new(Exposure::Idle),
                ticking: Cell::new(false),
                shown_seconds: Cell::new(None),
            }),
        };

        let state_for_draw = Rc::clone(&indicator.state);
        indicator.ring.set_draw_func(move |_, cr, width, height| {
            draw_ring(cr, width, height, state_for_draw.exposure.get());
        });

        // The subscription only holds weak references: it ends with the widget, so main
        // windows rebuilt at each sign-in do not pile up listeners.
        let button_weak = indicator.button.downgrade();
        let ring_weak = indicator.ring.downgrade();
        let state_weak = Rc::downgrade(&indicator.state);
        sensitive_clipboard::subscribe(move |exposure| {
            let (Some(button), Some(ring), Some(state)) = (
                button_weak.upgrade(),
                ring_weak.upgrade(),
                state_weak.upgrade(),
            ) else {
                return glib::ControlFlow::Break;
            };
            Self {
                button,
                ring,
                state,
            }
            .apply(exposure);
            glib::ControlFlow::Continue
        });

        indicator
    }

    pub fn widget(&self) -> &gtk4::Button {
        &self.button
    }

    /// Re-translate the tooltip after a language change.
    pub fn refresh_i18n(&self) {
        self.state.shown_seconds.set(None);
        self.refresh_tooltip(Instant::now());
    }

    fn apply(&self, exposure: Exposure) {
        self.state.exposure.set(exposure);
        self.state.shown_seconds.set(None);

        if matches!(exposure, Exposure::Idle) {
            self.button.remove_css_class(EXPOSED_CLASS);
        } else {
            self.button.add_css_class(EXPOSED_CLASS);
        }
        self.refresh_tooltip(Instant::now());
        self.ring.queue_draw();

        if matches!(exposure, Exposure::InClipboard { .. }) && !self.state.ticking.replace(true) {
            self.start_ticking();
        }
    }

    /// Animate the ring frame by frame while a value is in the clipboard; stops by itself
    /// afterwards, so the idle indicator costs nothing. Ticks only run while it is mapped.
    fn start_ticking(&self) {
        // Weak references: the ring owns this callback, a strong one would keep it alive.
        let button_weak = self.button.downgrade();
        let state_weak = Rc::downgrade(&self.state);
        self.ring.add_tick_callback(move |ring, _clock| {
            let (Some(button), Some(state)) = (button_weak.upgrade(), state_weak.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            if !matches!(state.exposure.get(), Exposure::InClipboard { .. }) {
                state.ticking.set(false);
                return glib::ControlFlow::Break;
            }
            let indicator = Self {
                button,
                ring: ring.clone(),
                state,
            };
            indicator.refresh_tooltip(Instant::now());
            ring.queue_draw();
            glib::ControlFlow::Continue
        });
    }

    fn refresh_tooltip(&self, now: Instant) {
        let exposure = self.state.exposure.get();
        let seconds = match exposure {
            Exposure::InClipboard { expires_at, .. } => Some(remaining_seconds(now, expires_at)),
            Exposure::Idle | Exposure::Decrypting => None,
        };
        if seconds.is_some() && self.state.shown_seconds.get() == seconds {
            return;
        }
        self.state.shown_seconds.set(seconds);

        let text = tooltip_for(exposure, now);
        // Also what screen readers announce: GTK exposes the tooltip as the description.
        self.button.set_tooltip_text(Some(text.as_str()));
    }
}

fn draw_ring(cr: &gtk4::cairo::Context, width: i32, height: i32, exposure: Exposure) {
    let size = f64::from(width.min(height));
    let center_x = f64::from(width) / 2.0;
    let center_y = f64::from(height) / 2.0;
    let radius = (size - RING_WIDTH) / 2.0;
    if radius <= 0.0 {
        return;
    }
    cr.set_line_width(RING_WIDTH);
    cr.set_line_cap(gtk4::cairo::LineCap::Round);

    let (red, green, blue, alpha) = TRACK_RGBA;
    cr.set_source_rgba(red, green, blue, alpha);
    cr.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
    if cr.stroke().is_err() {
        return;
    }

    let (fraction, alpha) = match exposure {
        Exposure::Idle => return,
        // Too brief to animate: a faint full ring.
        Exposure::Decrypting => (1.0, 0.45),
        Exposure::InClipboard {
            copied_at,
            expires_at,
            ..
        } => (
            remaining_fraction(Instant::now(), copied_at, expires_at),
            1.0,
        ),
    };
    if fraction <= 0.0 {
        return;
    }
    // The fuse starts at 12 o'clock and burns down counter-clockwise.
    let start = -PI / 2.0;
    let (red, green, blue) = FUSE_RGB;
    cr.set_source_rgba(red, green, blue, alpha);
    cr.arc(
        center_x,
        center_y,
        radius,
        start,
        start + 2.0 * PI * fraction,
    );
    let _ = cr.stroke();
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_fuse_burns_down_over_the_clearing_delay() {
        let copied_at = Instant::now();
        let expires_at = copied_at + Duration::from_secs(20);

        assert!((remaining_fraction(copied_at, copied_at, expires_at) - 1.0).abs() < f64::EPSILON);
        let halfway = copied_at + Duration::from_secs(10);
        assert!((remaining_fraction(halfway, copied_at, expires_at) - 0.5).abs() < 1e-9);
        assert!(remaining_fraction(expires_at, copied_at, expires_at).abs() < f64::EPSILON);
        let later = expires_at + Duration::from_secs(5);
        assert!(remaining_fraction(later, copied_at, expires_at).abs() < f64::EPSILON);
    }

    #[test]
    fn a_zero_delay_never_divides_by_zero() {
        let now = Instant::now();
        assert!(remaining_fraction(now, now, now).abs() < f64::EPSILON);
    }

    #[test]
    fn the_countdown_rounds_up() {
        let now = Instant::now();
        assert_eq!(remaining_seconds(now, now + Duration::from_secs(20)), 20);
        assert_eq!(
            remaining_seconds(now, now + Duration::from_millis(19_200)),
            20
        );
        assert_eq!(remaining_seconds(now, now + Duration::from_millis(300)), 1);
        assert_eq!(remaining_seconds(now, now), 0);
        assert_eq!(remaining_seconds(now + Duration::from_secs(1), now), 0);
    }
}
