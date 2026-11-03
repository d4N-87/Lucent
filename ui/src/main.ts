import { mount } from "svelte";
import App from "./App.svelte";
import { loadAppearance } from "./lib/appearance.svelte";
import { loadLanguage } from "./lib/i18n.svelte";
import { loadLook } from "./lib/look.svelte";
import { forwardToLog } from "./lib/log";
import "./app.css";

forwardToLog();
// Strings and theme must be there before the first render.
Promise.all([
  loadLanguage().catch((e) => console.error(`Lingua non caricata: ${e}`)),
  loadAppearance().catch((e) => console.error(`Aspetto non caricato: ${e}`)),
  loadLook().catch((e) => console.error(`Colori della tastiera non caricati: ${e}`)),
]).finally(() => mount(App, { target: document.getElementById("app")! }));
