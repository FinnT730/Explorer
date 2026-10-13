//! Window backdrop effects: Mica and acrylic on Windows 11, the closest
//! vibrancy material on macOS, nothing elsewhere.
//!
//! The effect is applied to the native window once, at startup. Whether it
//! took is reported back so the theme can stay opaque when it did not —
//! a translucent theme over an opaque window just looks washed out.

use crate::settings::Backdrop;

/// Apply `backdrop` to the window behind `handle`.
///
/// Returns true only when the platform actually applied it, which is false on
/// Windows 10, on Linux, and whenever the requested material is unavailable.
#[cfg(any(windows, target_os = "macos"))]
#[cfg_attr(target_os = "macos", allow(unused_variables))]
pub fn apply(
    handle: &impl raw_window_handle::HasWindowHandle,
    backdrop: Backdrop,
    dark: bool,
) -> bool {
    use window_vibrancy as wv;

    let result = match backdrop {
        Backdrop::None => return false,
        #[cfg(windows)]
        Backdrop::Mica => wv::apply_mica(handle, Some(dark)),
        #[cfg(windows)]
        Backdrop::Tabbed => wv::apply_tabbed(handle, Some(dark)),
        #[cfg(windows)]
        Backdrop::Acrylic => wv::apply_acrylic(handle, None),
        // macOS has no Mica; its sidebar material is the closest thing to the
        // wallpaper-tinted look, and the HUD material to acrylic.
        #[cfg(target_os = "macos")]
        Backdrop::Mica | Backdrop::Tabbed => wv::apply_vibrancy(
            handle,
            wv::NSVisualEffectMaterial::Sidebar,
            Some(wv::NSVisualEffectState::Active),
            None,
        ),
        #[cfg(target_os = "macos")]
        Backdrop::Acrylic => wv::apply_vibrancy(
            handle,
            wv::NSVisualEffectMaterial::HudWindow,
            Some(wv::NSVisualEffectState::Active),
            None,
        ),
    };

    match result {
        Ok(()) => true,
        Err(err) => {
            // Older Windows, or a window the compositor will not composite.
            eprintln!("explorer: backdrop unavailable ({err})");
            false
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn apply<H>(_handle: &H, _backdrop: Backdrop, _dark: bool) -> bool {
    false
}

/// Whether this build can do backdrops at all, for the settings menu.
pub const fn supported() -> bool {
    cfg!(any(windows, target_os = "macos"))
}
