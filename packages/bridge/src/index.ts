/**
 * `@viewers/bridge` — the typed boundary between the Vue frontends and the Rust backends.
 *
 * This is the single place where shared `invoke("...")` string literals live. Components must
 * never call `invoke` with a raw string directly; they import a typed function from here (for
 * shared commands) or from their app's own `bridge.ts` (for that app's format-specific
 * commands), which is built on the re-exported {@link invoke}.
 */
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

export { invoke, convertFileSrc };

/** A named group of extensions for a file dialog, e.g. `{ name: "PDF", extensions: ["pdf"] }`. */
export interface FileFilter {
  name: string;
  extensions: string[];
}

/**
 * The path the app was launched with (OS file association / double-click), or `null`.
 * Backed by viewer-tauri's shared `get_startup_file` command; returns the path once then
 * clears it, so call it exactly once on mount.
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
 *
 * (In a Tauri webview, HTML5 file drag-drop is disabled; this is the supported path.)
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
