pub mod activity;
pub mod app;
pub mod audio;
pub mod autostart;
mod game_watcher;
pub mod games;
/// 鼠标：独立 crate `openmouse-mouse`（原 `gpui/src/hid.rs`）。
pub use openmouse_mouse as hid;
pub mod native_window;
pub mod overlay;
pub mod presence_text;
pub mod tray;
pub mod profiles;
pub mod settings;
pub mod theme;
/// 键盘：独立 crate `openmouse-keyboard`。
pub use openmouse_keyboard;

#[cfg(windows)]
mod wallpaper_win;
