import { createAnthropic } from '@ai-sdk/anthropic'
import { createGoogleGenerativeAI } from '@ai-sdk/google'
import { createOpenAI } from '@ai-sdk/openai'
import { Chat } from '@ai-sdk/vue'
import { createOpenRouter } from '@openrouter/ai-sdk-provider'
import { useLocalStorage } from '@vueuse/core'
import { DirectChatTransport, ToolLoopAgent } from 'ai'
import dedent from 'dedent'
import { computed, ref, watch } from 'vue'

import { createAITools } from '@/ai/tools'
import { useEditorStore } from '@/stores/editor'
import { AI_PROVIDERS, DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER } from '@open-pencil/core'

import type { AIProviderID } from '@open-pencil/core'
import type { LanguageModel, UIMessage } from 'ai'

export { AI_PROVIDERS } from '@open-pencil/core'
export type { AIProviderDef, AIProviderID, ModelOption } from '@open-pencil/core'

const STORAGE_PREFIX = 'open-pencil:'
const LEGACY_KEY_STORAGE = `${STORAGE_PREFIX}openrouter-api-key`

function keyStorageKey(id: string) {
  return `${STORAGE_PREFIX}ai-key:${id}`
}

function migrateLegacyStorage() {
  const legacyKey = localStorage.getItem(LEGACY_KEY_STORAGE)
  if (legacyKey) {
    localStorage.setItem(keyStorageKey('openrouter'), legacyKey)
    localStorage.removeItem(LEGACY_KEY_STORAGE)
    if (!localStorage.getItem(`${STORAGE_PREFIX}ai-provider`)) {
      localStorage.setItem(`${STORAGE_PREFIX}ai-provider`, 'openrouter')
    }
  }
}

if (typeof window !== 'undefined') migrateLegacyStorage()

const SYSTEM_PROMPT = dedent`
  You are a design assistant inside OpenPencil, a Figma-like design editor.
  Help users create and modify designs. Be concise and direct.
  When describing changes, use specific design terminology.

  Use the render tool with JSX as the primary way to create designs.
  JSX supports full JavaScript expressions (map, ternaries, Array.from, etc.).
  Available tags: Frame, Text, Rectangle, Ellipse, Line, Star, Polygon, Group, Section.
  Available helper components: Screen, Panel, HStack, VStack, StatusBadge, MetricRow,
  ActionButton, ModeCard, RuleList.
  Common props: name, w, h, x, y, bg (hex color), stroke, rounded, opacity, rotate.
  Layout: flex="row"|"col", gap, justify, items, p, px, py, pt/pr/pb/pl, wrap.
  Text: size, weight, color, font, textAlign.
  Sizing: w/h accept numbers (px) or "hug"/"fill".

  Colors are hex strings (#ff0000). Coordinates use canvas space - (0, 0) is top-left.
  Always use tools to make changes. After creating nodes, briefly describe what you did.
  Use create_shape + set_layout only for simple single nodes; prefer render for layouts.

  First convert the user's request into a clean interface hierarchy:
  header/status, primary controls, secondary information, and notes/help.
  Design the interface instead of copying the user's full requirement text into the UI.
  Summarize long requirements into short labels, concise status text, and separate explanatory panels.

  Favor disciplined layout:
  use one main frame, consistent padding, aligned columns, equal-width cards when appropriate,
  and a limited typography scale with clear hierarchy.
  Prefer auto-layout over manual positioning for most UI.
  For layout-heavy requests, generate the full composition in a single render call with explicit sizes.
  Prefer the helper components for dashboards, admin screens, and HMIs instead of building every card from raw tags.

  Never put long paragraphs into narrow cards, buttons, badges, or labels.
  If text would wrap into awkward stacked words, shorten the copy, widen the container,
  or move the detail into a dedicated notes or rules panel.
  Controls should have short action-oriented labels. Detailed behavior belongs in supporting text areas.
  On desktop screens, keep body text at 14px or larger and avoid text below 12px.
  Keep side panels concise: a few short rules or metrics per panel, not dense paragraphs.

  For dashboards, HMIs, industrial screens, and control panels:
  prioritize readability, operator scanning, status clarity, and clear separation between
  controls, live state, alarms, and operating rules.
  Use short labels, large readable values, obvious status colors, and structured sections.
  Start from Screen, Panel, ModeCard, MetricRow, StatusBadge, and RuleList unless the user asks for a custom layout language.

  Avoid placeholder clutter, decorative noise, and oversized empty containers.
  If the user asks for a medium or high-fidelity screen, produce a complete balanced layout
  rather than a sparse wireframe.
`

const providerID = useLocalStorage<AIProviderID>(
  `${STORAGE_PREFIX}ai-provider`,
  DEFAULT_AI_PROVIDER
)
const apiKeyStorageKey = computed(() => keyStorageKey(providerID.value))
const apiKey = useLocalStorage(apiKeyStorageKey, '')
const modelID = useLocalStorage(`${STORAGE_PREFIX}ai-model`, DEFAULT_AI_MODEL)
const customBaseURL = useLocalStorage(`${STORAGE_PREFIX}ai-base-url`, '')
const customModelID = useLocalStorage(`${STORAGE_PREFIX}ai-custom-model`, '')
const activeTab = ref<'design' | 'ai'>('design')

const providerDef = computed(
  () => AI_PROVIDERS.find((p) => p.id === providerID.value) ?? AI_PROVIDERS[0]
)

const isConfigured = computed(() => {
  if (!apiKey.value) return false
  if (providerID.value === 'openai-compatible' && !customBaseURL.value) return false
  return true
})

watch(providerID, (id) => {
  const def = AI_PROVIDERS.find((p) => p.id === id)
  if (def?.defaultModel) {
    modelID.value = def.defaultModel
  }
  resetChat()
})

watch(modelID, () => resetChat())
watch(customModelID, () => resetChat())

function setAPIKey(key: string) {
  apiKey.value = key
}

function createModel(): LanguageModel {
  const key = apiKey.value
  const effectiveModelID =
    providerID.value === 'openai-compatible' ? customModelID.value : modelID.value

  switch (providerID.value) {
    case 'openrouter': {
      const openrouter = createOpenRouter({
        apiKey: key,
        headers: {
          'X-OpenRouter-Title': 'OpenPencil',
          'HTTP-Referer': 'https://github.com/open-pencil/open-pencil'
        }
      })
      return openrouter(effectiveModelID)
    }
    case 'anthropic': {
      const anthropic = createAnthropic({ apiKey: key })
      return anthropic(effectiveModelID)
    }
    case 'openai': {
      const openai = createOpenAI({ apiKey: key })
      return openai(effectiveModelID)
    }
    case 'google': {
      const google = createGoogleGenerativeAI({ apiKey: key })
      return google(effectiveModelID)
    }
    case 'openai-compatible': {
      const custom = createOpenAI({
        apiKey: key,
        baseURL: customBaseURL.value
      })
      return custom(effectiveModelID)
    }
  }
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- test-only mock transports don't implement full generics
let overrideTransport: (() => any) | null = null

let chat: Chat<UIMessage> | null = null

function createTransport() {
  if (overrideTransport) return overrideTransport()

  const tools = createAITools(useEditorStore())

  const agent = new ToolLoopAgent({
    model: createModel(),
    instructions: SYSTEM_PROMPT,
    tools
  })

  return new DirectChatTransport({ agent })
}

function ensureChat(): Chat<UIMessage> | null {
  if (!isConfigured.value) return null
  if (!chat) {
    chat = new Chat<UIMessage>({
      transport: createTransport()
    })
  }
  return chat
}

function resetChat() {
  chat = null
}

if (typeof window !== 'undefined') {
  window.__OPEN_PENCIL_SET_TRANSPORT__ = (factory) => {
    overrideTransport = factory
  }
}

export function useAIChat() {
  return {
    providerID,
    providerDef,
    apiKey,
    setAPIKey,
    modelID,
    customBaseURL,
    customModelID,
    activeTab,
    isConfigured,
    ensureChat,
    resetChat
  }
}
