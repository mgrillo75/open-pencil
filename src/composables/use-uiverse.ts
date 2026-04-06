import { computed, ref } from 'vue'

import { IS_TAURI } from '@/constants'
import { buildPreviewDoc } from '@/composables/uiverse-data'

import type { UiverseImportedComponent } from '@/composables/uiverse-data'

async function fetchUiversePost(url: string): Promise<UiverseImportedComponent> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<UiverseImportedComponent>('fetch_uiverse_post', { url })
}

export function useUiverseImport() {
  const loading = ref(false)
  const error = ref<string | null>(null)
  const component = ref<UiverseImportedComponent | null>(null)
  const html = ref('')
  const css = ref('')

  const isAvailable = computed(() => IS_TAURI)
  const previewDoc = computed(() => buildPreviewDoc(html.value, css.value, component.value))

  function reset() {
    loading.value = false
    error.value = null
    component.value = null
    html.value = ''
    css.value = ''
  }

  function setImportedComponent(imported: UiverseImportedComponent) {
    component.value = imported
    html.value = imported.html
    css.value = imported.css
    error.value = null
  }

  async function importFromUrl(url: string) {
    const trimmedUrl = url.trim()
    if (!trimmedUrl) {
      error.value = 'Please enter a Uiverse URL.'
      return
    }

    if (!isAvailable.value) {
      error.value = 'Uiverse import is available only in the desktop app.'
      return
    }

    loading.value = true
    error.value = null

    try {
      const imported = await fetchUiversePost(trimmedUrl)
      setImportedComponent(imported)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      error.value = message || 'Failed to import Uiverse component.'
    } finally {
      loading.value = false
    }
  }

  return {
    isAvailable,
    loading,
    error,
    component,
    html,
    css,
    previewDoc,
    setImportedComponent,
    importFromUrl,
    reset
  }
}
