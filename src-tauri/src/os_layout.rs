//! The keyboard layout Windows types with (Italian, Spanish, US…). Lucent asks Windows what each
//! key writes, so the preview shows the legends of the user's own layout and text macros press
//! the keys that really type each character. Nothing here talks to the keyboard.

use serde::Serialize;
use std::collections::BTreeMap;

/// Keys that type a character, by model key id, with their scan code (set 1). `Backslash` (ANSI)
/// and `NonUsHash` (ISO) are the same key for Windows.
pub const CHAR_KEYS: &[(&str, u32)] = &[
    ("Grave", 0x29),
    ("1", 0x02),
    ("2", 0x03),
    ("3", 0x04),
    ("4", 0x05),
    ("5", 0x06),
    ("6", 0x07),
    ("7", 0x08),
    ("8", 0x09),
    ("9", 0x0A),
    ("0", 0x0B),
    ("Minus", 0x0C),
    ("Equal", 0x0D),
    ("Q", 0x10),
    ("W", 0x11),
    ("E", 0x12),
    ("R", 0x13),
    ("T", 0x14),
    ("Y", 0x15),
    ("U", 0x16),
    ("I", 0x17),
    ("O", 0x18),
    ("P", 0x19),
    ("LeftBracket", 0x1A),
    ("RightBracket", 0x1B),
    ("A", 0x1E),
    ("S", 0x1F),
    ("D", 0x20),
    ("F", 0x21),
    ("G", 0x22),
    ("H", 0x23),
    ("J", 0x24),
    ("K", 0x25),
    ("L", 0x26),
    ("Semicolon", 0x27),
    ("Quote", 0x28),
    ("NonUsHash", 0x2B),
    ("Backslash", 0x2B),
    ("NonUsBackslash", 0x56),
    ("Z", 0x2C),
    ("X", 0x2D),
    ("C", 0x2E),
    ("V", 0x2F),
    ("B", 0x30),
    ("N", 0x31),
    ("M", 0x32),
    ("Comma", 0x33),
    ("Period", 0x34),
    ("Slash", 0x35),
    ("Space", 0x39),
];

/// What a key writes with one combination of modifiers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Typed {
    pub text: String,
    /// A dead key: it only puts an accent on the next letter and writes nothing by itself.
    pub dead: bool,
}

/// What a key writes alone, with Shift, with AltGr and with Shift+AltGr.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct KeyChars {
    pub base: Option<Typed>,
    pub shift: Option<Typed>,
    pub altgr: Option<Typed>,
    pub shift_altgr: Option<Typed>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OsLayout {
    /// Windows' layout id (KLID), e.g. "00000410" for Italian.
    pub id: String,
    /// Its name as Windows shows it, e.g. "Italiano".
    pub name: String,
    /// By model key id (see `CHAR_KEYS`).
    pub keys: BTreeMap<String, KeyChars>,
}

impl OsLayout {
    /// The legend printed on a key of this layout: what it writes alone, in capitals when Shift
    /// gives the capital (letters like Q or Ñ, but not the Italian ò, whose Shift is ç).
    pub fn legend(&self, id: &str) -> Option<String> {
        if id == "Space" {
            return None;
        }
        let chars = self.keys.get(id)?;
        let base = &chars.base.as_ref()?.text;
        let upper = base.to_uppercase();
        Some(if chars.shift.as_ref().is_some_and(|s| s.text == upper) { upper } else { base.clone() })
    }
}

/// A layout installed in Windows, offered as a choice.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutChoice {
    pub id: String,
    pub name: String,
}

/// The layout Windows uses for the window `hwnd` (0: this thread), or the installed layout `id`.
pub fn read(hwnd: isize, id: Option<&str>) -> Option<OsLayout> {
    #[cfg(windows)]
    {
        let hkl = match id {
            Some(id) => win::installed().into_iter().find(|&h| win::klid(h) == id)?,
            None => win::current(hwnd),
        };
        Some(win::read(hkl))
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, id);
        None
    }
}

/// The layouts installed in Windows.
pub fn installed() -> Vec<LayoutChoice> {
    #[cfg(windows)]
    {
        let mut out: Vec<LayoutChoice> = Vec::new();
        for hkl in win::installed() {
            let id = win::klid(hkl);
            if !out.iter().any(|c| c.id == id) {
                out.push(LayoutChoice { name: win::name(&id), id });
            }
        }
        out
    }
    #[cfg(not(windows))]
    Vec::new()
}

#[cfg(windows)]
mod win {
    use super::{CHAR_KEYS, KeyChars, OsLayout, Typed};
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyboardLayout, GetKeyboardLayoutList, HKL, MAPVK_VSC_TO_VK_EX, MapVirtualKeyExW, ToUnicodeEx, VK_CONTROL,
        VK_LCONTROL, VK_MENU, VK_RMENU, VK_SHIFT,
    };
    use windows_sys::Win32::UI::Shell::SHLoadIndirectString;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    const LAYOUTS: &str = r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts";
    /// ToUnicodeEx flag: leave the keyboard state alone, so dead keys don't stick (Windows 10 1607+).
    const NO_STATE_CHANGE: u32 = 4;

    pub fn current(hwnd: isize) -> HKL {
        // The layout belongs to a thread: the window's UI thread is the one that follows the user.
        let thread = if hwnd != 0 { unsafe { GetWindowThreadProcessId(hwnd as _, null_mut()) } } else { 0 };
        unsafe { GetKeyboardLayout(thread) }
    }

    pub fn installed() -> Vec<HKL> {
        let count = unsafe { GetKeyboardLayoutList(0, null_mut()) };
        let mut list = vec![null_mut(); count.max(0) as usize];
        let got = unsafe { GetKeyboardLayoutList(count, list.as_mut_ptr()) };
        list.truncate(got.max(0) as usize);
        list
    }

    /// The KLID of a layout handle: the high word names the layout, or (0xFxxx) points to a
    /// variant listed in the registry by its "Layout Id".
    pub fn klid(hkl: HKL) -> String {
        let device = ((hkl as usize >> 16) & 0xFFFF) as u32;
        if device & 0xF000 != 0xF000 {
            return format!("{device:08X}");
        }
        let wanted = format!("{:04x}", device & 0x0FFF);
        let found = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(LAYOUTS).ok().and_then(|layouts| {
            layouts.enum_keys().flatten().find(|k| {
                layouts.open_subkey(k).and_then(|key| key.get_value::<String, _>("Layout Id")).is_ok_and(|v| v.eq_ignore_ascii_case(&wanted))
            })
        });
        found.map(|k| k.to_uppercase()).unwrap_or_else(|| format!("{:08X}", hkl as usize as u32))
    }

    /// The layout's name in the Windows language ("Layout Display Name"), else its English name.
    pub fn name(klid: &str) -> String {
        let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(format!(r"{LAYOUTS}\{klid}")) else {
            return klid.to_string();
        };
        if let Ok(source) = key.get_value::<String, _>("Layout Display Name") {
            let wide: Vec<u16> = source.encode_utf16().chain([0]).collect();
            let mut buf = [0u16; 256];
            if unsafe { SHLoadIndirectString(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, null_mut()) } == 0 {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                return String::from_utf16_lossy(&buf[..len]);
            }
        }
        key.get_value::<String, _>("Layout Text").unwrap_or_else(|_| klid.to_string())
    }

    fn typed(hkl: HKL, scan: u32, state: &[u8; 256]) -> Option<Typed> {
        let vk = unsafe { MapVirtualKeyExW(scan, MAPVK_VSC_TO_VK_EX, hkl) };
        if vk == 0 {
            return None;
        }
        let mut buf = [0u16; 8];
        let n = unsafe { ToUnicodeEx(vk, scan, state.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, NO_STATE_CHANGE, hkl) };
        let (len, dead) = if n < 0 { (1, true) } else { (n as usize, false) };
        let text = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
        // Ctrl+Alt also gives control characters on keys without an AltGr character.
        if text.is_empty() || text.chars().all(char::is_control) {
            return None;
        }
        Some(Typed { text, dead })
    }

    pub fn read(hkl: HKL) -> OsLayout {
        let mut shift = [0u8; 256];
        shift[VK_SHIFT as usize] = 0x80;
        let mut altgr = [0u8; 256];
        for vk in [VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU] {
            altgr[vk as usize] = 0x80;
        }
        let mut shift_altgr = altgr;
        shift_altgr[VK_SHIFT as usize] = 0x80;
        let keys = CHAR_KEYS
            .iter()
            .map(|&(id, scan)| {
                let chars = KeyChars {
                    base: typed(hkl, scan, &[0; 256]),
                    shift: typed(hkl, scan, &shift),
                    altgr: typed(hkl, scan, &altgr),
                    shift_altgr: typed(hkl, scan, &shift_altgr),
                };
                (id.to_string(), chars)
            })
            .collect();
        let id = klid(hkl);
        OsLayout { name: name(&id), id, keys }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_char_key_has_a_scan_code_once() {
        let mut ids: Vec<&str> = CHAR_KEYS.iter().map(|(id, _)| *id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CHAR_KEYS.len());
    }

    #[test]
    fn legends_are_capital_letters_and_skip_space() {
        let typed = |t: &str| Some(Typed { text: t.into(), dead: false });
        let mut keys = BTreeMap::new();
        keys.insert("Q".to_string(), KeyChars { base: typed("q"), shift: typed("Q"), ..Default::default() });
        keys.insert("Semicolon".to_string(), KeyChars { base: typed("ñ"), shift: typed("Ñ"), ..Default::default() });
        keys.insert("Quote".to_string(), KeyChars { base: typed("ò"), shift: typed("ç"), ..Default::default() });
        keys.insert("Space".to_string(), KeyChars { base: typed(" "), ..Default::default() });
        let layout = OsLayout { id: "0000040A".into(), name: "Spagnolo".into(), keys };
        assert_eq!(layout.legend("Q").as_deref(), Some("Q"));
        assert_eq!(layout.legend("Semicolon").as_deref(), Some("Ñ"));
        assert_eq!(layout.legend("Quote").as_deref(), Some("ò"));
        assert_eq!(layout.legend("Space"), None);
        assert_eq!(layout.legend("Enter"), None);
    }

    /// Loads layouts this PC may not have (Spanish, US, German, French) just for the test, reads
    /// them and unloads them again. Run with `cargo test -p lucent other_layouts -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore = "loads keyboard layouts into the Windows session for a moment"]
    fn other_layouts() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KLF_NOTELLSHELL, LoadKeyboardLayoutW, UnloadKeyboardLayout};
        let before = win::installed();
        for (klid, expect) in [("0000040A", [("Semicolon", "Ñ"), ("Equal", "¡")]), ("00000409", [("Semicolon", ";"), ("Quote", "'")]), ("00000407", [("Y", "Z"), ("Minus", "ß")]), ("0000040C", [("Q", "A"), ("M", ",")])] {
            let wide: Vec<u16> = klid.encode_utf16().chain([0]).collect();
            let hkl = unsafe { LoadKeyboardLayoutW(wide.as_ptr(), KLF_NOTELLSHELL) };
            assert!(!hkl.is_null(), "{klid} not loaded");
            let layout = win::read(hkl);
            if !before.contains(&hkl) {
                unsafe { UnloadKeyboardLayout(hkl) };
            }
            println!("{} {}: {:?}", layout.id, layout.name, CHAR_KEYS.iter().map(|(id, _)| layout.legend(id).unwrap_or_default()).collect::<Vec<_>>().join(" "));
            let dead: Vec<String> = layout.keys.iter().flat_map(|(id, c)| [&c.base, &c.shift, &c.altgr, &c.shift_altgr].into_iter().flatten().filter(|t| t.dead).map(move |t| format!("{id}:{}", t.text))).collect();
            println!("    dead keys: {dead:?}");
            for (id, legend) in expect {
                assert_eq!(layout.legend(id).as_deref(), Some(legend), "{klid} {id}");
            }
        }
        assert_eq!(win::installed(), before, "the test must leave the installed layouts as they were");
    }

    /// Reads the layout of this machine; prints it so a run shows what Windows reports.
    #[cfg(windows)]
    #[test]
    fn reads_the_installed_layouts() {
        let all = installed();
        assert!(!all.is_empty());
        for choice in &all {
            let layout = read(0, Some(&choice.id)).expect("installed layout");
            assert_eq!(layout.legend("A").as_deref().map(|s| s.is_empty()), Some(false), "{}", choice.name);
            println!("{} {}: {:?}", choice.id, choice.name, CHAR_KEYS.iter().map(|(id, _)| layout.legend(id)).collect::<Vec<_>>());
        }
    }
}
