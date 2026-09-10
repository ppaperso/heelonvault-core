use gtk4::{Box as GtkBox, Button, Entry, Label};
use libadwaita as adw;

/// Structure contenant tous les widgets de la dialogue de configuration du PIN.
#[allow(dead_code)]
#[derive(Clone)]
pub struct PinSetupDialogWidgets {
    pub window: gtk4::Window,
    pub root: GtkBox,
    pub header: adw::HeaderBar,
    pub body: GtkBox,
    pub desc_label: Label,
    pub len_hint: Label,
    pub pin_entry: Entry,
    pub confirm_entry: Entry,
    pub feedback_label: Label,
    pub btn_row: GtkBox,
    pub save_button: Button,
    pub disable_button: Option<Button>,
}
