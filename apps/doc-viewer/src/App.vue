<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { EmptyState, Toolbar, ToolbarButton } from "@window/ui";
import PdfView from "./components/PdfView.vue";
import { isPdf, PDF_EXTENSIONS, type LoadedSource, type ViewerState } from "./types";
import { getStartupFile, openFile, convertFileSrc, onFileDrop, onOpenFile } from "./bridge";

const loaded = ref<LoadedSource | null>(null);
const error = ref<string | null>(null);
const loading = ref(false);
const dragging = ref(false);
const unlisteners: Array<() => void> = [];

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

// Single deduped entry point: every source (dialog, startup, drop, onOpenFile)
// funnels through here so the same file never loads twice.
let lastOpened: string | null = null;
async function openPath(path: string) {
  if (path === lastOpened) return;
  lastOpened = path;
  await loadFileByPath(path);
}

function handleDrop(paths: string[]) {
  const path = paths.find(isPdf);
  if (path) openPath(path);
  else error.value = "Please drop a PDF file.";
}

async function chooseFile() {
  const path = await openFile([{ name: "PDF", extensions: PDF_EXTENSIONS }]);
  if (!path) return;
  await openPath(path);
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
  // onOpenFile covers macOS runtime file-opens. Subscribe before pulling the
  // startup file so nothing is missed; everything funnels through openPath.
  const un1 = await onFileDrop(handleDrop, (hovering) => (dragging.value = hovering));
  const un2 = await onOpenFile(openPath);
  unlisteners.push(un1, un2);

  const startup = await getStartupFile();
  if (startup) await openPath(startup);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  unlisteners.forEach((u) => u());
});
</script>

<template>
  <div class="app">
    <Toolbar v-if="loaded">
      <template #start>
        <span class="file-name" :title="loaded.name">{{ loaded.name }}</span>
      </template>

      <template #center>
        <div class="group">
          <ToolbarButton
            title="Previous page"
            :disabled="st.currentPage <= 1"
            @click="viewer?.prev()"
          >
            ‹
          </ToolbarButton>
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
          <ToolbarButton
            title="Next page"
            :disabled="st.currentPage >= st.totalPages"
            @click="viewer?.next()"
          >
            ›
          </ToolbarButton>
        </div>

        <div class="group">
          <ToolbarButton title="Zoom out" @click="viewer?.zoomOut()">−</ToolbarButton>
          <span class="muted zoom">{{ st.scalePercent }}%</span>
          <ToolbarButton title="Zoom in" @click="viewer?.zoomIn()">+</ToolbarButton>
          <ToolbarButton title="Fit width" @click="viewer?.fitWidth()">Fit width</ToolbarButton>
          <ToolbarButton title="Fit page" @click="viewer?.fitPage()">Fit page</ToolbarButton>
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
          <ToolbarButton
            title="Previous match"
            :disabled="!st.findCount"
            @click="viewer?.findPrev()"
          >
            ‹
          </ToolbarButton>
          <ToolbarButton title="Next match" :disabled="!st.findCount" @click="viewer?.findNext()">
            ›
          </ToolbarButton>
        </div>
      </template>

      <template #end>
        <ToolbarButton variant="primary" @click="chooseFile">Open File</ToolbarButton>
      </template>
    </Toolbar>

    <main class="content">
      <PdfView
        v-if="loaded"
        ref="viewer"
        :source="loaded"
        @error="onViewError"
        @state="st = $event"
      />

      <EmptyState
        v-else
        title="Open a document"
        hint="Open or drop a PDF file to get started"
        :error="error"
        :dragging="dragging"
        :action-label="loading ? 'Loading…' : 'Open File'"
        @open="chooseFile"
      />
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

.file-name {
  color: var(--vw-fg-muted);
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group {
  display: flex;
  align-items: center;
  gap: var(--vw-space-1);
}

.muted {
  color: var(--vw-fg-muted);
  font-size: var(--vw-fs-md);
}

.zoom {
  min-width: 40px;
  text-align: center;
}

.page-input,
.find-input {
  height: var(--vw-control-h);
  border: 1px solid var(--vw-border-strong);
  border-radius: var(--vw-radius-sm);
  background: var(--vw-bg);
  color: var(--vw-fg);
  font: inherit;
  font-size: var(--vw-fs-sm);
}

.page-input {
  width: 44px;
  text-align: center;
}

.find-input {
  width: 140px;
  padding: 0 var(--vw-space-2);
}

.find-count {
  min-width: 34px;
  text-align: center;
}

.content {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  overflow: hidden;
}
</style>
