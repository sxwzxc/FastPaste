use rfd::{MessageButtons, MessageDialog, MessageLevel};

pub fn show_info(title: &str, msg: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(msg)
        .set_level(MessageLevel::Info)
        .set_buttons(MessageButtons::Ok)
        .show();
}

pub fn show_warning(title: &str, msg: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(msg)
        .set_level(MessageLevel::Warning)
        .set_buttons(MessageButtons::Ok)
        .show();
}

pub fn show_error(title: &str, msg: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_description(msg)
        .set_level(MessageLevel::Error)
        .set_buttons(MessageButtons::Ok)
        .show();
}
