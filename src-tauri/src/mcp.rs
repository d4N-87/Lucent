//! Local MCP server so AI agents can drive the app.
//!
//! Tool calls are not executed here: each one is forwarded to the UI as an `mcp-call`
//! event and answered through the `mcp_reply` command. The UI runs it through the same
//! code as the editor, so AI changes show up live, can be undone and are saved in the
//! active profile. Only loopback hosts are accepted (rmcp's default, against DNS rebinding).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub const PORT: u16 = 46327;
/// How long a tool call may wait for the UI (an upload to the keyboard takes ~150 ms).
const CALL_TIMEOUT: Duration = Duration::from_secs(15);

pub fn url() -> String {
    format!("http://127.0.0.1:{PORT}/mcp")
}

/// Forwards tool calls to the UI and routes its replies back to the waiting call.
pub struct Bridge {
    app: AppHandle,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
}

impl Bridge {
    pub fn new(app: AppHandle) -> Self {
        Self { app, next_id: AtomicU64::new(1), pending: Mutex::new(HashMap::new()) }
    }

    async fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = self.app.emit("mcp-call", json!({ "id": id, "tool": tool, "args": args })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(format!("Could not reach the app window: {e}"));
        }
        match tokio::time::timeout(CALL_TIMEOUT, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("The app dropped the request".into()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err("The app did not answer in time".into())
            }
        }
    }

    pub fn reply(&self, id: u64, result: Result<Value, String>) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
            let _ = tx.send(result);
        }
    }
}

// ---- Tool arguments ----

/// Keys can be named individually and/or by group; see `list_keys`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct KeySelection {
    /// Key ids such as "W", "Esc", "F1", "Space", "ShiftL", "Num7" (case-insensitive).
    #[serde(default)]
    pub keys: Vec<String>,
    /// Group ids: wasd, letters, numbers, fkeys, arrows, modifiers, navigation, numpad, all.
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Assignment {
    #[serde(flatten)]
    pub target: KeySelection,
    /// Hex color "#RRGGBB". Use "#000000" to turn keys off.
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PaintArgs {
    /// Applied in order, so later assignments win on overlapping keys.
    pub assignments: Vec<Assignment>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ColorArg {
    /// Hex color "#RRGGBB".
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EffectArgs {
    /// Effect id from get_device_info (e.g. "wave", "breathing"), or its name.
    pub effect: String,
    /// 0 to speed_levels - 1. Omit to keep the current speed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<u8>,
    /// "left" or "right", for effects that move. Omit to keep the current one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Hex color like "#FF3B30"; also turns random colors off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Let the keyboard pick the colors (effects with color "single_or_random").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ImageArgs {
    /// Absolute path of a PNG, JPEG, GIF, WebP or BMP file on this PC.
    pub path: String,
    /// 1 = largest crop with the keyboard's proportions (default); up to 8 zooms in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f64>,
    /// Center of the crop as a fraction of the picture, 0..1 (default 0.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_y: Option<f64>,
    /// 0..1 saturation boost, since photos look dull on LEDs (default 0.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vividness: Option<f64>,
}

/// Pictures larger than this are refused: they would only slow the app down.
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LightingModeArgs {
    /// "per_key" (colors painted per key) or "effect" (the current hardware effect).
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LogArgs {
    /// How many of the most recent lines to return (default 80, max 1000).
    pub lines: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct VerboseLogArgs {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LanguageArgs {
    /// A language code from list_languages (e.g. "it", "en"), or "auto" to follow the system.
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct KeyFunction {
    /// "default" (factory), "key", "media", "macro" or "disabled".
    #[serde(rename = "type")]
    pub kind: String,
    /// For "key": the key id it should send, from list_keys (e.g. "Esc").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// For "media": play_pause, previous, next, stop, mute, volume_down, volume_up, calculator,
    /// browser, mail, computer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<String>,
    /// For "macro": the macro's name (see get_key_functions, save_macro).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetKeyFunctionArgs {
    /// Key id from list_keys (e.g. "CapsLock", "Pause").
    pub key: String,
    pub function: KeyFunction,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroStepArg {
    /// Key id from list_keys.
    pub key: String,
    /// true = key goes down, false = key goes up. Every key pressed should also be released.
    pub pressed: bool,
    /// Wait after this step, in ms (rounded to the keyboard's step; default 50).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveMacroArgs {
    /// A macro with the same name is replaced (keys playing it keep playing it).
    pub name: String,
    /// Times it plays on each press, 1-99 (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u8>,
    /// The key events, in order. Either steps or text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<MacroStepArg>>,
    /// Text to type with the Windows keyboard layout in use (see get_keyboard_layout): any character
    /// that layout has on a key, with Shift and/or AltGr, plus new lines and tabs. Shift and AltGr
    /// steps are added as needed; an accent alone on a dead key is followed by Space.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MacroRef {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct KeyboardLookArgs {
    /// Case color of the preview: "black", "white", "gray" or a hex color. Omit to keep it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    /// Keycap color of the preview: "black", "white", "gray" or a hex color. Omit to keep it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AppearanceArgs {
    /// "auto" (follows the system), "light" or "dark". The keyboard preview stays dark.
    pub appearance: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReportRateArgs {
    /// One of report_rates_hz from get_device_info (on the GMMK v1: 125, 250, 500, 1000).
    pub hz: u16,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StartupArgs {
    /// Start the app with Windows, hidden in the tray (notification area).
    pub autostart: Option<bool>,
    /// Closing the window keeps the app running in the tray instead of quitting.
    pub close_to_tray: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct IntensityArgs {
    /// Overall brightness, 0-100.
    pub percent: u8,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GradientArgs {
    /// Two or more hex colors, from left to right (or top to bottom).
    pub colors: Vec<String>,
    /// Defaults to horizontal.
    pub direction: Option<Direction>,
    /// Keys to apply it to; all keys when empty.
    #[serde(flatten, default)]
    pub target: KeySelection,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Harmony {
    Complementary,
    Analogous,
    Triadic,
    Split,
    Tetradic,
    Mono,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HarmonyMode {
    /// Background, letters, numbers + F keys, modifiers, and arrows/navigation/numpad each get one color.
    Groups,
    /// Left-to-right gradient through the palette.
    Gradient,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HarmonyArgs {
    /// Hex color the palette is built from.
    pub base_color: String,
    pub harmony: Harmony,
    pub mode: HarmonyMode,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ThemeRef {
    /// Theme id or name (presets included, e.g. "Synthwave").
    pub theme: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserThemeRef {
    /// Id or name of a user theme (presets can't be changed).
    pub theme: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SaveThemeArgs {
    /// Save as a new theme with this name. Without a name, updates the user theme the colors come from.
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenameThemeArgs {
    /// Id or name of a user theme.
    pub theme: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProfileRef {
    /// Profile id or name.
    pub profile: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreateProfileArgs {
    pub name: String,
    /// Profile id or name to copy (colors, keys and macros). Without it the new profile has factory
    /// keys, no macros and the current colors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_of: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FormatArg {
    /// Format id or name from list_models, e.g. "format_tkl_ansi" or "TKL ANSI".
    pub format: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct KeyboardLayoutArgs {
    /// Keyboard shape: "auto" (guessed from the Windows layout: US means ANSI), "iso" or "ansi".
    /// Leave out to keep it.
    pub shape: Option<String>,
    /// Windows keyboard layout the legends and text macros follow: "auto" (the one in use), or an
    /// installed layout's id or name from get_keyboard_layout. Leave out to keep it.
    pub layout: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PathArg {
    /// Absolute path of a file on this PC.
    pub path: String,
}

/// Profile files are a few kB; anything this big is not one.
const MAX_PROFILE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenameProfileArgs {
    /// Profile id or name.
    pub profile: String,
    pub name: String,
}

// ---- Server ----

#[derive(Clone)]
pub struct LucentMcp {
    bridge: Arc<Bridge>,
}

impl LucentMcp {
    fn new(bridge: Arc<Bridge>) -> Self {
        Self { bridge }
    }

    async fn forward(&self, tool: &str, args: impl Serialize) -> Result<CallToolResult, McpError> {
        let args = serde_json::to_value(args).unwrap_or(Value::Null);
        log::info!("MCP ▸ {tool} {args}");
        Ok(match self.bridge.call(tool, args).await {
            Ok(value) => {
                let text = serde_json::to_string_pretty(&value).unwrap_or_default();
                CallToolResult::success(vec![ContentBlock::text(text)])
            }
            Err(e) => {
                log::warn!("MCP ▸ {tool} fallito: {e}");
                CallToolResult::error(vec![ContentBlock::text(e)])
            }
        })
    }
}

#[tool_router]
impl LucentMcp {
    #[tool(description = "Current state: connection, active profile, the theme its colors come from and \
        whether they were edited, background color, intensity, keys with their own color, undo/redo availability.")]
    async fn get_state(&self) -> Result<CallToolResult, McpError> {
        self.forward("get_state", json!({})).await
    }

    #[tool(description = "Keyboard model, every key (id, printed label, position) and the key groups.")]
    async fn list_keys(&self) -> Result<CallToolResult, McpError> {
        self.forward("list_keys", json!({})).await
    }

    #[tool(description = "Give keys their own color. Each assignment targets keys and/or groups. \
        Changes are live on the keyboard and saved in the active profile.")]
    async fn paint_keys(&self, Parameters(args): Parameters<PaintArgs>) -> Result<CallToolResult, McpError> {
        self.forward("paint_keys", args).await
    }

    #[tool(description = "Put keys back on the background color (removes their own color).")]
    async fn reset_keys(&self, Parameters(args): Parameters<KeySelection>) -> Result<CallToolResult, McpError> {
        self.forward("reset_keys", args).await
    }

    #[tool(description = "Set the background color, used by every key without its own color.")]
    async fn set_background(&self, Parameters(args): Parameters<ColorArg>) -> Result<CallToolResult, McpError> {
        self.forward("set_background", args).await
    }

    #[tool(description = "Set overall brightness, 0-100 %.")]
    async fn set_intensity(&self, Parameters(args): Parameters<IntensityArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_intensity", args).await
    }

    #[tool(description = "Color keys with a gradient through the given colors, following key positions.")]
    async fn apply_gradient(&self, Parameters(args): Parameters<GradientArgs>) -> Result<CallToolResult, McpError> {
        self.forward("apply_gradient", args).await
    }

    #[tool(description = "Build a color-harmony palette from a base color and apply it to key groups or as a gradient.")]
    async fn apply_harmony(&self, Parameters(args): Parameters<HarmonyArgs>) -> Result<CallToolResult, McpError> {
        self.forward("apply_harmony", args).await
    }

    #[tool(description = "List themes (saved lighting setups): the user's own and the built-in presets, \
        which one the active profile's colors come from, and whether they were edited since.")]
    async fn list_themes(&self) -> Result<CallToolResult, McpError> {
        self.forward("list_themes", json!({})).await
    }

    #[tool(description = "Put a theme's colors on the active profile and send them to the keyboard. \
        One undo step; the profile keeps its keys and macros.")]
    async fn apply_theme(&self, Parameters(args): Parameters<ThemeRef>) -> Result<CallToolResult, McpError> {
        self.forward("apply_theme", args).await
    }

    #[tool(description = "Save the current colors as a theme: a new one when a name is given (required \
        when they come from a preset), otherwise into the user theme they come from.")]
    async fn save_theme(&self, Parameters(args): Parameters<SaveThemeArgs>) -> Result<CallToolResult, McpError> {
        self.forward("save_theme", args).await
    }

    #[tool(description = "Put back the colors of the theme the active profile follows, dropping the edits \
        made since (undo brings them back).")]
    async fn revert_to_theme(&self) -> Result<CallToolResult, McpError> {
        self.forward("revert_to_theme", json!({})).await
    }

    #[tool(description = "Rename a user theme.")]
    async fn rename_theme(&self, Parameters(args): Parameters<RenameThemeArgs>) -> Result<CallToolResult, McpError> {
        self.forward("rename_theme", args).await
    }

    #[tool(description = "Permanently delete a user theme. Profiles using it keep their colors.")]
    async fn delete_theme(&self, Parameters(args): Parameters<UserThemeRef>) -> Result<CallToolResult, McpError> {
        self.forward("delete_theme", args).await
    }

    #[tool(description = "List profiles. A profile is what the keyboard does as a whole: colors (from a \
        theme, maybe edited), key functions and macros. Exactly one is active; it saves itself on every edit.")]
    async fn list_profiles(&self) -> Result<CallToolResult, McpError> {
        self.forward("list_profiles", json!({})).await
    }

    #[tool(description = "Make a profile the active one. Its colors are sent if they differ from what is on; \
        its keys and macros are written into the keyboard only if they differ from the keyboard's \
        (keys_written tells).")]
    async fn switch_profile(&self, Parameters(args): Parameters<ProfileRef>) -> Result<CallToolResult, McpError> {
        self.forward("switch_profile", args).await
    }

    #[tool(description = "Create a profile and make it active: a copy of copy_of, or factory keys with the \
        current colors. Factory keys are written into the keyboard if it has other keys.")]
    async fn create_profile(&self, Parameters(args): Parameters<CreateProfileArgs>) -> Result<CallToolResult, McpError> {
        self.forward("create_profile", args).await
    }

    #[tool(description = "Rename a profile.")]
    async fn rename_profile(&self, Parameters(args): Parameters<RenameProfileArgs>) -> Result<CallToolResult, McpError> {
        self.forward("rename_profile", args).await
    }

    #[tool(description = "Save a profile (colors, keys, macros) as a .lucent-profile.json file in the app's \
        exports folder, to keep or share. Returns the file's path.")]
    async fn export_profile(&self, Parameters(args): Parameters<ProfileRef>) -> Result<CallToolResult, McpError> {
        self.forward("export_profile", args).await
    }

    #[tool(description = "Add the profile in a .lucent-profile.json file (made by export_profile). It gets a \
        free name and does not become active, so nothing is written to the keyboard.")]
    async fn import_profile(&self, Parameters(args): Parameters<PathArg>) -> Result<CallToolResult, McpError> {
        let path = std::path::Path::new(&args.path);
        let text = match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_PROFILE_BYTES => Err("Not a profile file: too large".to_string()),
            Ok(_) => std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", args.path)),
            Err(e) => Err(format!("Cannot read {}: {e}", args.path)),
        };
        let text = match text {
            Ok(text) => text,
            Err(e) => return Ok(CallToolResult::error(vec![ContentBlock::text(e)])),
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.forward("import_profile", json!({ "name": name, "text": text })).await
    }

    #[tool(description = "Permanently delete a profile; the last one can't be deleted. Deleting the active \
        profile switches to another one, which may write its keys into the keyboard.")]
    async fn delete_profile(&self, Parameters(args): Parameters<ProfileRef>) -> Result<CallToolResult, McpError> {
        self.forward("delete_profile", args).await
    }

    #[tool(description = "Undo the last change.")]
    async fn undo(&self) -> Result<CallToolResult, McpError> {
        self.forward("undo", json!({})).await
    }

    #[tool(description = "Redo the last undone change.")]
    async fn redo(&self) -> Result<CallToolResult, McpError> {
        self.forward("redo", json!({})).await
    }

    #[tool(description = "Keyboard model, driver and capabilities: brightness levels, USB report rate \
        (report_rate_hz, report_rates_hz), and every hardware \
        effect with its id, name, speed levels, directions and color support (none, single, single_or_random). \
        Effects with verified=false have guessed parameters. support tells how far the app trusts the \
        keyboard: verified, unverified (a model from the models folder: key changes wait for the user's OK) \
        or unconfigured (no model: the app sends it nothing and only draws a preview).")]
    async fn get_device_info(&self) -> Result<CallToolResult, McpError> {
        self.forward("get_device_info", json!({})).await
    }

    #[tool(description = "Keyboard models the app knows (they drive real keyboards) and formats it can draw \
        (full-size, TKL, 75%, 65%, 60%, ANSI or ISO: only to preview a keyboard without a model).")]
    async fn list_models(&self) -> Result<CallToolResult, McpError> {
        self.forward("list_models", json!({})).await
    }

    #[tool(description = "Choose the format that draws a keyboard without a model (remembered). Only while \
        such a keyboard is connected; nothing is sent to it.")]
    async fn set_preview_format(&self, Parameters(args): Parameters<FormatArg>) -> Result<CallToolResult, McpError> {
        self.forward("set_preview_format", args).await
    }

    #[tool(description = "The keyboard's shape (ISO or ANSI) and the Windows keyboard layout its legends \
        and text macros follow (Italian, Spanish, US…), with the layouts installed in Windows. The model \
        was verified in one shape (verified_as); drawn in another, support is unverified and key writes \
        wait for the user's OK.")]
    async fn get_keyboard_layout(&self) -> Result<CallToolResult, McpError> {
        self.forward("get_keyboard_layout", json!({})).await
    }

    #[tool(description = "Choose the keyboard's shape (auto, iso, ansi) and/or the Windows layout its legends \
        and text macros follow (auto, or an installed layout from get_keyboard_layout). Only the drawing and \
        text macros change: nothing is sent to the keyboard.")]
    async fn set_keyboard_layout(&self, Parameters(args): Parameters<KeyboardLayoutArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_keyboard_layout", args).await
    }

    #[tool(description = "Set the keyboard's USB report rate (how often it reports keys to the PC). \
        The keyboard stores it. 1000 Hz is the most responsive; lower rates only help with hubs, KVM \
        switches or old PCs that have trouble. Current value and choices: report_rate_hz / report_rates_hz \
        in get_device_info.")]
    async fn set_report_rate(&self, Parameters(args): Parameters<ReportRateArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_report_rate", args).await
    }

    #[tool(description = "What the keys do: keys changed from the factory (another key, a media key, a \
        macro or disabled), the macros stored in the keyboard with their steps, and whether the keyboard has \
        the app's setup (on_keyboard).")]
    async fn get_key_functions(&self) -> Result<CallToolResult, McpError> {
        self.forward("get_key_functions", json!({})).await
    }

    #[tool(description = "Change what one key does. Written into the keyboard right away (it works without \
        the app too) and saved; undoable in the app. The Fn key can't be changed.")]
    async fn set_key_function(&self, Parameters(args): Parameters<SetKeyFunctionArgs>) -> Result<CallToolResult, McpError> {
        let mut value = serde_json::to_value(&args).unwrap_or(Value::Null);
        // The UI reads the macro name as `macro`.
        if let Some(f) = value.get_mut("function").and_then(Value::as_object_mut)
            && let Some(name) = f.remove("macro_name")
        {
            f.insert("macro".into(), name);
        }
        self.forward("set_key_function", value).await
    }

    #[tool(description = "Put keys back to their factory function (all keys when none are given). \
        Written into the keyboard.")]
    async fn reset_key_functions(&self, Parameters(args): Parameters<KeySelection>) -> Result<CallToolResult, McpError> {
        self.forward("reset_key_functions", args).await
    }

    #[tool(description = "Create or replace a macro stored in the keyboard, from key steps or from text. \
        Assign it to a key with set_key_function (type \"macro\").")]
    async fn save_macro(&self, Parameters(args): Parameters<SaveMacroArgs>) -> Result<CallToolResult, McpError> {
        self.forward("save_macro", args).await
    }

    #[tool(description = "Delete a macro. Keys playing it go back to their factory function.")]
    async fn delete_macro(&self, Parameters(args): Parameters<MacroRef>) -> Result<CallToolResult, McpError> {
        self.forward("delete_macro", args).await
    }

    #[tool(description = "Run a hardware effect on the keyboard (switches the lighting to effect mode). \
        Unspecified parameters keep their current value. Brightness follows set_intensity in steps of 25%. \
        Painting keys or setting the background switches back to per-key colors.")]
    async fn set_effect(&self, Parameters(args): Parameters<EffectArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_effect", args).await
    }

    #[tool(description = "Paint the keys from a picture file: the picture is cropped to the keyboard's \
        proportions and every key takes the average color under it (switches to per-key mode, one undo step).")]
    async fn apply_image(&self, Parameters(args): Parameters<ImageArgs>) -> Result<CallToolResult, McpError> {
        let path = std::path::Path::new(&args.path);
        let size = match std::fs::metadata(path) {
            Ok(meta) => meta.len(),
            Err(e) => return Ok(CallToolResult::error(vec![ContentBlock::text(format!("Cannot read {}: {e}", args.path))])),
        };
        if size > MAX_IMAGE_BYTES {
            return Ok(CallToolResult::error(vec![ContentBlock::text("Picture too large (max 25 MB)")]));
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => return Ok(CallToolResult::error(vec![ContentBlock::text(format!("Cannot read {}: {e}", args.path))])),
        };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.forward(
            "apply_image",
            json!({
                "name": name,
                "data": BASE64.encode(bytes),
                "zoom": args.zoom,
                "center_x": args.center_x,
                "center_y": args.center_y,
                "vividness": args.vividness,
            }),
        )
        .await
    }

    #[tool(description = "Switch between per-key colors and the hardware effect, keeping both setups.")]
    async fn set_lighting_mode(
        &self,
        Parameters(args): Parameters<LightingModeArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.forward("set_lighting_mode", args).await
    }

    #[tool(description = "Upload the current colors to the keyboard now (normally automatic).")]
    async fn send_to_keyboard(&self) -> Result<CallToolResult, McpError> {
        self.forward("send_to_keyboard", json!({})).await
    }

    #[tool(description = "Interface languages: the setting (\"auto\" follows the system), the language in use, \
        and every language found in the lang folder (one .ini file each). Tools always answer in English; \
        names shown to the user (profiles, presets, effects' ui_name) follow the interface language.")]
    async fn list_languages(&self) -> Result<CallToolResult, McpError> {
        self.forward("list_languages", json!({})).await
    }

    #[tool(description = "Case and keycap colors of the on-screen keyboard, to match the user's real one \
        (an app setting: the keyboard itself does not change). Current colors: preview_look in get_state.")]
    async fn set_keyboard_look(&self, Parameters(args): Parameters<KeyboardLookArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_keyboard_look", args).await
    }

    #[tool(description = "Light or dark interface (remembered); \"auto\" follows Windows. The current \
        one is ui_appearance in get_state.")]
    async fn set_appearance(&self, Parameters(args): Parameters<AppearanceArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_appearance", args).await
    }

    #[tool(description = "Change the interface language (remembered). Use a code from list_languages, or \"auto\".")]
    async fn set_language(&self, Parameters(args): Parameters<LanguageArgs>) -> Result<CallToolResult, McpError> {
        self.forward("set_language", args).await
    }

    #[tool(description = "Startup and closing options (remembered): start with Windows in the tray, and \
        whether closing the window keeps the app in the tray. Omitted options stay as they are; call with \
        no arguments to read them.")]
    async fn set_startup_options(&self, Parameters(args): Parameters<StartupArgs>) -> Result<CallToolResult, McpError> {
        log::info!("MCP ▸ set_startup_options {args:?}");
        Ok(match crate::set_startup_options(self.bridge.app.clone(), args.autostart, args.close_to_tray) {
            Ok(options) => CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&options).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        })
    }

    #[tool(description = "Last lines of the app log: startup paths, keyboard connection, uploads and their \
        timing, key writes, HID errors, failed saves, MCP calls. With verbose logging on, every HID packet too. \
        Read it first when something did not work.")]
    async fn get_log(&self, Parameters(args): Parameters<LogArgs>) -> Result<CallToolResult, McpError> {
        let lines = args.lines.unwrap_or(80).clamp(1, 1000);
        Ok(match crate::log_tail(&self.bridge.app, lines) {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        })
    }

    #[tool(description = "Turn verbose logging on or off (remembered). When on, every packet exchanged \
        with the keyboard is logged in hex (TX/RX). Turn it off when done: it makes the log grow fast.")]
    async fn set_verbose_logging(
        &self,
        Parameters(args): Parameters<VerboseLogArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(match crate::set_verbose_log(self.bridge.app.clone(), args.enabled) {
            Ok(status) => CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&status).unwrap_or_default(),
            )]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        })
    }
}

#[tool_handler]
impl ServerHandler for LucentMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("lucent", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Controls a GMMK keyboard through the Lucent app: its RGB lighting, what its keys \
                 do and the macros stored in it (get_key_functions, set_key_function, save_macro). \
                 Lighting is either per-key (a background color plus optional per-key colors) or a \
                 hardware effect the keyboard runs itself (get_device_info, set_effect), scaled by an intensity. \
                 Call list_keys once to learn key ids and groups, and get_state to see the current setup. \
                 Everything belongs to the active profile (colors, key functions, macros; list_profiles, \
                 switch_profile), which saves itself on every edit; edits show live and can be undone. \
                 Themes are saved colors a profile can take (list_themes, apply_theme, save_theme). \
                 Colors are hex strings like \"#FF3B30\". \
                 If something fails or looks wrong, read get_log.",
            )
    }
}

/// Starts the server on 127.0.0.1. Cancel the returned token to stop it.
pub async fn start(bridge: Arc<Bridge>) -> Result<CancellationToken, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", PORT))
        .await
        .map_err(|e| crate::i18n::tr("backend.mcp_port_unavailable", &[("port", &PORT), ("error", &e)]))?;
    let cancel = CancellationToken::new();
    let service = StreamableHttpService::new(
        move || Ok(LucentMcp::new(bridge.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_cancellation_token(cancel.child_token()),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let shutdown = cancel.clone();
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;
    });
    Ok(cancel)
}
