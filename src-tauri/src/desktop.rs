//! Tray icon and start with Windows.
//!
//! The tray menu is built from labels and profiles sent by the UI, so it follows the
//! interface language; its clicks go back to the UI as `tray-profile`, `tray-theme` and
//! `tray-quit` events.
//! Start with Windows is a value under `HKCU\...\Run` holding this exe's path: it needs no
//! installer or admin rights, so it suits the portable app. If the folder moves, the next
//! start from the new place rewrites the path.

use crate::i18n::tr;
use serde::Deserialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

/// Passed by the startup entry: the app starts in the tray, without opening the window.
pub const AUTOSTART_ARG: &str = "--autostart";
const TRAY_ID: &str = "main";
const PROFILE_PREFIX: &str = "profile:";
const THEME_PREFIX: &str = "theme:";

pub fn started_by_autostart() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}

pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("Finestra principale non trovata");
        return;
    };
    for (what, result) in [("show", window.show()), ("unminimize", window.unminimize()), ("focus", window.set_focus())] {
        if let Err(e) = result {
            log::warn!("Finestra: {what} fallito: {e}");
        }
    }
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Lucent")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "quit" => {
                let _ = app.emit("tray-quit", ());
            }
            id => {
                if let Some(profile) = id.strip_prefix(PROFILE_PREFIX) {
                    let _ = app.emit("tray-profile", profile.to_string());
                } else if let Some(theme) = id.strip_prefix(THEME_PREFIX) {
                    let _ = app.emit("tray-theme", theme.to_string());
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[derive(Deserialize)]
pub struct TrayItem {
    id: String,
    name: String,
}

/// Labels in the interface language, the profiles to switch to, and the themes to apply to the
/// active profile.
#[derive(Deserialize)]
pub struct TrayMenu {
    show: String,
    quit: String,
    profiles: Vec<TrayItem>,
    active_profile: Option<String>,
    themes_title: String,
    themes: Vec<TrayItem>,
    presets: Vec<TrayItem>,
    /// Theme the active profile's colors come from, while unchanged.
    active_theme: Option<String>,
}

#[tauri::command]
pub fn set_tray_menu(app: AppHandle, menu: TrayMenu) -> Result<(), String> {
    let tray = app.tray_by_id(TRAY_ID).ok_or_else(|| tr("backend.tray_unavailable", &[]))?;
    let build = || -> tauri::Result<Menu<tauri::Wry>> {
        let item = |prefix: &str, p: &TrayItem, active: &Option<String>| {
            let checked = active.as_deref() == Some(p.id.as_str());
            CheckMenuItem::with_id(&app, format!("{prefix}{}", p.id), &p.name, true, checked, None::<&str>)
        };
        let root = Menu::new(&app)?;
        root.append(&MenuItem::with_id(&app, "show", &menu.show, true, None::<&str>)?)?;
        root.append(&PredefinedMenuItem::separator(&app)?)?;
        for p in &menu.profiles {
            root.append(&item(PROFILE_PREFIX, p, &menu.active_profile)?)?;
        }
        root.append(&PredefinedMenuItem::separator(&app)?)?;
        let themes = Submenu::new(&app, &menu.themes_title, true)?;
        for t in &menu.themes {
            themes.append(&item(THEME_PREFIX, t, &menu.active_theme)?)?;
        }
        if !menu.themes.is_empty() {
            themes.append(&PredefinedMenuItem::separator(&app)?)?;
        }
        for t in &menu.presets {
            themes.append(&item(THEME_PREFIX, t, &menu.active_theme)?)?;
        }
        root.append(&themes)?;
        root.append(&PredefinedMenuItem::separator(&app)?)?;
        root.append(&MenuItem::with_id(&app, "quit", &menu.quit, true, None::<&str>)?)?;
        Ok(root)
    };
    let menu = build().map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())
}

// ---- Start with Windows ----

/// What the startup entry runs: this exe, started in the tray.
fn startup_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" {AUTOSTART_ARG}", exe.display()))
}

#[cfg(windows)]
mod run_key {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Lucent";

    pub fn get() -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(PATH).ok()?.get_value(NAME).ok()
    }

    pub fn set(command: &str) -> std::io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(PATH)?;
        key.set_value(NAME, &command)
    }

    pub fn remove() -> std::io::Result<()> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(PATH, KEY_SET_VALUE)?;
        match key.delete_value(NAME) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

pub fn autostart_enabled() -> bool {
    #[cfg(windows)]
    return run_key::get().is_some();
    #[cfg(not(windows))]
    false
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let result = if enabled { run_key::set(&startup_command()?) } else { run_key::remove() };
        result.map_err(|e| tr("backend.autostart_failed", &[("error", &e)]))?;
        log::info!("Avvio con Windows {}", if enabled { "attivato" } else { "disattivato" });
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err(tr("backend.autostart_unsupported", &[]))
    }
}

/// Points the startup entry at this exe again, in case the portable folder was moved.
pub fn refresh_autostart() {
    #[cfg(windows)]
    if let (Some(current), Ok(wanted)) = (run_key::get(), startup_command())
        && current != wanted
    {
        match run_key::set(&wanted) {
            Ok(()) => log::info!("Avvio con Windows aggiornato al nuovo percorso: {wanted}"),
            Err(e) => log::warn!("Avvio con Windows non aggiornato ({e}): punta ancora a {current}"),
        }
    }
}
