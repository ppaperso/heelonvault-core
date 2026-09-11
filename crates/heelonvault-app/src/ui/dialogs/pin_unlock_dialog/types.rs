use gtk4::{Box, Button, Entry, Image, Label, Window};
use libadwaita as adw;

/// Structure contenant tous les widgets de la dialogue de debitage par PIN.
#[allow(dead_code)]
#[derive(Clone)]
pub struct PinUnlockDialogWidgets {
    pub window: Window,
    pub root: Box,
    pub header: adw::HeaderBar,
    pub body: Box,
    pub lock_icon: Image,
    pub title_label: Label,
    pub subtitle_label: Label,
    pub pin_entry: Entry,
    pub feedback_label: Label,
    pub btn_row: Box,
    pub unlock_button: Button,
    pub fallback_button: Button,
    pub quit_button: Button,
}
