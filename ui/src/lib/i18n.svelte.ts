import { invoke } from "@tauri-apps/api/core";

/** Interface strings from `lang/*.ini` (see src-tauri/src/i18n.rs). Keys are `section.key`. */
export interface Language {
  code: string;
  /** The language's own name, e.g. "Italiano". */
  name: string;
}

interface LanguageState {
  /** "auto" or a language code. */
  setting: string;
  active: string;
  languages: Language[];
  strings: Record<string, string>;
  english: Record<string, string>;
  folder: string | null;
}

type Params = Record<string, string | number>;

let lang = $state<LanguageState>({
  setting: "auto",
  active: "en",
  languages: [],
  strings: {},
  english: {},
  folder: null,
});

const fill = (text: string, params?: Params) =>
  params ? text.replace(/\{(\w+)\}/g, (m, name) => (name in params ? String(params[name]) : m)) : text;

/** Text for `key` in the interface language. Reactive: the UI follows language changes. */
export const t = (key: string, params?: Params) => fill(lang.strings[key] ?? key, params);

/** English text for `key`, for MCP answers. */
export const tEn = (key: string, params?: Params) => fill(lang.english[key] ?? key, params);

export const has = (key: string) => key in lang.strings;

/** Singular or plural form: `${key}_one` or `${key}_other`, with `{n}` filled in. */
export const plural = (key: string, n: number) => t(`${key}_${n === 1 ? "one" : "other"}`, { n });

export const language = {
  get setting() {
    return lang.setting;
  },
  get active() {
    return lang.active;
  },
  get languages() {
    return lang.languages;
  },
  get folder() {
    return lang.folder;
  },
  /** Name of the language in use. */
  get name() {
    return lang.languages.find((l) => l.code === lang.active)?.name ?? lang.active;
  },
};

function apply(next: LanguageState) {
  lang = next;
  document.documentElement.lang = next.active;
}

/** Loads the chosen language; called once before the app is mounted. */
export async function loadLanguage() {
  apply(await invoke<LanguageState>("language", { system: navigator.language }));
}

/** "auto" follows the system language. Remembered in the settings. */
export async function setLanguage(code: string) {
  apply(await invoke<LanguageState>("set_language", { language: code, system: navigator.language }));
}

export const openLangDir = () => invoke<void>("open_lang_dir");
