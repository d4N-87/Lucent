#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
mod i18n;
mod mcp;
mod os_layout;

use i18n::tr;

use lucent_core::driver::{Capabilities, DeviceInfo, EffectSettings, KeyAction, KeyConfig, KeyboardDriver};
use lucent_core::drivers::{Detection, UnknownKeyboard, detect};
use lucent_core::keyboard::model::{self, KeyDef, Model};
use hidapi::HidApi;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tokio_util::sync::CancellationToken;

/// HID state shared by all commands. `HidApi` is kept alive so devices can be re-enumerated.
struct Backend {
    api: Option<HidApi>,
    device: Option<Box<dyn KeyboardDriver>>,
    /// The connected keyboard's model as detected (or the last one when nothing is connected).
    base: Model,
    /// What the app draws and edits: `base` in the ISO or ANSI shape the user has (see
    /// `apply_standard`), or a format chosen to preview a keyboard without a model.
    layout: Model,
    /// Support level of the connected keyboard (see `Status::support`).
    support: &'static str,
    unknown: Option<UnknownKeyboard>,
    /// USB ids of every model, from the last connect: what check_keyboard watches for.
    known_ids: Vec<(u16, u16)>,
}

impl Default for Backend {
    fn default() -> Self {
        let model = model::default_model();
        Self { api: None, device: None, base: model.clone(), layout: model, support: "none", unknown: None, known_ids: Vec::new() }
    }
}

#[derive(Serialize)]
struct Status {
    connected: bool,
    message: String,
    /// "verified" (everything checked on this model), "unverified" (a model from the `models`
    /// folder: key writes need the user's OK), "unconfigured" (a keyboard without a model: preview
    /// only, nothing is sent to it) or "none".
    support: &'static str,
    /// The keyboard without a model, when `support` is "unconfigured".
    unknown: Option<UnknownKeyboard>,
}

#[derive(Serialize)]
struct Layout {
    id: String,
    name: String,
    /// "model" (a real keyboard) or "format" (a drawing, for preview only).
    kind: &'static str,
    /// "iso" or "ansi": the shape drawn.
    standard: String,
    /// The shape the model was verified in, when it was: another one makes it "unverified".
    verified_as: Option<String>,
    width: f32,
    height: f32,
    keys: Vec<KeyDef>,
    led_count: usize,
    /// The Windows keyboard layout the legends come from, and what each key types with it (by
    /// key id), so text macros press the right keys. Missing when Windows can't tell.
    typing: Option<os_layout::OsLayout>,
}

impl From<&Model> for Layout {
    fn from(m: &Model) -> Self {
        Layout {
            id: m.id.clone(),
            name: m.name.clone(),
            kind: if m.is_format() { "format" } else { "model" },
            standard: m.standard.clone(),
            verified_as: None,
            width: m.width,
            height: m.height,
            keys: m.keys.clone(),
            led_count: m.led_count(),
            typing: None,
        }
    }
}

/// `models` next to the executable: a contributor's own keyboards (docs/nuova-tastiera.md).
fn models_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("models"))
}

/// The contributor's models first, so they win over a built-in model with the same USB ids.
fn all_models() -> Vec<Model> {
    let (mut models, problems) = models_dir().map(|d| model::load_dir(&d)).unwrap_or_default();
    for p in problems {
        log::warn!("Modello ignorato: {p}");
    }
    for m in &models {
        log::info!("Modello dalla cartella models (non verificato): {} ({})", m.name, m.id);
    }
    models.extend(model::builtin_models());
    models
}

/// For trying the unconfigured-keyboard screens without such a keyboard: start the app with
/// LUCENT_SIMULATE_UNKNOWN=1.
fn simulated_unknown() -> Option<UnknownKeyboard> {
    std::env::var_os("LUCENT_SIMULATE_UNKNOWN").map(|_| UnknownKeyboard {
        vid: "320F".into(),
        pid: "5044".into(),
        vendor: "Glorious".into(),
        manufacturer: "Glorious".into(),
        product: "GMMK Pro (simulata)".into(),
    })
}

impl Backend {
    /// `preview` is the format to draw if the keyboard has no model.
    fn connect(&mut self, preview: Option<String>, standard: Option<String>, os: Option<&os_layout::OsLayout>) -> Status {
        self.device = None;
        self.unknown = None;
        self.support = "none";
        let api = match self.api.as_mut() {
            Some(api) => {
                let _ = api.refresh_devices();
                api
            }
            None => match HidApi::new() {
                Ok(api) => self.api.insert(api),
                Err(e) => return self.status(tr("backend.hid_init_failed", &[("error", &e)])),
            },
        };
        let models = all_models();
        self.known_ids = models.iter().filter_map(|m| m.usb.as_ref()?.parse()).collect();
        let detection = match simulated_unknown() {
            Some(k) => Detection::Unconfigured(k),
            None => detect(api, &models),
        };
        match detection {
            Detection::Known { model, driver } => {
                let message = tr("backend.keyboard_connected", &[("model", &driver.info().model)]);
                log::info!("{message} · driver {} · modello {}", driver.info().driver, model.id);
                self.device = Some(driver);
                self.base = model;
                self.apply_standard(standard.as_deref(), os);
                self.status(message)
            }
            Detection::Failed { model, error } => {
                log::warn!("Connessione fallita ({}): {error}", model.id);
                self.status(tr("backend.keyboard_open_failed", &[("error", &error)]))
            }
            Detection::Unconfigured(k) => {
                log::warn!(
                    "Tastiera senza modello: {} {} (VID {} PID {}): solo anteprima, nessun comando inviato",
                    k.manufacturer,
                    k.product,
                    k.vid,
                    k.pid
                );
                let formats = model::builtin_formats();
                let wanted = preview.and_then(|id| formats.iter().find(|f| f.id == id).cloned());
                self.layout = wanted.unwrap_or_else(|| formats[0].clone());
                self.base = self.layout.clone();
                self.support = "unconfigured";
                let message = tr("backend.keyboard_unconfigured", &[("name", &format!("{} {}", k.manufacturer, k.product))]);
                self.unknown = Some(k);
                self.status(message)
            }
            Detection::None => {
                log::warn!("Connessione fallita: nessuna tastiera supportata trovata");
                self.status(tr("backend.keyboard_not_found", &[]))
            }
        }
    }

    /// Draws the model ISO or ANSI: the user's choice, else ANSI when Windows types with a US layout
    /// on an ISO model. A shape the model wasn't verified in is "unverified": the keys may sit
    /// elsewhere in its keymap, so key writes wait for the user's OK.
    fn apply_standard(&mut self, setting: Option<&str>, os: Option<&os_layout::OsLayout>) {
        if self.support == "unconfigured" {
            return;
        }
        let wanted = setting.filter(|s| matches!(*s, "iso" | "ansi")).map(str::to_string).unwrap_or_else(|| self.guess_standard(os));
        self.layout = match wanted.as_str() {
            "ansi" => self.base.as_ansi().unwrap_or_else(|| self.base.clone()),
            _ => self.base.clone(),
        };
        if self.device.is_some() {
            let as_verified = self.base.verified && self.layout.standard == self.base.standard;
            self.support = if as_verified { "verified" } else { "unverified" };
        }
    }

    /// The shape "auto" picks: ANSI for an ISO model when Windows types with a US layout.
    fn guess_standard(&self, os: Option<&os_layout::OsLayout>) -> String {
        let us = os.is_some_and(|os| os.id.ends_with("0409"));
        if us && self.base.standard == "iso" { "ansi".into() } else { self.base.standard.clone() }
    }

    fn status(&self, message: String) -> Status {
        Status { connected: self.device.is_some(), message, support: self.support, unknown: self.unknown.clone() }
    }
}

/// Runs `f` on a blocking thread with the backend locked: HID calls block (a color upload takes
/// ~150 ms, a key write longer).
async fn with_backend<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&mut Backend) -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Mutex<Backend>>();
        let mut backend = state.lock().map_err(|_| tr("backend.hid_state_unavailable", &[]))?;
        Ok(f(&mut backend))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether the keyboard came or went, from the USB device list only: nothing is sent to it.
/// "lost" (the connected keyboard, or the one without a model, is gone; the app forgets it),
/// "available" (a keyboard with a model is plugged in but not connected), or "same".
#[tauri::command]
async fn check_keyboard(app: AppHandle) -> Result<&'static str, String> {
    with_backend(app, |b| {
        let Some(api) = b.api.as_mut() else { return "same" };
        if api.refresh_devices().is_err() {
            return "same";
        }
        let present = |vid: u16, pid: u16| api.device_list().any(|d| d.vendor_id() == vid && d.product_id() == pid);
        let hex = |s: &str| u16::from_str_radix(s, 16).ok();
        if b.device.is_some() {
            let ids = b.layout.usb.as_ref().and_then(|u| u.parse());
            if ids.is_some_and(|(vid, pid)| !present(vid, pid)) {
                log::warn!("Tastiera scollegata");
                b.device = None;
                b.support = "none";
                return "lost";
            }
            return "same";
        }
        if let Some(k) = &b.unknown {
            if let (Some(vid), Some(pid)) = (hex(&k.vid), hex(&k.pid))
                && !present(vid, pid)
            {
                b.unknown = None;
                b.support = "none";
                return "lost";
            }
            return "same";
        }
        let known = b.known_ids.iter().any(|&(vid, pid)| present(vid, pid));
        if known { "available" } else { "same" }
    })
    .await
}

#[tauri::command]
async fn connect(app: AppHandle) -> Result<Status, String> {
    let settings = load_settings(&app);
    let os = typing_layout(&app, &settings);
    with_backend(app, move |b| b.connect(settings.preview_format, settings.key_format, os.as_ref())).await
}

/// The Windows layout the legends and text macros follow: the chosen one if still installed,
/// else the one in use.
fn typing_layout(app: &AppHandle, settings: &Settings) -> Option<os_layout::OsLayout> {
    let hwnd = main_hwnd(app);
    settings.key_layout.as_deref().and_then(|id| os_layout::read(hwnd, Some(id))).or_else(|| os_layout::read(hwnd, None))
}

/// The model's keys with the legends of the user's own keyboard: characters as the Windows
/// layout types them, named keys (Enter, Shift…) in the interface language. Keys neither knows
/// keep the model's legend.
fn labeled(m: &Model, base: &Model, os: Option<&os_layout::OsLayout>) -> Layout {
    let mut layout = Layout::from(m);
    layout.verified_as = (base.verified && !m.is_format()).then(|| base.standard.clone());
    for key in &mut layout.keys {
        let legend = os.and_then(|os| os.legend(&key.id)).or_else(|| i18n::lookup(&format!("keycap.{}", key.id)));
        if let Some(legend) = legend {
            key.label = legend;
        }
    }
    layout.typing = os.cloned();
    layout
}

/// The main window, whose thread follows the keyboard layout the user switches to.
fn main_hwnd(app: &AppHandle) -> isize {
    #[cfg(windows)]
    if let Some(hwnd) = app.get_webview_window("main").and_then(|w| w.hwnd().ok()) {
        return hwnd.0 as isize;
    }
    let _ = app;
    0
}

/// What to draw: see `Backend::layout`.
#[tauri::command]
async fn layout(app: AppHandle) -> Result<Layout, String> {
    let settings = load_settings(&app);
    let os = typing_layout(&app, &settings);
    with_backend(app, move |b| {
        // The Windows layout may have changed since the last call, and with it the guessed shape.
        b.apply_standard(settings.key_format.as_deref(), os.as_ref());
        labeled(&b.layout, &b.base, os.as_ref())
    })
    .await
}

#[derive(Serialize)]
struct LayoutSummary {
    id: String,
    name: String,
    kind: &'static str,
    format: String,
    standard: String,
    keys: usize,
    verified: bool,
}

#[derive(Serialize)]
struct KeyboardLayoutState {
    /// "auto", "iso" or "ansi".
    format: String,
    /// "auto" or a Windows layout id (KLID).
    layout: String,
    /// The Windows layout in use for legends and text macros, and its id.
    layout_name: Option<String>,
    layout_id: Option<String>,
    /// Layouts installed in Windows.
    installed: Vec<os_layout::LayoutChoice>,
    /// The shape drawn ("iso" or "ansi") and the support level that follows from it.
    standard: String,
    support: &'static str,
    /// The shape "auto" would draw.
    auto_standard: String,
}

async fn keyboard_layout_state(app: AppHandle) -> Result<KeyboardLayoutState, String> {
    let settings = load_settings(&app);
    let os = typing_layout(&app, &settings);
    let (standard, support, auto_standard) = with_backend(app.clone(), {
        let (format, os) = (settings.key_format.clone(), os.clone());
        move |b| {
            b.apply_standard(format.as_deref(), os.as_ref());
            (b.layout.standard.clone(), b.support, b.guess_standard(os.as_ref()))
        }
    })
    .await?;
    Ok(KeyboardLayoutState {
        format: settings.key_format.unwrap_or_else(|| "auto".into()),
        layout: settings.key_layout.unwrap_or_else(|| "auto".into()),
        layout_name: os.as_ref().map(|o| o.name.clone()),
        layout_id: os.map(|o| o.id),
        installed: os_layout::installed(),
        standard,
        support,
        auto_standard,
    })
}

/// The keyboard's shape and the Windows layout its legends follow (Settings › Keyboard).
#[tauri::command]
async fn keyboard_layout(app: AppHandle) -> Result<KeyboardLayoutState, String> {
    keyboard_layout_state(app).await
}

/// Chooses the shape ("auto", "iso", "ansi") and/or the Windows layout ("auto" or a KLID from
/// `installed`). Only the drawing and text macros change: nothing is sent to the keyboard.
#[tauri::command]
async fn set_keyboard_layout(app: AppHandle, format: Option<String>, layout: Option<String>) -> Result<KeyboardLayoutState, String> {
    if let Some(f) = &format
        && !matches!(f.as_str(), "auto" | "iso" | "ansi")
    {
        return Err(tr("backend.unknown_key_format", &[("format", f)]));
    }
    if let Some(l) = &layout
        && l != "auto"
        && !os_layout::installed().iter().any(|c| c.id.eq_ignore_ascii_case(l))
    {
        return Err(tr("backend.unknown_key_layout", &[("layout", l)]));
    }
    update_settings(&app, |s| {
        if let Some(f) = format {
            s.key_format = (f != "auto").then_some(f);
        }
        if let Some(l) = layout {
            s.key_layout = (l != "auto").then(|| l.to_uppercase());
        }
    })?;
    let state = keyboard_layout_state(app).await?;
    log::info!("Tastiera: formato {} ({}), layout {} ({:?})", state.format, state.standard, state.layout, state.layout_name);
    Ok(state)
}

/// Every model the app knows and every format it can draw.
#[tauri::command]
fn list_layouts() -> Vec<LayoutSummary> {
    all_models()
        .iter()
        .chain(model::builtin_formats().iter())
        .map(|m| LayoutSummary {
            id: m.id.clone(),
            name: m.name.clone(),
            kind: if m.is_format() { "format" } else { "model" },
            format: m.format.clone(),
            standard: m.standard.clone(),
            keys: m.keys.len(),
            verified: m.verified,
        })
        .collect()
}

/// Chooses the format that draws a keyboard without a model (remembered). Only then can it change.
#[tauri::command]
async fn set_preview_format(app: AppHandle, id: String) -> Result<Layout, String> {
    let format = model::builtin_formats()
        .into_iter()
        .find(|f| f.id == id)
        .ok_or_else(|| tr("backend.unknown_format", &[("id", &id)]))?;
    update_settings(&app, |s| s.preview_format = Some(id))?;
    let os = typing_layout(&app, &load_settings(&app));
    with_backend(app, move |b| {
        if b.support != "unconfigured" {
            return Err(tr("backend.format_only_unconfigured", &[]));
        }
        b.layout = format;
        b.base = b.layout.clone();
        Ok(labeled(&b.layout, &b.base, os.as_ref()))
    })
    .await?
}

/// Uploads the full per-key color buffer. Returns how long the upload took, in ms.
#[tauri::command]
async fn send_colors(app: AppHandle, colors: Vec<[u8; 3]>, brightness: u8) -> Result<u64, String> {
    with_backend(app, move |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        let start = Instant::now();
        match dev.set_per_key(&colors, brightness) {
            Ok(()) => {
                let ms = start.elapsed().as_millis() as u64;
                log::debug!("Colori inviati in {ms} ms (luminosità {brightness})");
                Ok(ms)
            }
            Err(e) => {
                log::error!("Invio colori fallito, tastiera considerata scollegata: {e}");
                b.device = None;
                Err(e)
            }
        }
    })
    .await?
}

#[derive(Serialize)]
struct DeviceDescription {
    info: DeviceInfo,
    capabilities: Capabilities,
}

/// Model, driver and what the connected keyboard can do; `None` when disconnected.
#[tauri::command]
async fn device_info(app: AppHandle) -> Result<Option<DeviceDescription>, String> {
    with_backend(app, |b| {
        b.device.as_ref().map(|dev| DeviceDescription {
            info: dev.info().clone(),
            capabilities: dev.capabilities().clone(),
        })
    })
    .await
}

/// Switches the keyboard to a hardware effect.
#[tauri::command]
async fn set_effect(app: AppHandle, settings: EffectSettings) -> Result<(), String> {
    with_backend(app, move |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        if !dev.capabilities().effects.iter().any(|fx| fx.id == settings.effect) {
            return Err(tr("backend.effect_unsupported", &[("effect", &settings.effect)]));
        }
        match dev.set_effect(&settings) {
            Ok(()) => {
                log::info!("Effetto impostato: {settings:?}");
                Ok(())
            }
            Err(e) => {
                log::error!("Impostazione effetto fallita, tastiera considerata scollegata ({settings:?}): {e}");
                b.device = None;
                Err(e)
            }
        }
    })
    .await?
}

/// Current USB report rate in Hz. Read-only.
#[tauri::command]
async fn report_rate(app: AppHandle) -> Result<u16, String> {
    with_backend(app, |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        dev.report_rate()
    })
    .await?
}

/// Writes the USB report rate into the keyboard, which keeps it.
#[tauri::command]
async fn set_report_rate(app: AppHandle, hz: u16) -> Result<(), String> {
    with_backend(app, move |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        if !dev.capabilities().report_rates.contains(&hz) {
            return Err(tr("backend.report_rate_unsupported", &[("hz", &hz)]));
        }
        dev.set_report_rate(hz).inspect_err(|e| log::error!("Report rate non impostata ({hz} Hz): {e}"))
    })
    .await?
}

#[derive(Serialize)]
struct KeysOnKeyboard {
    config: KeyConfig,
    /// Factory function of every key, by slot.
    defaults: BTreeMap<usize, KeyAction>,
}

/// Key functions and macros as the keyboard has them. Read-only.
#[tauri::command]
async fn read_keys(app: AppHandle) -> Result<KeysOnKeyboard, String> {
    with_backend(app, |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        let config = dev.read_keys().inspect_err(|e| log::error!("Lettura tasti fallita: {e}"))?;
        Ok(KeysOnKeyboard { config, defaults: dev.default_keys() })
    })
    .await?
}

/// Writes key functions and macros into the keyboard. The very first time, the keyboard's
/// keymap and macro memory are saved to `backup` in the data folder.
#[tauri::command]
async fn write_keys(app: AppHandle, config: KeyConfig) -> Result<(), String> {
    let backup_dir = data_dir(&app)?.join("backup");
    with_backend(app, move |b| {
        let Some(dev) = &b.device else {
            return Err(tr("backend.keyboard_not_connected", &[]));
        };
        let has_backup = std::fs::read_dir(&backup_dir).is_ok_and(|mut d| d.next().is_some());
        if !has_backup {
            let stamp = unix_seconds();
            std::fs::create_dir_all(&backup_dir)
                .map_err(|e| tr("backend.create_dir_failed", &[("path", &backup_dir.display()), ("error", &e)]))?;
            for (name, data) in dev.backup_keys()? {
                let path = backup_dir.join(format!("{name}-{stamp}.bin"));
                std::fs::write(&path, data)
                    .map_err(|e| tr("backend.file_save_failed", &[("path", &path.display()), ("error", &e)]))?;
                log::info!("Backup dei tasti: {}", path.display());
            }
        }
        dev.write_keys(&config).inspect_err(|e| log::error!("Scrittura tasti fallita: {e}"))
    })
    .await?
}

/// Seconds since 1970, to name backup files.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

/// Saved lighting setups ("My themes"), in `themes.json`. Profiles copy their lighting from these.
#[derive(Serialize, Deserialize, Default)]
struct ThemeStore {
    #[serde(default)]
    themes: Vec<Theme>,
}

#[derive(Serialize, Deserialize)]
struct Theme {
    id: String,
    name: String,
    background: [u8; 3],
    /// Per-key colors over the background, by slot.
    #[serde(default)]
    overrides: BTreeMap<usize, [u8; 3]>,
    /// 0..100
    #[serde(default = "full_intensity")]
    intensity: u8,
    /// "per_key" or "effect"; missing in themes saved before effects existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    /// Hardware effect setup (ui/src/lib/api.ts `EffectState`), kept as the UI wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    effect: Option<Value>,
}

fn full_intensity() -> u8 {
    100
}

// ---- Storage ----

/// `data` folder next to the executable, when it is writable: the app is portable and keeps
/// profiles, settings and logs there. Resolved once in `main`.
static PORTABLE_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

fn find_portable_dir() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("data");
    std::fs::create_dir_all(&dir).ok()?;
    let probe = dir.join(".scrivibile");
    std::fs::write(&probe, b"").ok()?;
    let _ = std::fs::remove_file(&probe);
    Some(dir)
}

fn portable_dir() -> Option<&'static PathBuf> {
    PORTABLE_DIR.get_or_init(find_portable_dir).as_ref()
}

/// Profiles and settings: the portable folder, or the OS config dir if it is read-only.
fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = match portable_dir() {
        Some(dir) => dir.clone(),
        None => app.path().app_config_dir().map_err(|e| e.to_string())?,
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| tr("backend.create_dir_failed", &[("path", &dir.display()), ("error", &e)]))?;
    Ok(dir)
}

fn themes_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("themes.json"))
}

/// Reads a JSON file the user may have edited: Notepad and PowerShell can start it with a byte
/// order mark, which serde_json refuses.
fn read_json_text(path: &std::path::Path) -> std::io::Result<String> {
    let text = std::fs::read_to_string(path)?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    })
}

/// Writes through a temp file, so a crash mid-write can't corrupt the old copy.
fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path))
}

#[tauri::command]
fn load_themes(app: AppHandle) -> Result<ThemeStore, String> {
    let path = themes_path(&app)?;
    let text = match read_json_text(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("Nessun file temi in {}: si parte dai preimpostati", path.display());
            return Ok(ThemeStore::default());
        }
        Err(e) => {
            log::error!("Lettura di {} fallita: {e}", path.display());
            return Err(tr("backend.themes_read_failed", &[("error", &e)]));
        }
    };
    match serde_json::from_str::<ThemeStore>(&text) {
        Ok(store) => {
            log::info!("Caricati {} temi da {}", store.themes.len(), path.display());
            Ok(store)
        }
        Err(e) => {
            // Keep the unreadable file aside instead of overwriting it on the next save.
            let backup = path.with_extension("json.corrotto");
            let _ = std::fs::rename(&path, &backup);
            log::error!("File temi non valido ({e}), spostato in {}", backup.display());
            Err(tr("backend.themes_invalid", &[("error", &e), ("path", &backup.display())]))
        }
    }
}

#[tauri::command]
fn save_themes(app: AppHandle, store: ThemeStore) -> Result<(), String> {
    let path = themes_path(&app)?;
    let text = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    match write_atomic(&path, &text) {
        Ok(()) => {
            log::debug!("Salvati {} temi in {}", store.themes.len(), path.display());
            Ok(())
        }
        Err(e) => {
            log::error!("Salvataggio di {} fallito: {e}", path.display());
            Err(tr("backend.themes_save_failed", &[("error", &e)]))
        }
    }
}

// ---- Profiles ----
// A profile is lighting + key functions + macros (ui/src/lib/api.ts `UserProfile`), one file
// per profile in `profiles/`, kept as the UI wrote it. The active one is in the settings.

#[derive(Serialize)]
struct ProfilesOnDisk {
    profiles: Vec<Value>,
    active: Option<String>,
}

fn profiles_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = data_dir(app)?.join("profiles");
    std::fs::create_dir_all(&dir)
        .map_err(|e| tr("backend.create_dir_failed", &[("path", &dir.display()), ("error", &e)]))?;
    Ok(dir)
}

/// File of a profile; ids are UUIDs, anything else could point outside the folder.
fn profile_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(tr("backend.invalid_profile_id", &[("id", &id)]));
    }
    Ok(profiles_dir(app)?.join(format!("{id}.json")))
}

#[tauri::command]
fn load_user_profiles(app: AppHandle) -> Result<ProfilesOnDisk, String> {
    let dir = profiles_dir(&app)?;
    let entries = std::fs::read_dir(&dir).map_err(|e| tr("backend.profiles_read_failed", &[("error", &e)]))?;
    let mut profiles = Vec::new();
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let parsed = read_json_text(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()));
        match parsed {
            Ok(profile) if profile.get("id").and_then(Value::as_str).is_some() => profiles.push(profile),
            Ok(_) => log::error!("Profilo senza id ignorato: {}", path.display()),
            Err(e) => {
                // Same as themes: set aside, never overwritten.
                let backup = path.with_extension("json.corrotto");
                let _ = std::fs::rename(&path, &backup);
                log::error!("Profilo non valido ({e}), spostato in {}", backup.display());
            }
        }
    }
    // Oldest first: the order they were made in.
    profiles.sort_by_key(|p| p.get("created").and_then(Value::as_u64).unwrap_or_default());
    let active = load_settings(&app).active_profile;
    log::info!(
        "Caricati {} profili da {} (attivo: {})",
        profiles.len(),
        dir.display(),
        active.as_deref().unwrap_or("nessuno")
    );
    Ok(ProfilesOnDisk { profiles, active })
}

#[tauri::command]
fn save_user_profile(app: AppHandle, profile: Value) -> Result<(), String> {
    let id = profile.get("id").and_then(Value::as_str).unwrap_or_default();
    let path = profile_path(&app, id)?;
    let text = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    write_atomic(&path, &text).map_err(|e| {
        log::error!("Salvataggio di {} fallito: {e}", path.display());
        tr("backend.profile_save_failed", &[("error", &e)])
    })
}

#[tauri::command]
fn delete_user_profile(app: AppHandle, id: String) -> Result<(), String> {
    let path = profile_path(&app, &id)?;
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(tr("backend.profile_delete_failed", &[("error", &e)])),
        _ => {
            log::info!("Profilo eliminato: {}", path.display());
            Ok(())
        }
    }
}

#[tauri::command]
fn set_active_profile(app: AppHandle, id: String) -> Result<(), String> {
    update_settings(&app, |s| s.active_profile = Some(id))
}

// ---- Settings ----

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    #[serde(default)]
    mcp_enabled: bool,
    /// Logs every HID packet (trace level) instead of info and above.
    #[serde(default)]
    verbose_log: bool,
    /// Interface language: a file name in `lang/` without `.ini`, or "auto" for the system's.
    #[serde(default = "auto_language")]
    language: String,
    /// Closing the window keeps the app running in the tray. Missing fields take `Default`.
    close_to_tray: bool,
    /// "auto" (follows the system), "light" or "dark".
    appearance: String,
    /// Case and keycap colors of the keyboard preview (ui/src/lib/look.svelte.ts); `None` = black.
    keyboard_look: Option<Value>,
    /// Id of the profile in use (see `load_user_profiles`).
    active_profile: Option<String>,
    /// Format drawn for a keyboard without a model (`list_layouts`).
    preview_format: Option<String>,
    /// Shape of the keyboard: "iso" or "ansi"; `None` guesses it from the Windows layout.
    key_format: Option<String>,
    /// Windows keyboard layout (KLID) the legends and text macros follow; `None` = the one in use.
    key_layout: Option<String>,
}

fn auto_language() -> String {
    i18n::AUTO.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self { mcp_enabled: false, verbose_log: false, language: auto_language(), close_to_tray: true, appearance: "auto".into(), keyboard_look: None, active_profile: None, preview_format: None, key_format: None, key_layout: None }
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("settings.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    let Ok(path) = settings_path(app) else { return Settings::default() };
    let Ok(text) = read_json_text(&path) else { return Settings::default() };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        // Set aside like themes and profiles, so the next save doesn't overwrite it.
        let backup = path.with_extension("json.corrotto");
        let _ = std::fs::rename(&path, &backup);
        log::warn!("Impostazioni non valide ({e}), spostate in {}: uso i valori predefiniti", backup.display());
        Settings::default()
    })
}

/// Held while settings are read, changed and written, so two changes at once (e.g. a profile
/// switch and the appearance) can't overwrite each other.
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

/// Changes the settings with `change` and saves them.
fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut settings = load_settings(app);
    change(&mut settings);
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    write_atomic(&settings_path(app)?, &text).map_err(|e| {
        log::error!("Salvataggio impostazioni fallito: {e}");
        tr("backend.settings_save_failed", &[("error", &e)])
    })
}

// ---- Logging ----

/// Base name of the log file in the app log dir; rotated files get a date suffix.
const LOG_NAME: &str = "lucent";

fn log_dir(app: &AppHandle) -> Result<PathBuf, String> {
    match portable_dir() {
        Some(dir) => Ok(dir.join("logs")),
        None => app.path().app_log_dir().map_err(|e| e.to_string()),
    }
}

fn apply_log_level(verbose: bool) {
    log::set_max_level(if verbose { log::LevelFilter::Trace } else { log::LevelFilter::Info });
}

#[derive(Serialize)]
struct LogStatus {
    verbose: bool,
    path: String,
}

fn log_status_of(app: &AppHandle, verbose: bool) -> Result<LogStatus, String> {
    let path = log_dir(app)?.join(format!("{LOG_NAME}.log"));
    Ok(LogStatus { verbose, path: path.display().to_string() })
}

#[tauri::command]
fn log_status(app: AppHandle) -> Result<LogStatus, String> {
    log_status_of(&app, load_settings(&app).verbose_log)
}

#[tauri::command]
fn set_verbose_log(app: AppHandle, enabled: bool) -> Result<LogStatus, String> {
    update_settings(&app, |s| s.verbose_log = enabled)?;
    apply_log_level(enabled);
    log::info!("Log dettagliato {}", if enabled { "attivato" } else { "disattivato" });
    log_status_of(&app, enabled)
}

#[tauri::command]
fn open_log_dir(app: AppHandle) -> Result<(), String> {
    let dir = log_dir(&app)?;
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| tr("backend.open_dir_failed", &[("path", &dir.display()), ("error", &e)]))
}

/// Saves a file made by the UI (e.g. the key template) into `esportazioni` in the data folder and
/// shows it in the file manager. Returns its path.
#[tauri::command]
fn save_export(app: AppHandle, name: String, data: Vec<u8>, reveal: Option<bool>) -> Result<String, String> {
    let file_name = std::path::Path::new(&name)
        .file_name()
        .ok_or_else(|| tr("backend.invalid_file_name", &[("name", &name)]))?;
    let dir = data_dir(&app)?.join("esportazioni");
    std::fs::create_dir_all(&dir)
        .map_err(|e| tr("backend.create_dir_failed", &[("path", &dir.display()), ("error", &e)]))?;
    let path = dir.join(file_name);
    std::fs::write(&path, data)
        .map_err(|e| tr("backend.file_save_failed", &[("path", &path.display()), ("error", &e)]))?;
    log::info!("Esportato {}", path.display());
    if reveal.unwrap_or(true) {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
        #[cfg(not(target_os = "windows"))]
        let _ = open_dir(&dir);
    }
    Ok(path.display().to_string())
}

#[cfg(not(target_os = "windows"))]
fn open_dir(dir: &std::path::Path) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(target_os = "macos"))]
    let opener = "xdg-open";
    std::process::Command::new(opener).arg(dir).spawn()
}

/// Last `lines` lines of the current log file (used by the MCP `get_log` tool).
fn log_tail(app: &AppHandle, lines: usize) -> Result<String, String> {
    let path = log_dir(app)?.join(format!("{LOG_NAME}.log"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let all: Vec<&str> = text.lines().collect();
    Ok(all[all.len().saturating_sub(lines)..].join("\n"))
}

// ---- Language ----

#[derive(Serialize)]
struct LanguageState {
    /// What the user chose: "auto" or a language code.
    setting: String,
    /// The language in use.
    active: String,
    languages: Vec<i18n::Language>,
    /// Every string of the active language, English where it has none.
    strings: BTreeMap<String, String>,
    /// English strings, for MCP answers.
    english: BTreeMap<String, String>,
    /// Folder holding the `.ini` files.
    folder: Option<String>,
}

fn language_state(setting: String, system: &str) -> LanguageState {
    let languages = i18n::languages();
    let active = i18n::resolve(&setting, system, &languages);
    let strings = i18n::strings(&active);
    i18n::set_active(strings.clone());
    LanguageState {
        setting,
        active,
        languages,
        strings,
        english: i18n::strings(i18n::FALLBACK),
        folder: i18n::lang_dir().map(|d| d.display().to_string()),
    }
}

/// Loads the chosen language; `system` is the system language (e.g. "it-IT") for "auto".
#[tauri::command]
fn language(app: AppHandle, system: String) -> LanguageState {
    let state = language_state(load_settings(&app).language, &system);
    log::info!("Lingua: {} (impostazione: {}, sistema: {system})", state.active, state.setting);
    state
}

#[tauri::command]
fn set_language(app: AppHandle, language: String, system: String) -> Result<LanguageState, String> {
    let language = language.trim().to_lowercase();
    if language != i18n::AUTO && !i18n::languages().iter().any(|l| l.code == language) {
        return Err(tr("backend.unknown_language", &[("language", &language)]));
    }
    update_settings(&app, |s| s.language = language.clone())?;
    let state = language_state(language, &system);
    log::info!("Lingua cambiata: {} (impostazione: {})", state.active, state.setting);
    Ok(state)
}

#[tauri::command]
fn open_lang_dir() -> Result<(), String> {
    let dir = i18n::lang_dir().ok_or_else(|| tr("backend.lang_dir_unavailable", &[]))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| tr("backend.create_dir_failed", &[("path", &dir.display()), ("error", &e)]))?;
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| tr("backend.open_dir_failed", &[("path", &dir.display()), ("error", &e)]))
}

// ---- Appearance ----

#[tauri::command]
fn appearance(app: AppHandle) -> String {
    load_settings(&app).appearance
}

#[tauri::command]
fn set_appearance(app: AppHandle, appearance: String) -> Result<String, String> {
    if !["auto", "light", "dark"].contains(&appearance.as_str()) {
        return Err(tr("backend.unknown_appearance", &[("appearance", &appearance)]));
    }
    update_settings(&app, |s| s.appearance = appearance.clone())?;
    log::info!("Aspetto: {appearance}");
    Ok(appearance)
}

#[tauri::command]
fn keyboard_look(app: AppHandle) -> Option<Value> {
    load_settings(&app).keyboard_look
}

#[tauri::command]
fn set_keyboard_look(app: AppHandle, look: Value) -> Result<(), String> {
    update_settings(&app, |s| s.keyboard_look = Some(look))
}

// ---- Startup and closing ----

#[derive(Serialize)]
struct StartupOptions {
    /// Starts with Windows, in the tray.
    autostart: bool,
    close_to_tray: bool,
    /// False where start with Windows is not available.
    autostart_supported: bool,
}

fn startup_options_of(app: &AppHandle) -> StartupOptions {
    StartupOptions {
        autostart: desktop::autostart_enabled(),
        close_to_tray: load_settings(app).close_to_tray,
        autostart_supported: cfg!(windows),
    }
}

#[tauri::command]
fn startup_options(app: AppHandle) -> StartupOptions {
    startup_options_of(&app)
}

/// Changes the given options; `None` leaves one as it is.
#[tauri::command]
fn set_startup_options(
    app: AppHandle,
    autostart: Option<bool>,
    close_to_tray: Option<bool>,
) -> Result<StartupOptions, String> {
    if let Some(enabled) = autostart {
        desktop::set_autostart(enabled)?;
    }
    if let Some(enabled) = close_to_tray {
        update_settings(&app, |s| s.close_to_tray = enabled)?;
        log::info!("Chiusura nella tray {}", if enabled { "attivata" } else { "disattivata" });
    }
    Ok(startup_options_of(&app))
}

// ---- MCP ----

struct McpState {
    bridge: Arc<mcp::Bridge>,
    /// Cancels the running server; `None` when stopped.
    server: tokio::sync::Mutex<Option<CancellationToken>>,
    last_error: Mutex<Option<String>>,
}

#[derive(Serialize)]
struct McpStatus {
    enabled: bool,
    running: bool,
    url: String,
    error: Option<String>,
}

async fn mcp_status_of(state: &McpState, enabled: bool) -> McpStatus {
    McpStatus {
        enabled,
        running: state.server.lock().await.is_some(),
        url: mcp::url(),
        error: state.last_error.lock().unwrap().clone(),
    }
}

async fn mcp_apply(state: &McpState, enabled: bool) {
    let mut server = state.server.lock().await;
    match (enabled, server.is_some()) {
        (true, false) => match mcp::start(state.bridge.clone()).await {
            Ok(token) => {
                log::info!("Server MCP attivo su {}", mcp::url());
                *server = Some(token);
                *state.last_error.lock().unwrap() = None;
            }
            Err(e) => {
                log::warn!("Server MCP non avviato: {e}");
                *state.last_error.lock().unwrap() = Some(e);
            }
        },
        (false, true) => {
            if let Some(token) = server.take() {
                token.cancel();
                log::info!("Server MCP fermato");
            }
        }
        _ => {}
    }
}

#[tauri::command]
async fn mcp_status(app: AppHandle) -> Result<McpStatus, String> {
    let enabled = load_settings(&app).mcp_enabled;
    Ok(mcp_status_of(&app.state::<McpState>(), enabled).await)
}

#[tauri::command]
async fn mcp_set_enabled(app: AppHandle, enabled: bool) -> Result<McpStatus, String> {
    update_settings(&app, |s| s.mcp_enabled = enabled)?;
    let state = app.state::<McpState>();
    mcp_apply(&state, enabled).await;
    Ok(mcp_status_of(&state, enabled).await)
}

/// The UI's answer to an `mcp-call` event.
#[tauri::command]
fn mcp_reply(app: AppHandle, id: u64, ok: Option<Value>, error: Option<String>) {
    let result = match error {
        Some(e) => Err(e),
        None => Ok(ok.unwrap_or(Value::Null)),
    };
    app.state::<McpState>().bridge.reply(id, result);
}

fn main() {
    tauri::Builder::default()
        // Opening the app again shows the running one, which may be hidden in the tray.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("App riaperta mentre era già attiva: mostro la finestra");
            desktop::show_main(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                // Everything is collected; `apply_log_level` picks what is actually written.
                .level(log::LevelFilter::Trace)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .level_for("hyper", log::LevelFilter::Info)
                .level_for("hyper_util", log::LevelFilter::Info)
                // The MCP library logs every message it handles; our own MCP lines are enough.
                .level_for("rmcp", log::LevelFilter::Warn)
                .level_for("tracing", log::LevelFilter::Warn)
                .targets([
                    Target::new(match portable_dir() {
                        Some(dir) => TargetKind::Folder { path: dir.join("logs"), file_name: Some(LOG_NAME.into()) },
                        None => TargetKind::LogDir { file_name: Some(LOG_NAME.into()) },
                    }),
                    Target::new(TargetKind::Stdout),
                ])
                .max_file_size(2_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(5))
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .manage(Mutex::new(Backend::default()))
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = load_settings(&handle);
            apply_log_level(settings.verbose_log);
            log::info!(
                "Lucent {} avviato ({} {})",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            match data_dir(&handle) {
                Ok(dir) if portable_dir().is_some() => log::info!("Cartella dati (portatile): {}", dir.display()),
                Ok(dir) => log::info!("Cartella dati (di sistema, accanto all'exe non si può scrivere): {}", dir.display()),
                Err(e) => log::error!("Cartella dati non disponibile: {e}"),
            }
            if let Ok(dir) = log_dir(&handle) {
                log::info!("Cartella log: {}", dir.display());
            }
            i18n::install_builtin();
            desktop::refresh_autostart();
            let tray = desktop::create_tray(&handle);
            if let Err(e) = &tray {
                log::error!("Icona nella tray non creata: {e}");
            }
            // Without a tray icon a hidden window could not be reopened.
            if desktop::started_by_autostart() && tray.is_ok() {
                log::info!("Avviato con Windows: resto nella tray");
            } else {
                desktop::show_main(&handle);
            }
            app.manage(McpState {
                bridge: Arc::new(mcp::Bridge::new(handle.clone())),
                server: tokio::sync::Mutex::new(None),
                last_error: Mutex::new(None),
            });
            if settings.mcp_enabled {
                // The port may still be held by an instance that is closing: retry for ~10 s.
                tauri::async_runtime::spawn(async move {
                    let state = handle.state::<McpState>();
                    for _ in 0..20 {
                        mcp_apply(&state, true).await;
                        if state.server.lock().await.is_some() {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            check_keyboard,
            layout,
            send_colors,
            device_info,
            set_effect,
            report_rate,
            set_report_rate,
            read_keys,
            write_keys,
            list_layouts,
            set_preview_format,
            keyboard_layout,
            set_keyboard_layout,
            load_themes,
            save_themes,
            load_user_profiles,
            save_user_profile,
            delete_user_profile,
            set_active_profile,
            mcp_status,
            mcp_set_enabled,
            mcp_reply,
            log_status,
            set_verbose_log,
            open_log_dir,
            save_export,
            language,
            set_language,
            open_lang_dir,
            startup_options,
            set_startup_options,
            appearance,
            set_appearance,
            keyboard_look,
            set_keyboard_look,
            desktop::set_tray_menu
        ])
        .run(tauri::generate_context!())
        .expect("errore avvio Lucent");
}
