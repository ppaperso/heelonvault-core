use super::*;

impl MainWindow {
    pub(super) const DEFAULT_AUTO_LOCK_TIMEOUT_SECS: u64 = 5 * 60;
    pub(super) const DEFAULT_WINDOW_WIDTH: i32 = 1180;
    pub(super) const DEFAULT_WINDOW_HEIGHT: i32 = 760;
    pub(super) const MIN_WINDOW_WIDTH: i32 = 980;
    pub(super) const MIN_WINDOW_HEIGHT: i32 = 640;

    pub(super) fn professional_customer_name(license_badge_text: &str) -> Option<String> {
        for prefix in [
            "Licence pro - ",
            "Pro: ",
            "Professional - ",
            "Professional: ",
        ] {
            if let Some(customer_name) = license_badge_text.strip_prefix(prefix) {
                let normalized = customer_name.trim();
                if !normalized.is_empty() {
                    return Some(normalized.to_ascii_uppercase());
                }
            }
        }

        None
    }

    pub(in crate::ui::windows::main_window) fn build_header_license_badge(
        license_badge_text: &str,
    ) -> gtk4::Widget {
        header::build_header_license_badge(license_badge_text)
    }

    pub(in crate::ui::windows::main_window) fn initial_window_launch(
    ) -> window_sizing::MainWindowLaunch {
        window_sizing::resolve_main_window_launch(
            Self::DEFAULT_WINDOW_WIDTH,
            Self::DEFAULT_WINDOW_HEIGHT,
            Self::MIN_WINDOW_WIDTH,
            Self::MIN_WINDOW_HEIGHT,
        )
    }
}
