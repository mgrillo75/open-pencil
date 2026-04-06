<script setup lang="ts">
import { computed, ref, watch } from 'vue'

import { useUiverseImport } from '@/composables/use-uiverse'
import { toImportedComponent, toUiverseNodeData } from '@/composables/uiverse-data'
import { rasterizeUiversePreviewFrame } from '@/composables/use-uiverse-preview'
import { toast } from '@/composables/use-toast'
import { useEditorStore } from '@/stores/editor'

const store = useEditorStore()
const uiverse = useUiverseImport()

const urlInput = ref('')
const lastImportedUrl = ref('')
const placing = ref(false)
const previewFrame = ref<HTMLIFrameElement | null>(null)

const isAvailable = computed(() => uiverse.isAvailable.value)
const loading = computed(() => uiverse.loading.value)
const errorMessage = computed(() => uiverse.error.value ?? '')
const component = computed(() => uiverse.component.value)
const previewDoc = computed(() => uiverse.previewDoc.value)
const htmlModel = uiverse.html
const cssModel = uiverse.css

const selectedUiverseBinding = computed(() => {
  void store.state.sceneVersion
  return store.getSelectedUiverseBinding()
})

const hasImportedComponent = computed(() => Boolean(component.value))
const canImport = computed(() => isAvailable.value && !loading.value && urlInput.value.trim().length > 0)
const canPlace = computed(
  () => isAvailable.value && hasImportedComponent.value && !loading.value && !placing.value
)
const placeButtonLabel = computed(() =>
  selectedUiverseBinding.value ? 'Update Selected' : 'Add to Canvas'
)

const authorLabel = computed(() => {
  if (!component.value) return ''
  return component.value.authorName ?? component.value.authorUsername ?? 'Unknown'
})

const sourceHref = computed(() => {
  if (component.value?.sourceWebsite) return component.value.sourceWebsite
  if (component.value?.url) return component.value.url
  if (lastImportedUrl.value) return lastImportedUrl.value
  return ''
})

watch(
  () => selectedUiverseBinding.value?.frameId ?? null,
  () => {
    const selected = selectedUiverseBinding.value
    if (!selected) return
    const imported = toImportedComponent(selected.data)
    uiverse.setImportedComponent(imported)
    urlInput.value = imported.url
    lastImportedUrl.value = imported.url
  },
  { immediate: true }
)

async function onImport() {
  const next = urlInput.value.trim()
  if (!next || loading.value || !isAvailable.value) return
  await uiverse.importFromUrl(next)
  if (!errorMessage.value) lastImportedUrl.value = next
}

function resetWorkspace() {
  uiverse.reset()
  urlInput.value = ''
  lastImportedUrl.value = ''
}

async function syncToCanvas() {
  const frame = previewFrame.value
  const imported = component.value
  if (!frame || !imported) return

  placing.value = true
  try {
    const snapshot = await rasterizeUiversePreviewFrame(frame)
    const nodeId = await store.placeUiverseSnapshot({
      data: toUiverseNodeData(
        imported,
        htmlModel.value,
        cssModel.value,
        selectedUiverseBinding.value?.data.lastSnapshotHash ?? null
      ),
      pngBytes: snapshot.data,
      width: snapshot.width,
      height: snapshot.height
    })
    if (nodeId) {
      store.select([nodeId])
      toast.show(
        selectedUiverseBinding.value
          ? 'Updated Uiverse component on canvas'
          : 'Added Uiverse component to canvas'
      )
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Failed to place Uiverse component.'
    toast.show(message, 'error')
  } finally {
    placing.value = false
  }
}
</script>

<template>
  <div data-test-id="uiverse-workspace" class="flex min-h-0 flex-1 flex-col">
    <div class="flex shrink-0 flex-col gap-2 border-b border-border px-3 py-2">
      <div class="flex items-center justify-between">
        <p class="text-[11px] font-semibold tracking-wide text-muted uppercase">Uiverse</p>
        <button
          class="rounded px-1.5 py-0.5 text-[11px] text-muted hover:bg-hover hover:text-surface"
          :disabled="loading || placing"
          @click="resetWorkspace"
        >
          Reset
        </button>
      </div>
      <form class="flex items-center gap-2" @submit.prevent="onImport">
        <input
          v-model="urlInput"
          data-test-id="uiverse-url-input"
          type="url"
          placeholder="https://uiverse.io/username/friendly-id"
          class="h-8 min-w-0 flex-1 rounded border border-border bg-input px-2 text-xs text-surface outline-none focus:border-accent"
        />
        <button
          data-test-id="uiverse-import-button"
          type="submit"
          class="inline-flex h-8 items-center gap-1 rounded border border-border px-2.5 text-xs text-surface transition-colors hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!canImport"
        >
          <icon-lucide-loader-circle v-if="loading" class="size-3 animate-spin" />
          <icon-lucide-download v-else class="size-3" />
          Import
        </button>
      </form>
      <p v-if="!isAvailable" class="text-[11px] text-muted">
        Uiverse import is available in desktop mode (Tauri) only.
      </p>
      <p v-else class="text-[11px] text-muted">
        Import a Uiverse URL, preview it, then add it to the canvas as a persisted snapshot-backed layer.
      </p>
      <p
        v-if="selectedUiverseBinding"
        data-test-id="uiverse-selected-node"
        class="text-[11px] text-accent"
      >
        Editing the currently selected Uiverse layer on canvas.
      </p>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto p-3">
      <div
        v-if="errorMessage"
        data-test-id="uiverse-error"
        class="mb-3 rounded border border-red-500/40 bg-red-500/10 px-2.5 py-2 text-xs text-red-300"
      >
        {{ errorMessage }}
      </div>

      <div
        v-if="hasImportedComponent"
        class="mb-3 flex flex-wrap items-center gap-2 rounded border border-border bg-panel-2/50 px-2.5 py-2 text-[11px] text-muted"
      >
        <span class="font-medium text-surface">{{
          component?.title || component?.friendlyId || 'Imported component'
        }}</span>
        <span>by {{ authorLabel }}</span>
        <span v-if="component?.type" class="rounded bg-hover px-1.5 py-0.5 uppercase">{{
          component.type
        }}</span>
        <button
          data-test-id="uiverse-canvas-button"
          class="inline-flex items-center gap-1 rounded border border-border px-2 py-1 text-[11px] text-surface transition-colors hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!canPlace"
          @click="syncToCanvas"
        >
          <icon-lucide-loader-circle v-if="placing" class="size-3 animate-spin" />
          <icon-lucide-image-plus v-else class="size-3" />
          {{ placeButtonLabel }}
        </button>
        <a
          v-if="sourceHref"
          class="ml-auto inline-flex items-center gap-1 text-accent hover:underline"
          :href="sourceHref"
          target="_blank"
          rel="noreferrer"
        >
          Source
          <icon-lucide-external-link class="size-3" />
        </a>
      </div>

      <div v-if="hasImportedComponent" class="grid min-h-0 gap-3 xl:grid-cols-[1.1fr_1fr]">
        <section class="flex min-h-0 flex-col rounded border border-border bg-panel-2/40">
          <header class="flex shrink-0 items-center justify-between border-b border-border px-2.5 py-1.5">
            <span class="text-[11px] text-muted">Live Preview</span>
            <span class="text-[11px] text-muted">Snapshot source</span>
          </header>
          <div class="min-h-[240px] flex-1 p-2">
            <iframe
              ref="previewFrame"
              data-test-id="uiverse-preview-frame"
              :srcdoc="previewDoc"
              sandbox="allow-forms allow-modals allow-pointer-lock allow-same-origin allow-scripts"
              class="size-full rounded border border-border bg-white"
              title="Uiverse component preview"
            />
          </div>
        </section>

        <section class="flex min-h-0 flex-col gap-3">
          <div class="rounded border border-border bg-panel-2/40">
            <div class="border-b border-border px-2.5 py-1.5 text-[11px] text-muted">HTML</div>
            <textarea
              v-model="htmlModel"
              data-test-id="uiverse-html-editor"
              class="h-56 w-full resize-y bg-transparent px-2.5 py-2 font-mono text-xs text-surface outline-none"
              spellcheck="false"
            />
          </div>

          <div class="rounded border border-border bg-panel-2/40">
            <div class="border-b border-border px-2.5 py-1.5 text-[11px] text-muted">CSS</div>
            <textarea
              v-model="cssModel"
              data-test-id="uiverse-css-editor"
              class="h-56 w-full resize-y bg-transparent px-2.5 py-2 font-mono text-xs text-surface outline-none"
              spellcheck="false"
            />
          </div>
        </section>
      </div>

      <div
        v-else-if="!loading && !errorMessage"
        data-test-id="uiverse-empty"
        class="flex min-h-[200px] items-center justify-center rounded border border-dashed border-border px-4 text-center text-xs text-muted"
      >
        Paste a Uiverse URL and import to start editing HTML/CSS with live preview.
      </div>
    </div>
  </div>
</template>
