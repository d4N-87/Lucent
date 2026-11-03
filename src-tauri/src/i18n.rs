//! Interface languages: one `.ini` file per language in `lang/` next to the executable.
//!
//! Italian and English are built in and written to `lang/` at startup, so anyone can copy
//! `en.ini` to e.g. `es.ini` and translate it. A missing line falls back to English. The
//! language in use also translates the backend messages the UI shows (errors, status).

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const FALLBACK: &str = "en";
/// Setting value that follows the system language.
pub const AUTO: &str = "auto";

const BUILTIN: &[(&str, &str)] = &[("en", include_str!("../../lang/en.ini")), ("it", include_str!("../../lang/it.ini"))];

type Strings = BTreeMap<String, String>;

/// Strings of the language in use, for backend messages.
static ACTIVE: RwLock<Option<Strings>> = RwLock::new(None);

/// `key = value` lines grouped under `[section]` headers become `section.key`.
/// Lines starting with `;` or `#` are comments; `\n` in a value is a line break.
pub fn parse(text: &str) -> Strings {
    let mut out = Strings::new();
    let mut section = String::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        let full = if section.is_empty() { key.to_string() } else { format!("{section}.{key}") };
        out.insert(full, value.trim().replace("\\n", "\n"));
    }
    out
}

/// `lang` next to the executable.
pub fn lang_dir() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("lang"))
}

/// Writes the built-in languages into `lang/`, so they match this version of the app.
pub fn install_builtin() {
    let Some(dir) = lang_dir() else { return };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("Cartella lingue non creata ({e}): uso le lingue incluse");
        return;
    }
    for (code, text) in BUILTIN {
        let path = dir.join(format!("{code}.ini"));
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*text)
            && let Err(e) = std::fs::write(&path, text)
        {
            log::warn!("Scrittura di {} fallita: {e}", path.display());
        }
    }
}

fn read_file(dir: &Path, code: &str) -> Option<Strings> {
    let path = dir.join(format!("{code}.ini"));
    let text = std::fs::read_to_string(&path).ok()?;
    Some(parse(&text))
}

fn builtin(code: &str) -> Option<Strings> {
    BUILTIN.iter().find(|(c, _)| *c == code).map(|(_, text)| parse(text))
}

/// The language's own strings: its file in `lang/`, or the built-in copy.
fn own_strings(code: &str) -> Option<Strings> {
    let file = lang_dir().and_then(|dir| read_file(&dir, code));
    match (builtin(code), file) {
        (Some(mut base), Some(file)) => {
            base.extend(file);
            Some(base)
        }
        (base, file) => file.or(base),
    }
}

/// Every string of `code`, with English for the lines it lacks.
pub fn strings(code: &str) -> Strings {
    let mut out = own_strings(FALLBACK).unwrap_or_default();
    if code != FALLBACK {
        out.extend(own_strings(code).unwrap_or_default());
    }
    out
}

#[derive(Serialize, Clone)]
pub struct Language {
    /// File name without `.ini`, e.g. "it" or "pt-br".
    pub code: String,
    /// The language's own name, from `[meta] name`.
    pub name: String,
}

/// Built-in languages plus every `.ini` file in `lang/`, sorted by name.
pub fn languages() -> Vec<Language> {
    let mut codes: Vec<String> = BUILTIN.iter().map(|(c, _)| c.to_string()).collect();
    if let Some(entries) = lang_dir().and_then(|dir| std::fs::read_dir(dir).ok()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("ini"))
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                let code = stem.to_lowercase();
                if !codes.contains(&code) {
                    codes.push(code);
                }
            }
        }
    }
    let mut list: Vec<Language> = codes
        .into_iter()
        .map(|code| {
            let name = own_strings(&code)
                .and_then(|s| s.get("meta.name").cloned())
                .unwrap_or_else(|| code.clone());
            Language { code, name }
        })
        .collect();
    list.sort_by_key(|l| l.name.to_lowercase());
    list
}

/// The language to use: the chosen one if it exists, otherwise the system's
/// (e.g. "it-IT" → "it-it", then "it"), otherwise English.
pub fn resolve(setting: &str, system: &str, available: &[Language]) -> String {
    let has = |code: &str| available.iter().any(|l| l.code == code);
    let setting = setting.to_lowercase();
    if setting != AUTO && has(&setting) {
        return setting;
    }
    let system = system.to_lowercase().replace('_', "-");
    let primary = system.split('-').next().unwrap_or_default().to_string();
    [system, primary].into_iter().find(|c| has(c)).unwrap_or_else(|| FALLBACK.to_string())
}

pub fn set_active(strings: Strings) {
    *ACTIVE.write().unwrap() = Some(strings);
}

/// The text for `key` in the language in use (or English), if any language file has it.
pub fn lookup(key: &str) -> Option<String> {
    let guard = ACTIVE.read().unwrap();
    guard
        .as_ref()
        .and_then(|s| s.get(key).cloned())
        .or_else(|| builtin(FALLBACK).and_then(|s| s.get(key).cloned()))
}

/// Backend message in the language in use. `{name}` placeholders are filled from `args`.
pub fn tr(key: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let text = lookup(key).unwrap_or_else(|| key.to_string());
    args.iter().fold(text, |text, (name, value)| text.replace(&format!("{{{name}}}"), &value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_comments_and_breaks() {
        let s = parse("\u{feff}; note\n[app]\ntitle = Hi = there\n# c\nbody=a\\nb\n");
        assert_eq!(s["app.title"], "Hi = there");
        assert_eq!(s["app.body"], "a\nb");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn builtin_languages_have_the_same_keys() {
        let en = builtin("en").unwrap();
        let it = builtin("it").unwrap();
        let missing: Vec<_> = en.keys().filter(|k| !it.contains_key(*k)).collect();
        let extra: Vec<_> = it.keys().filter(|k| !en.contains_key(*k)).collect();
        assert!(missing.is_empty() && extra.is_empty(), "missing in it: {missing:?}, only in it: {extra:?}");
    }

    #[test]
    fn resolves_system_language() {
        let langs = vec![
            Language { code: "en".into(), name: "English".into() },
            Language { code: "it".into(), name: "Italiano".into() },
        ];
        assert_eq!(resolve("auto", "it-IT", &langs), "it");
        assert_eq!(resolve("auto", "de-DE", &langs), "en");
        assert_eq!(resolve("en", "it-IT", &langs), "en");
        assert_eq!(resolve("xx", "it", &langs), "it");
    }
}
