const PREVIEW_STAGE_SELECTOR = '[data-uiverse-preview-stage="true"]'
const XHTML_NAMESPACE = 'http://www.w3.org/1999/xhtml'

interface UiversePreviewSnapshot {
  data: Uint8Array
  width: number
  height: number
}

function escapeXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;')
}

function serializeStyleSheets(doc: Document): string {
  return Array.from(doc.querySelectorAll('style'))
    .map((node) => node.textContent ?? '')
    .join('\n')
}

function normalizeCloneState(source: Element, clone: Element) {
  const sourceElements = [source, ...Array.from(source.querySelectorAll('*'))]
  const cloneElements = [clone, ...Array.from(clone.querySelectorAll('*'))]

  for (let i = 0; i < Math.min(sourceElements.length, cloneElements.length); i++) {
    const sourceElement = sourceElements[i]
    const cloneElement = cloneElements[i]

    if (sourceElement instanceof HTMLInputElement && cloneElement instanceof HTMLInputElement) {
      if (sourceElement.checked) cloneElement.setAttribute('checked', '')
      else cloneElement.removeAttribute('checked')

      cloneElement.setAttribute('value', sourceElement.value)
    } else if (
      sourceElement instanceof HTMLTextAreaElement &&
      cloneElement instanceof HTMLTextAreaElement
    ) {
      cloneElement.textContent = sourceElement.value
    } else if (
      sourceElement instanceof HTMLOptionElement &&
      cloneElement instanceof HTMLOptionElement
    ) {
      if (sourceElement.selected) cloneElement.setAttribute('selected', '')
      else cloneElement.removeAttribute('selected')
    }
  }
}

function serializeStageToXhtml(stage: HTMLElement): string {
  const clone = stage.cloneNode(true) as HTMLElement
  normalizeCloneState(stage, clone)
  clone.setAttribute('xmlns', XHTML_NAMESPACE)
  return new XMLSerializer().serializeToString(clone)
}

function waitForNextPaint(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  })
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image()
    image.decoding = 'async'
    image.onload = () => resolve(image)
    image.onerror = () => reject(new Error('Failed to load Uiverse snapshot image.'))
    image.src = url
  })
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) resolve(blob)
      else reject(new Error('Failed to encode Uiverse snapshot image.'))
    }, 'image/png')
  })
}

export async function rasterizeUiversePreviewFrame(
  frame: HTMLIFrameElement
): Promise<UiversePreviewSnapshot> {
  await waitForNextPaint()

  const doc = frame.contentDocument
  if (!doc) throw new Error('Uiverse preview is not ready yet.')

  const stage = doc.querySelector<HTMLElement>(PREVIEW_STAGE_SELECTOR)
  if (!stage) throw new Error('Uiverse preview stage is missing.')

  const width = Math.max(1, Math.ceil(stage.scrollWidth || stage.getBoundingClientRect().width))
  const height = Math.max(1, Math.ceil(stage.scrollHeight || stage.getBoundingClientRect().height))
  const styles = serializeStyleSheets(doc)
  const serializedStage = serializeStageToXhtml(stage)

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
  <foreignObject width="100%" height="100%">
    <div xmlns="${XHTML_NAMESPACE}" style="width:${width}px;height:${height}px;">
      <style>${escapeXml(styles)}</style>
      ${serializedStage}
    </div>
  </foreignObject>
</svg>`
  const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`

  const image = await loadImage(url)
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height

  const context = canvas.getContext('2d')
  if (!context) throw new Error('Failed to initialize Uiverse snapshot canvas.')

  context.drawImage(image, 0, 0, width, height)
  const blob = await canvasToBlob(canvas)
  const data = new Uint8Array(await blob.arrayBuffer())
  return { data, width, height }
}
