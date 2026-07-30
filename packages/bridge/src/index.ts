/**
 * `@window/bridge` — typed boundary between the Vue frontends and the Rust backends.
 *
 * Holds the shared `invoke("...")` string literals as typed functions. App-specific
 * commands live in each app's own `bridge.ts`, built on the re-exported {@link invoke}.
 */
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

export { invoke, convertFileSrc };

/** A named group of extensions for a file dialog, e.g. `{ name: "PDF", extensions: ["pdf"] }`. */
export interface FileFilter {
  name: string;
  extensions: string[];
}

/**
 * The path the app was launched with (OS file association / double-click), or `null`.
 * Returns the path once then clears it, so call exactly once on mount.
 */
export function getStartupFile(): Promise<string | null> {
  return invoke<string | null>("get_startup_file");
}

/** Open a native single-file picker. Returns the chosen path, or `null` if cancelled. */
export async function openFile(filters: FileFilter[], title?: string): Promise<string | null> {
  const selected = await open({ multiple: false, directory: false, title, filters });
  return typeof selected === "string" ? selected : null;
}

/** Open a native save dialog. Returns the chosen path, or `null` if cancelled. */
export function saveFile(filters: FileFilter[], defaultPath?: string): Promise<string | null> {
  return save({ filters, defaultPath });
}

/**
 * Subscribe to OS drag-and-drop of files onto the window. Calls `onDrop` with the dropped
 * paths. Returns a promise resolving to an unlisten function.
 * (HTML5 file drag-drop is disabled in a Tauri webview, so this is the supported path.)
 */
export function onFileDrop(
  onDrop: (paths: string[]) => void,
  onHover?: (hovering: boolean) => void,
): Promise<() => void> {
  return getCurrentWebview().onDragDropEvent((event) => {
    const { type } = event.payload;
    if (type === "over" || type === "enter") onHover?.(true);
    else if (type === "leave") onHover?.(false);
    else if (type === "drop") {
      onHover?.(false);
      onDrop(event.payload.paths);
    }
  });
}

/**
 * Subscribe to runtime file-open requests routed from the backend. `onOpen` is called with
 * the path. Returns a promise resolving to an unlisten function.
 *
 * Used on macOS, where the OS delivers file-opens as Apple Events to the running app. On
 * Windows/Linux each file-open is a new process (file arrives via {@link getStartupFile}),
 * so this listener never fires there.
 */
export function onOpenFile(onOpen: (path: string) => void): Promise<() => void> {
  return listen<string>("open-file", (event) => onOpen(event.payload));
}

/**
 * Start watching the currently-open file for on-disk changes. Replaces any previous watch.
 * Call after loading a file; changes arrive via {@link onFileChanged}. Detection only — the
 * app decides what to do.
 */
export function watchFile(path: string): Promise<void> {
  return invoke<void>("watch_file", { path });
}

/** Subscribe to "the open file changed on disk" notifications. Returns an unlisten function. */
export function onFileChanged(onChange: () => void): Promise<() => void> {
  return listen("file-changed", () => onChange());
}
