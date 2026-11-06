//! OS helpers: autostart, single instance, theme, work area, window chrome.
//! Windows implementation is `platform_win.rs`, Linux in `platform_linux.rs`.

#[cfg(not(windows))]
#[path = "platform_linux.rs"]
mod linux;
#[cfg(windows)]
#[path = "platform_win.rs"]
mod win;

#[cfg(not(windows))]
pub use linux::*;
#[cfg(windows)]
pub use win::*;
