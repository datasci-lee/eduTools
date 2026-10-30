#[cfg(windows)]
mod app;
#[cfg(windows)]
mod app_icon;
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
#[cfg(windows)]
mod log;
#[cfg(any(windows, test))]
mod monitor;
#[cfg(windows)]
mod overlay;
#[cfg(any(windows, test))]
mod palette;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod z_bands;

#[cfg(windows)]
fn main() {
    std::process::exit(app::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("EduTools 앱은 Windows 전용입니다");
    std::process::exit(1);
}
