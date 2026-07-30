// A loaded file is a PDF the renderer reads directly via an asset:// URL
// (produced by convertFileSrc), so PDF.js never has to cross the IPC boundary.
export type LoadedSource = { name: string; url: string };

export const PDF_EXTENSIONS = ["pdf"];

export function isPdf(path: string): boolean {
  const lower = path.toLowerCase();
  return PDF_EXTENSIONS.some((ext) => lower.endsWith(`.${ext}`));
}

// Viewer display state surfaced from PdfView to the top bar. Shared so both
// sides reference one definition instead of hand-maintaining matching shapes.
export interface ViewerState {
  currentPage: number;
  totalPages: number;
  scalePercent: number;
  findIndex: number;
  findCount: number;
}
