import { error, warn } from "@tauri-apps/plugin-log";

/** Sends UI errors and warnings to the app log file, next to the backend's entries. */
export function forwardToLog() {
  const forward = (level: "error" | "warn", write: (msg: string) => Promise<void>) => {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      original(...args);
      write(`UI: ${args.map(String).join(" ")}`).catch(() => {});
    };
  };
  forward("error", error);
  forward("warn", warn);
  window.addEventListener("error", (e) => {
    error(`UI: ${e.message} (${e.filename}:${e.lineno})`).catch(() => {});
  });
  window.addEventListener("unhandledrejection", (e) => {
    error(`UI: promise non gestita: ${String(e.reason)}`).catch(() => {});
  });
}
