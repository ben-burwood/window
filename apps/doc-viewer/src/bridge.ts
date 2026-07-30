// doc-viewer's typed IPC surface, built on the shared @window/bridge. Components import from
// here, never call invoke directly. doc-viewer has no format-specific backend commands: it
// reads the PDF bytes directly via the asset:// protocol (convertFileSrc) and only needs the
// shared startup-file / file-picker / drag-drop helpers, so this is purely re-exports.
export { getStartupFile, openFile, convertFileSrc, onFileDrop, onOpenFile } from "@window/bridge";
