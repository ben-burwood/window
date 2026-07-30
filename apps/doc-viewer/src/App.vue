<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import PdfView from "./components/PdfView.vue";
import { isPdf, PDF_EXTENSIONS, type LoadedSource, type ViewerState } from "./types";

const loaded = ref<LoadedSource | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dragOver = ref(false);
let unlistenDrop: (() => void) | null = null;

const viewer = ref<InstanceType<typeof PdfView> | null>(null);

// Display state pushed up from the viewer, rendered in the top bar.
const st = ref<ViewerState>({
  currentPage: 1,
  totalPages: 0,
  scalePercent: 100,
  findIndex: 0,
  findCount: 0,
});

// Local editable copy of the page number so typing doesn't fight the viewer.
const pageInput = ref("1");
watch(
  () => st.value.currentPage,
  (p) => {
    pageInput.value = String(p);
  },
);
function commitPage() {
  const n = parseInt(pageInput.value, 10);
  if (!Number.isNaN(n)) viewer.value?.goto(n);
  else pageInput.value = String(st.value.currentPage);
}

const findQuery = ref("");
const findInput = ref<HTMLInputElement | null>(null);
function onFindInput() {
  viewer.value?.setFind(findQuery.value);
}

function basename(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

async function loadFileByPath(path: string) {
  loading.value = true;
  error.value = null;
  try {
    const name = basename(path);
    // The viewer reads the PDF directly via the asset:// protocol, so the bytes
    // never cross the IPC boundary.
    loaded.value = { name, url: convertFileSrc(path) };
  } catch (e) {
    loaded.value = null;
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

function onViewError(message: string) {
  loaded.value = null;
  error.value = message;
}

async function openFile() {
  const path = (await open({
    multiple: false,
    filters: [{ name: "PDF", extensions: PDF_EXTENSIONS }],
  })) as string | null;
  if (!path) return;
  await loadFileByPath(path);
}

// Ctrl/Cmd+F focuses the find box (the classic find shortcut).
function onKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f" && loaded.value) {
    e.preventDefault();
    findInput.value?.focus();
    findInput.value?.select();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);

  // OS file drops are delivered by the webview (HTML5 drop events don't carry a
  // real filesystem path under Tauri), so we listen for the native drag-drop.
  unlistenDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
    const p = event.payload;
    if (p.type === "enter" || p.type === "over") {
      dragOver.value = true;
    } else if (p.type === "leave") {
      dragOver.value = false;
    } else if (p.type === "drop") {
      dragOver.value = false;
      const path = p.paths.find(isPdf);
      if (path) await loadFileByPath(path);
      else error.value = "Please drop a PDF file.";
    }
  });

  const startupFile = await invoke<string | null>("get_startup_file");
  if (startupFile) await loadFileByPath(startupFile);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  unlistenDrop?.();
});
</script>

<template>
  <div class="app">
    <header v-if="loaded" class="topbar">
      <span class="file-name" :title="loaded.name">{{ loaded.name }}</span>

      <div class="sep"></div>

      <div class="group">
        <button
          class="tb"
          title="Previous page"
          :disabled="st.currentPage <= 1"
          @click="viewer?.prev()"
        >
          ‹
        </button>
        <input
          class="page-input"
          type="text"
          inputmode="numeric"
          :value="pageInput"
          @input="pageInput = ($event.target as HTMLInputElement).value"
          @change="commitPage"
          @keyup.enter="commitPage"
        />
        <span class="muted">/ {{ st.totalPages }}</span>
        <button
          class="tb"
          title="Next page"
          :disabled="st.currentPage >= st.totalPages"
          @click="viewer?.next()"
        >
          ›
        </button>
      </div>

      <div class="group">
        <button class="tb" title="Zoom out" @click="viewer?.zoomOut()">−</button>
        <span class="muted zoom">{{ st.scalePercent }}%</span>
        <button class="tb" title="Zoom in" @click="viewer?.zoomIn()">+</button>
        <button class="tb text" title="Fit width" @click="viewer?.fitWidth()">Fit width</button>
        <button class="tb text" title="Fit page" @click="viewer?.fitPage()">Fit page</button>
      </div>

      <div class="group find">
        <input
          ref="findInput"
          class="find-input"
          type="text"
          placeholder="Find"
          v-model="findQuery"
          @input="onFindInput"
          @keyup.enter="viewer?.findNext()"
        />
        <span class="muted find-count">
          {{ st.findCount ? `${st.findIndex}/${st.findCount}` : findQuery ? "0/0" : "" }}
        </span>
        <button
          class="tb"
          title="Previous match"
          :disabled="!st.findCount"
          @click="viewer?.findPrev()"
        >
          ‹
        </button>
        <button
          class="tb"
          title="Next match"
          :disabled="!st.findCount"
          @click="viewer?.findNext()"
        >
          ›
        </button>
      </div>

      <button class="btn-primary open-btn" @click="openFile">Open File</button>
    </header>

    <main class="content">
      <PdfView
        v-if="loaded"
        ref="viewer"
        :source="loaded"
        @error="onViewError"
        @state="st = $event"
      />

      <div v-else class="overlay">
        <div class="drop-zone" :class="{ dragging: dragOver }">
          <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round"
              d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z" />
          </svg>
          <p v-if="dragOver" class="hint">Drop to open</p>
          <p v-else-if="error" class="hint error">{{ error }}</p>
          <p v-else class="hint">Open or drop a PDF file to get started</p>
          <button class="btn-primary drop-open" @click="openFile" :disabled="loading">
            {{ loading ? "Loading…" : "Open File" }}
          </button>
        </div>
      </div>
    </main>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}

.topbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 14px;
  padding: 6px 14px;
  background: #f8f8f8;
  border-bottom: 1px solid #babfc7;
  font-size: 14px;
  flex-shrink: 0;
}

.file-name {
  color: #475569;
  font-size: 14px;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sep {
  width: 1px;
  align-self: stretch;
  background: #d5d9df;
}

.group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.muted {
  color: #475569;
  font-size: 13px;
}

.zoom {
  min-width: 40px;
  text-align: center;
}

.tb {
  min-width: 30px;
  height: 28px;
  padding: 0 8px;
  font-size: 16px;
  line-height: 1;
  border: 1px solid #babfc7;
  border-radius: 4px;
  background: #fff;
  color: #181d1f;
  cursor: pointer;
  transition: background 0.15s;
}

.tb.text {
  font-size: 13px;
}

.tb:hover:not(:disabled) {
  background: #eef2f7;
}

.tb:disabled {
  opacity: 0.4;
  cursor: default;
}

.page-input,
.find-input {
  height: 28px;
  border: 1px solid #babfc7;
  border-radius: 4px;
  font-size: 13px;
}

.page-input {
  width: 44px;
  text-align: center;
}

.find-input {
  width: 140px;
  padding: 0 8px;
}

.find-count {
  min-width: 34px;
  text-align: center;
}

.btn-primary {
  border: none;
  border-radius: 4px;
  font-family: inherit;
  background: #2196f3;
  color: #fff;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-primary:hover:not(:disabled) {
  background: color-mix(in srgb, #2196f3 82%, #000);
}

.btn-primary:disabled {
  opacity: 0.55;
  cursor: default;
}

.open-btn {
  margin-left: auto;
  padding: 4px 14px;
  font-size: 0.8rem;
}

.drop-open {
  padding: 10px 28px;
  font-size: 14px;
}

.content {
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}

/* Launch screen — matches data-framer's empty state */
.overlay {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 48px 64px;
  border: 2px dashed #babfc7;
  border-radius: 16px;
  color: #181d1f;
  transition: border-color 0.15s, background 0.15s;
}

.drop-zone.dragging {
  border-color: #2196f3;
  background: rgba(33, 150, 243, 0.07);
}

.icon {
  width: 48px;
  height: 48px;
  color: #aaa;
}

.hint {
  margin: 0;
  font-size: 0.95rem;
  color: #888;
}

.hint.error {
  color: #b91c1c;
  max-width: 360px;
  text-align: center;
}

</style>
