#[cfg(windows)]
mod app;
#[cfg(windows)]
mod app_icon;
#[cfg(any(windows, test))]
mod attr_icon_input;
#[cfg(windows)]
mod break_screen;
#[cfg(windows)]
mod capture;
#[cfg(any(windows, test))]
mod display_apply;
#[cfg(any(windows, test))]
mod help;
#[cfg(any(windows, test))]
mod help_state;
#[cfg(windows)]
mod hotkey;
#[cfg(test)]
mod icon_assets;
#[cfg(windows)]
mod ink_view;
#[cfg(windows)]
mod kbhook;
#[cfg(windows)]
mod keymap;
#[cfg(any(windows, test))]
mod lesson_badge;
#[cfg(windows)]
mod log;
#[cfg(any(windows, test))]
mod monitor;
#[cfg(any(windows, test))]
mod music;
#[cfg(windows)]
mod overlay;
#[cfg(any(windows, test))]
mod palette;
#[cfg(windows)]
mod radial_palette;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod selection;
#[cfg(windows)]
mod settings_file;
#[cfg(any(windows, test))]
mod settings_ui;
#[cfg(windows)]
mod text_entry;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod z_bands;
#[cfg(windows)]
mod zoom_view;

#[cfg(windows)]
fn main() {
    std::process::exit(app::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("EduTools 앱은 Windows 전용입니다");
    std::process::exit(1);
}
