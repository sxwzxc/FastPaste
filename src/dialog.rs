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

pub fn show_confirm(title: &str, msg: &str) -> bool {
    let result = MessageDialog::new()
        .set_title(title)
        .set_description(msg)
        .set_level(MessageLevel::Warning)
        .set_buttons(MessageButtons::YesNo)
        .show();
    result == rfd::MessageDialogResult::Yes
}

pub fn notify_bubble(title: &str, body: &str) {
    // 托盘气泡在 tray 模块中直接通过 TrayIcon 实现，这里仅日志
    log::info!("[BUBBLE] {}: {}", title, body);
}
