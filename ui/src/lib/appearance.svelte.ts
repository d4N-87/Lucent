import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Light or dark interface; "auto" follows the system. The keyboard preview stays dark. */
export type Appearance = "auto" | "light" | "dark";

export const APPEARANCES: Appearance[] = ["auto", "light", "dark"];

const system = matchMedia("(prefers-color-scheme: light)");
let state = $state<{ setting: Appearance; resolved: "light" | "dark" }>({ setting: "auto", resolved: "dark" });

export const appearance = {
  get setting() {
    return state.setting;
  },
  /** The theme in use. */
  get resolved() {
    return state.resolved;
  },
};

/**
 * The window frame and title bar follow too; null lets Windows decide. The window's theme also
 * forces the system query, so it is set first: after "light", "auto" must not read a forced light.
 */
async function apply(setting: Appearance) {
  await getCurrentWindow()
    .setTheme(setting === "auto" ? null : setting)
    .catch((e) => console.warn(`Tema della finestra non impostato: ${e}`));
  const resolved = setting === "auto" ? (system.matches ? "light" : "dark") : setting;
  state = { setting, resolved };
  document.documentElement.dataset.theme = resolved;
}

system.addEventListener("change", () => state.setting === "auto" && apply("auto"));

/** Applies the saved appearance; called once before the app is mounted. */
export async function loadAppearance() {
  await apply(await invoke<Appearance>("appearance"));
}

export async function setAppearance(setting: Appearance) {
  await apply(await invoke<Appearance>("set_appearance", { appearance: setting }));
}
