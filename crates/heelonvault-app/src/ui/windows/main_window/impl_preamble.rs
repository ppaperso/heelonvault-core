use super::*;

impl MainWindow {
    pub(super) const DEFAULT_WINDOW_WIDTH: i32 = 1180;
    pub(super) const DEFAULT_WINDOW_HEIGHT: i32 = 760;
    pub(super) const MIN_WINDOW_WIDTH: i32 = 980;
    pub(super) const MIN_WINDOW_HEIGHT: i32 = 640;

    pub(in crate::ui::windows::main_window) fn initial_window_launch()
    -> window_sizing::MainWindowLaunch {
        window_sizing::resolve_main_window_launch(
            Self::DEFAULT_WINDOW_WIDTH,
            Self::DEFAULT_WINDOW_HEIGHT,
            Self::MIN_WINDOW_WIDTH,
            Self::MIN_WINDOW_HEIGHT,
        )
    }
}
