import { transform } from 'sucrase'

import * as React from './mini-react'
import { renderTree, type RenderResult } from './renderer'
import { resolveToTree, type TreeNode } from './tree'

import type { SceneGraph } from '../scene-graph'

/**
 * Build a component function from a JSX string using sucrase.
 * Works in both Node/Bun and the browser (no native bindings).
 */
export function buildComponent(jsxString: string): () => unknown {
  const code = `
    const h = React.createElement
    const Frame = 'frame', Text = 'text', Rectangle = 'rectangle', Ellipse = 'ellipse'
    const Line = 'line', Star = 'star', Polygon = 'polygon', Vector = 'vector'
    const Group = 'group', Section = 'section', View = 'frame', Rect = 'rectangle'
    const TONES = {
      neutral: { bg: '#111827', stroke: '#334155', text: '#E5E7EB', muted: '#94A3B8', accent: '#64748B' },
      ok: { bg: '#0F1E17', stroke: '#1F7A45', text: '#DCFCE7', muted: '#86EFAC', accent: '#22C55E' },
      warn: { bg: '#211806', stroke: '#A16207', text: '#FEF3C7', muted: '#FCD34D', accent: '#F59E0B' },
      danger: { bg: '#220D0D', stroke: '#B91C1C', text: '#FEE2E2', muted: '#FCA5A5', accent: '#EF4444' },
      info: { bg: '#0B1730', stroke: '#1D4ED8', text: '#DBEAFE', muted: '#93C5FD', accent: '#3B82F6' }
    }
    const tone = (value = 'neutral') => TONES[value] || TONES.neutral
    const Screen = ({
      name = 'Screen',
      w: screenWidth = 1600,
      h: screenHeight = 900,
      bg = '#0B1220',
      p = 24,
      gap = 24,
      children,
      ...rest
    }) => h(Frame, { name, w: screenWidth, h: screenHeight, bg, flex: 'col', gap, p, ...rest }, children)
    const VStack = ({ gap = 12, children, ...rest }) =>
      h(Frame, { flex: 'col', gap, ...rest }, children)
    const HStack = ({ gap = 12, items = 'center', justify = 'start', children, ...rest }) =>
      h(Frame, { flex: 'row', gap, items, justify, ...rest }, children)
    const Panel = ({
      name = 'Panel',
      bg = '#111827',
      stroke = '#1F2937',
      rounded = 16,
      p = 20,
      gap = 16,
      flex = 'col',
      children,
      ...rest
    }) => h(Frame, { name, bg, stroke, rounded, p, gap, flex, ...rest }, children)
    const StatusBadge = ({ label, value, tone: badgeTone = 'ok', children, ...rest }) => {
      const colors = tone(badgeTone)
      return h(
        HStack,
        { gap: 10, bg: colors.bg, stroke: colors.stroke, rounded: 999, px: 14, py: 10, ...rest },
        [
          h(Ellipse, { w: 10, h: 10, bg: colors.accent }),
          h(VStack, { gap: 2 }, [
            h(Text, { size: 12, weight: 'bold', color: colors.text }, label || children),
            value ? h(Text, { size: 12, color: colors.muted }, value) : null
          ])
        ]
      )
    }
    const MetricRow = ({ label, value, tone: metricTone = 'neutral', ...rest }) => {
      const colors = tone(metricTone)
      const valueColor = metricTone === 'neutral' ? '#F8FAFC' : colors.accent
      return h(
        HStack,
        { justify: 'between', items: 'center', bg: '#0F172A', rounded: 10, px: 12, py: 10, ...rest },
        [
          h(Text, { size: 13, weight: 'medium', color: '#94A3B8' }, label),
          h(Text, { size: 18, weight: 'bold', color: valueColor }, value)
        ]
      )
    }
    const ActionButton = ({
      label,
      tone: buttonTone = 'neutral',
      kind = 'solid',
      children,
      ...rest
    }) => {
      const colors = tone(buttonTone)
      const bg = kind === 'outline' ? '#0F172A' : colors.accent
      const stroke = kind === 'outline' ? colors.stroke : colors.accent
      const textColor = kind === 'outline' ? colors.text : '#F8FAFC'
      return h(
        Frame,
        { flex: 'row', justify: 'center', items: 'center', h: 44, rounded: 10, bg, stroke, ...rest },
        h(Text, { size: 14, weight: 'bold', color: textColor }, label || children)
      )
    }
    const ModeCard = ({
      title,
      subtitle,
      tone: cardTone = 'neutral',
      state = 'READY',
      primary = 'Apply',
      secondary = 'Details',
      children,
      ...rest
    }) => {
      const colors = tone(cardTone)
      return h(
        Panel,
        { bg: '#101826', stroke: colors.stroke, gap: 16, p: 20, ...rest },
        [
          h(HStack, { justify: 'between', items: 'center' }, [
            h(Text, { size: 22, weight: 'bold', color: '#F8FAFC' }, title),
            h(StatusBadge, { label: state, tone: cardTone })
          ]),
          subtitle ? h(Text, { size: 14, color: '#94A3B8' }, subtitle) : null,
          children,
          h(HStack, { gap: 12 }, [
            primary ? h(ActionButton, { label: primary, tone: cardTone, w: 'fill' }) : null,
            secondary ? h(ActionButton, { label: secondary, tone: cardTone, kind: 'outline', w: 'fill' }) : null
          ])
        ]
      )
    }
    const RuleList = ({ title = 'Rules', items = [], tone: listTone = 'info', ...rest }) => {
      const colors = tone(listTone)
      return h(
        Panel,
        { gap: 12, ...rest },
        [
          h(Text, { size: 16, weight: 'bold', color: colors.text }, title),
          ...(items || []).map((item) =>
            h(HStack, { gap: 10, items: 'start' }, [
              h(Ellipse, { w: 8, h: 8, bg: colors.accent }),
              h(Text, { size: 14, color: '#CBD5E1' }, item)
            ])
          )
        ]
      )
    }
    return function Component() { return ${jsxString.trim()} }
  `

  const result = transform(code, {
    transforms: ['jsx'],
    jsxPragma: 'h',
    production: true
  })

  return new Function('React', result.code)(React) as () => unknown
}

interface RenderJSXOptions {
  x?: number
  y?: number
  parentId?: string
}

/**
 * Render a JSX string into the scene graph.
 * Works in both Node/Bun and the browser.
 */
export function renderJSX(
  graph: SceneGraph,
  jsxString: string,
  options?: RenderJSXOptions
): RenderResult {
  const Component = buildComponent(jsxString)
  const element = React.createElement(Component, null)
  const tree = resolveToTree(element)

  if (!tree) {
    throw new Error('JSX must return a Figma element (Frame, Text, etc)')
  }

  return renderTree(graph, tree, options)
}

/**
 * Render a pre-built TreeNode into the scene graph.
 */
export function renderTreeNode(
  graph: SceneGraph,
  tree: TreeNode,
  options?: RenderJSXOptions
): RenderResult {
  return renderTree(graph, tree, options)
}
