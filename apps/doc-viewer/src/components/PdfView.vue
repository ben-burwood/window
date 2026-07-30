<script setup lang="ts">
import { onMounted, onUnmounted, nextTick, ref, watch } from "vue";
import * as pdfjsLib from "pdfjs-dist";
import type {
  PDFDocumentProxy,
  PDFPageProxy,
  RenderTask,
} from "pdfjs-dist";
// Vite resolves this to a hashed URL for the bundled worker.
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import type { LoadedSource, ViewerState } from "../types";

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;

const props = defineProps<{ source: LoadedSource }>();
const emit = defineEmits<{
  error: [message: string];
  state: [state: ViewerState];
}>();

const scrollContainer = ref<HTMLDivElement | null>(null);

// Reactive UI state, surfaced to the parent top bar via the `state` event.
const numPages = ref(0);
const currentPage = ref(1);
const scale = ref(1);
const findIndex = ref(0); // 1-based index of the active match, 0 when none
const findCount = ref(0);
type FitMode = "none" | "width" | "page";
let fitMode: FitMode = "width";
let findQuery = "";

// Held outside Vue reactivity on purpose: PDF.js manages its own internal state
// and should not be wrapped in a reactive proxy.
let pdfDoc: PDFDocumentProxy | null = null;
const pageProxies = new Map<number, PDFPageProxy>();
const baseSizes = new Map<number, { w: number; h: number }>();
const renderTasks = new Map<number, RenderTask>();
const textLayers = new Map<number, pdfjsLib.TextLayer>();
const pageTextDivs = new Map<number, HTMLElement[]>();
const pageTextStrs = new Map<number, string[]>();

// Element registries, keyed by 1-based page number.
const canvases = new Map<number, HTMLCanvasElement>();
const textEls = new Map<number, HTMLElement>();
const pageEls = new Map<number, HTMLElement>();
let observer: IntersectionObserver | null = null;

// Template ref callbacks: register the element under its page number, or drop
// it when the page unmounts.
function elSetter<T extends HTMLElement>(map: Map<number, T>) {
  return (el: Element | null, n: number) => {
    if (el) map.set(n, el as T);
    else map.delete(n);
  };
}
const setCanvas = elSetter(canvases);
const setTextEl = elSetter(textEls);
const setPageEl = elSetter(pageEls);

const PAGE_GAP = 16;
const MIN_SCALE = 0.25;
const MAX_SCALE = 6;

function emitState() {
  emit("state", {
    currentPage: currentPage.value,
    totalPages: numPages.value,
    scalePercent: Math.round(scale.value * 100),
    findIndex: findIndex.value,
    findCount: findCount.value,
  });
}
watch([currentPage, numPages, scale, findIndex, findCount], emitState);

// Available space for a page (minus padding), and the largest page at scale 1,
// so a single fit factor suits every page.
function containerSize(): { w: number; h: number } {
  const el = scrollContainer.value;
  return {
    w: el ? el.clientWidth - PAGE_GAP * 2 : 800,
    h: el ? el.clientHeight - PAGE_GAP * 2 : 600,
  };
}
function maxBase(): { w: number; h: number } {
  let w = 1;
  let h = 1;
  for (const s of baseSizes.values()) {
    w = Math.max(w, s.w);
    h = Math.max(h, s.h);
  }
  return { w, h };
}
function clampScale(s: number): number {
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, s));
}

function applyFit() {
  const c = containerSize();
  const b = maxBase();
  if (fitMode === "width") {
    scale.value = clampScale(c.w / b.w);
  } else if (fitMode === "page") {
    scale.value = clampScale(Math.min(c.w / b.w, c.h / b.h));
  }
}

async function renderPage(n: number) {
  const page = pageProxies.get(n);
  const canvas = canvases.get(n);
  if (!page || !canvas) return;

  renderTasks.get(n)?.cancel();

  const dpr = window.devicePixelRatio || 1;
  const viewport = page.getViewport({ scale: scale.value });
  canvas.width = Math.floor(viewport.width * dpr);
  canvas.height = Math.floor(viewport.height * dpr);
  canvas.style.width = `${Math.floor(viewport.width)}px`;
  canvas.style.height = `${Math.floor(viewport.height)}px`;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const task = page.render({
    canvasContext: ctx,
    viewport,
    transform: dpr !== 1 ? [dpr, 0, 0, dpr, 0, 0] : undefined,
  });
  renderTasks.set(n, task);

  // Text layer — invisible, selectable text positioned over the canvas.
  const textEl = textEls.get(n);
  if (textEl) {
    textLayers.get(n)?.cancel();
    textEl.replaceChildren();
    textEl.style.setProperty("--scale-factor", String(scale.value));
    textEl.style.width = `${Math.floor(viewport.width)}px`;
    textEl.style.height = `${Math.floor(viewport.height)}px`;
    const layer = new pdfjsLib.TextLayer({
      textContentSource: page.streamTextContent(),
      container: textEl,
      viewport,
    });
    textLayers.set(n, layer);
    try {
      await layer.render();
      pageTextDivs.set(n, layer.textDivs);
      pageTextStrs.set(n, layer.textContentItemsStr);
    } catch {
      // Cancelled by a newer render; ignore.
    }
  }

  try {
    await task.promise;
  } catch {
    // Cancelled by a newer render (e.g. a zoom change); ignore.
  }
}

async function renderAll() {
  for (let n = 1; n <= numPages.value; n++) await renderPage(n);
  if (findQuery) recomputeMatches(false);
}

function observePages() {
  observer?.disconnect();
  observer = new IntersectionObserver(
    (entries) => {
      let best: { n: number; ratio: number } | null = null;
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        const n = Number((e.target as HTMLElement).dataset.page);
        if (!best || e.intersectionRatio > best.ratio)
          best = { n, ratio: e.intersectionRatio };
      }
      if (best) currentPage.value = best.n;
    },
    { root: scrollContainer.value, threshold: [0.1, 0.5, 0.9] },
  );
  for (const el of pageEls.values()) observer.observe(el);
}

async function destroyDoc() {
  for (const t of renderTasks.values()) t.cancel();
  renderTasks.clear();
  for (const l of textLayers.values()) l.cancel();
  textLayers.clear();
  observer?.disconnect();
  observer = null;
  clearHighlights();
  for (const p of pageProxies.values()) p.cleanup();
  pageProxies.clear();
  baseSizes.clear();
  canvases.clear();
  textEls.clear();
  pageEls.clear();
  pageTextDivs.clear();
  pageTextStrs.clear();
  if (pdfDoc) {
    await pdfDoc.destroy();
    pdfDoc = null;
  }
}

async function loadDocument(url: string) {
  await destroyDoc();
  numPages.value = 0;
  currentPage.value = 1;
  findQuery = "";
  findIndex.value = 0;
  findCount.value = 0;
  fitMode = "width";
  try {
    const doc = await pdfjsLib.getDocument({ url }).promise;
    pdfDoc = doc;
    const count = doc.numPages;
    // Fetch page proxies concurrently — each is an independent worker round-trip,
    // so a serial loop would make open latency grow with page count.
    const pages = await Promise.all(
      Array.from({ length: count }, (_, i) => doc.getPage(i + 1)),
    );
    pages.forEach((page, i) => {
      const n = i + 1;
      pageProxies.set(n, page);
      const vp = page.getViewport({ scale: 1 });
      baseSizes.set(n, { w: vp.width, h: vp.height });
    });
    numPages.value = count; // reveals the page elements in the template
    await nextTick();
    applyFit();
    await renderAll();
    observePages();
  } catch (e) {
    emit("error", e instanceof Error ? e.message : String(e));
  }
}

// --- Find ---
interface Match {
  page: number;
  div: number;
  start: number;
  length: number;
}
let matches: Match[] = [];

function supportsHighlight(): boolean {
  return (
    typeof CSS !== "undefined" &&
    "highlights" in CSS &&
    typeof (globalThis as any).Highlight === "function"
  );
}
function clearHighlights() {
  if (supportsHighlight()) {
    (CSS as any).highlights.delete("pdf-find");
    (CSS as any).highlights.delete("pdf-find-current");
  }
}
function applyHighlights() {
  if (!supportsHighlight()) return;
  const all = new (globalThis as any).Highlight();
  const cur = new (globalThis as any).Highlight();
  matches.forEach((m, i) => {
    const div = pageTextDivs.get(m.page)?.[m.div];
    const node = div?.firstChild;
    if (!node || node.nodeType !== Node.TEXT_NODE) return;
    const len = (node as Text).length;
    const range = document.createRange();
    range.setStart(node, Math.min(m.start, len));
    range.setEnd(node, Math.min(m.start + m.length, len));
    (i === findIndex.value - 1 ? cur : all).add(range);
  });
  (CSS as any).highlights.set("pdf-find", all);
  (CSS as any).highlights.set("pdf-find-current", cur);
}

function recomputeMatches(resetIndex: boolean) {
  matches = [];
  const q = findQuery.trim().toLowerCase();
  if (!q) {
    findCount.value = 0;
    findIndex.value = 0;
    clearHighlights();
    return;
  }
  for (let n = 1; n <= numPages.value; n++) {
    const strs = pageTextStrs.get(n);
    if (!strs) continue;
    for (let d = 0; d < strs.length; d++) {
      const hay = strs[d].toLowerCase();
      let from = 0;
      let idx = hay.indexOf(q, from);
      while (idx !== -1) {
        matches.push({ page: n, div: d, start: idx, length: q.length });
        from = idx + q.length;
        idx = hay.indexOf(q, from);
      }
    }
  }
  findCount.value = matches.length;
  if (resetIndex) findIndex.value = matches.length ? 1 : 0;
  else findIndex.value = Math.min(findIndex.value || 1, matches.length) || 0;
  applyHighlights();
}

// Scroll a descendant into view by moving ONLY the PDF scroll container.
// We deliberately avoid Element.scrollIntoView(): it scrolls every scrollable
// ancestor, and since `overflow: hidden` blocks user scrolling but not
// programmatic scrolling, it would shift the app chrome (navbar) off-screen.
function scrollElementIntoView(el: HTMLElement, block: "start" | "center") {
  const container = scrollContainer.value;
  if (!container) return;
  const elRect = el.getBoundingClientRect();
  const cRect = container.getBoundingClientRect();
  const delta = elRect.top - cRect.top;
  const offset =
    block === "start" ? PAGE_GAP : (container.clientHeight - elRect.height) / 2;
  container.scrollTo({
    top: container.scrollTop + delta - offset,
    behavior: "smooth",
  });
}

function scrollToMatch(i: number) {
  const m = matches[i];
  if (!m) return;
  const div = pageTextDivs.get(m.page)?.[m.div];
  if (div) scrollElementIntoView(div, "center");
}

// --- Exposed controls (driven by the top bar) ---
function goto(page: number) {
  const p = Math.min(numPages.value, Math.max(1, page));
  const el = pageEls.get(p);
  if (el) scrollElementIntoView(el, "start");
  currentPage.value = p;
}
function prev() {
  goto(currentPage.value - 1);
}
function next() {
  goto(currentPage.value + 1);
}
function zoomIn() {
  fitMode = "none";
  scale.value = clampScale(scale.value * 1.2);
  renderAll();
}
function zoomOut() {
  fitMode = "none";
  scale.value = clampScale(scale.value / 1.2);
  renderAll();
}
function fitWidth() {
  fitMode = "width";
  applyFit();
  renderAll();
}
function fitPage() {
  fitMode = "page";
  applyFit();
  renderAll();
}
function setFind(query: string) {
  findQuery = query;
  recomputeMatches(true);
  if (matches.length) scrollToMatch(0);
}
function findNext() {
  if (!matches.length) return;
  findIndex.value = (findIndex.value % matches.length) + 1;
  applyHighlights();
  scrollToMatch(findIndex.value - 1);
}
function findPrev() {
  if (!matches.length) return;
  findIndex.value = findIndex.value <= 1 ? matches.length : findIndex.value - 1;
  applyHighlights();
  scrollToMatch(findIndex.value - 1);
}

defineExpose({
  prev,
  next,
  goto,
  zoomIn,
  zoomOut,
  fitWidth,
  fitPage,
  setFind,
  findNext,
  findPrev,
});

// --- Ctrl / Cmd + wheel zoom ---
let renderScheduled = false;
function scheduleRender() {
  if (renderScheduled) return;
  renderScheduled = true;
  requestAnimationFrame(() => {
    renderScheduled = false;
    renderAll();
  });
}
function onWheel(e: WheelEvent) {
  if (!e.ctrlKey && !e.metaKey) return;
  e.preventDefault();
  fitMode = "none";
  scale.value = clampScale(scale.value * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
  scheduleRender();
}

function onResize() {
  if (fitMode === "none") return;
  applyFit();
  renderAll();
}

onMounted(() => {
  window.addEventListener("resize", onResize);
  loadDocument(props.source.url);
});

watch(
  () => props.source.url,
  (url) => loadDocument(url),
);

onUnmounted(async () => {
  window.removeEventListener("resize", onResize);
  await destroyDoc();
});
</script>

<template>
  <div
    ref="scrollContainer"
    class="scroll"
    @wheel="onWheel"
  >
    <div
      v-for="n in numPages"
      :key="n"
      class="page"
      :data-page="n"
      :ref="(el) => setPageEl(el as Element | null, n)"
    >
      <canvas :ref="(el) => setCanvas(el as Element | null, n)"></canvas>
      <div
        class="textLayer"
        :ref="(el) => setTextEl(el as Element | null, n)"
      ></div>
    </div>
  </div>
</template>

<style scoped>
.scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: #525659;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 16px;
}

.page {
  position: relative;
  background: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
  line-height: 0;
}

canvas {
  display: block;
}
</style>
