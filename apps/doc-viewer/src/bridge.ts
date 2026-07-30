// doc-viewer's IPC surface. Components import from here, never call invoke directly.
// The PDF is read directly via asset:// (convertFileSrc), so there are no format-specific
// backend commands — this only re-exports the shared startup / file-picker / drag-drop helpers.
export {
  getStartupFile,
  openFile,
  convertFileSrc,
  onFileDrop,
  onOpenFile,
  watchFile,
  onFileChanged,
} from "@window/bridge";
