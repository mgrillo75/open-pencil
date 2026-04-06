import type { SceneNode } from '@open-pencil/core'

export const UIVERSE_PLUGIN_DATA_KEY = 'open-pencil:uiverse'

export interface UiverseImportedComponent {
  url: string
  id: string
  username: string
  friendlyId: string
  type: string | null
  theme: string | null
  backgroundColor: string | null
  version: number
  title: string | null
  authorName: string | null
  authorUsername: string | null
  sourceWebsite: string | null
  html: string
  css: string
}

export interface UiverseNodeData extends UiverseImportedComponent {
  lastSnapshotHash: string | null
}

interface UiversePreviewSource {
  backgroundColor?: string | null
  theme?: string | null
}

export interface UiverseSelectionBinding {
  frameId: string
  previewNodeId: string | null
  data: UiverseNodeData
}

export function resolvePreviewBackground(component: UiversePreviewSource | null): string {
  const explicitBackground = component?.backgroundColor?.trim()
  if (explicitBackground) return explicitBackground

  const theme = component?.theme?.trim().toLowerCase()
  return theme === 'dark' ? '#212121' : '#ffffff'
}

export function buildPreviewDoc(
  html: string,
  css: string,
  component: UiversePreviewSource | null
): string {
  const background = resolvePreviewBackground(component)

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light dark;
      }

      *,
      *::before,
      *::after {
        box-sizing: border-box;
      }

      body {
        margin: 0;
        min-height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        background: ${background};
        overflow: auto;
      }

      .uiverse-preview-stage {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 24px;
        background: ${background};
      }

      .uiverse-preview-center {
        max-width: 100%;
        max-height: 100%;
        display: inline-flex;
        align-items: center;
        justify-content: center;
      }

${css}
    </style>
  </head>
  <body>
    <div class="uiverse-preview-stage" data-uiverse-preview-stage="true">
      <div class="uiverse-preview-center">
${html}
      </div>
    </div>
  </body>
</html>`
}

export function toUiverseNodeData(
  component: UiverseImportedComponent,
  html: string,
  css: string,
  lastSnapshotHash: string | null
): UiverseNodeData {
  return {
    ...component,
    html,
    css,
    lastSnapshotHash
  }
}

export function toImportedComponent(data: UiverseNodeData): UiverseImportedComponent {
  const { lastSnapshotHash: _lastSnapshotHash, ...component } = data
  return component
}

export function serializeUiverseNodeData(data: UiverseNodeData): string {
  return JSON.stringify(data)
}

export function parseUiverseNodeData(value: string | null | undefined): UiverseNodeData | null {
  if (!value) return null

  try {
    const parsed = JSON.parse(value) as Partial<UiverseNodeData>
    if (typeof parsed.url !== 'string') return null
    if (typeof parsed.id !== 'string') return null
    if (typeof parsed.username !== 'string') return null
    if (typeof parsed.friendlyId !== 'string') return null
    if (typeof parsed.version !== 'number') return null
    if (typeof parsed.html !== 'string') return null
    if (typeof parsed.css !== 'string') return null

    return {
      url: parsed.url,
      id: parsed.id,
      username: parsed.username,
      friendlyId: parsed.friendlyId,
      type: typeof parsed.type === 'string' ? parsed.type : null,
      theme: typeof parsed.theme === 'string' ? parsed.theme : null,
      backgroundColor:
        typeof parsed.backgroundColor === 'string' ? parsed.backgroundColor : null,
      version: parsed.version,
      title: typeof parsed.title === 'string' ? parsed.title : null,
      authorName: typeof parsed.authorName === 'string' ? parsed.authorName : null,
      authorUsername:
        typeof parsed.authorUsername === 'string' ? parsed.authorUsername : null,
      sourceWebsite:
        typeof parsed.sourceWebsite === 'string' ? parsed.sourceWebsite : null,
      html: parsed.html,
      css: parsed.css,
      lastSnapshotHash:
        typeof parsed.lastSnapshotHash === 'string' ? parsed.lastSnapshotHash : null
    }
  } catch {
    return null
  }
}

export function getUiverseNodeData(node: Pick<SceneNode, 'pluginData'> | null | undefined) {
  return parseUiverseNodeData(node?.pluginData?.[UIVERSE_PLUGIN_DATA_KEY])
}
