// ---------------------------------------------------------------------------
// Figma Component Set → createVariants() code generation
//
// Given a Figma ComponentSetNode, produces a createVariants()-based component
// definition that maps Figma variant properties to motion variant axes.
// ---------------------------------------------------------------------------

import { convertNode, createContext, generateImports, type ConvertContext } from "./convert.ts"

export interface VariantGenResult {
  code: string
  componentName: string
}

/**
 * Generate a createVariants() component definition from a Figma ComponentSetNode.
 *
 * The component set's variant properties become axes, and each variant's
 * visual differences become motion targets (fill, opacity, scale, etc.).
 */
export async function generateVariantComponent(node: ComponentSetNode): Promise<VariantGenResult | null> {
  const variantProps = extractVariantProperties(node)
  if (variantProps.length === 0) return null

  const componentName = sanitizeName(node.name)

  const defaultChild = node.children[0] as ComponentNode | undefined
  if (!defaultChild) return null

  // Build variant axes by diffing each variant against the default.
  const axes = buildVariantAxes(node, variantProps)

  // Detect if any axis has actual motion-level diffs
  const hasMotionDiffs = axes.some(axis =>
    Object.values(axis.values).some(target => Object.keys(target).length > 0)
  )

  if (hasMotionDiffs) {
    return generateMotionVariant(node, componentName, variantProps, axes, defaultChild)
  } else {
    return generateStructuralVariant(node, componentName, variantProps)
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function extractVariantProperties(node: ComponentSetNode): string[] {
  const defs = node.componentPropertyDefinitions
  return Object.keys(defs).filter(key => defs[key].type === "VARIANT")
}

interface VariantAxis {
  name: string
  values: Record<string, Record<string, unknown>>
}

function buildVariantAxes(
  node: ComponentSetNode,
  variantProps: string[],
): VariantAxis[] {
  const defaultChild = node.children[0] as ComponentNode
  const defaultTarget = extractMotionTarget(defaultChild)

   return variantProps.map(function(prop) {
    var safeName = sanitizeName(prop)
    safeName = safeName[0].toLowerCase() + safeName.slice(1)
    var values: Record<string, Record<string, unknown>> = {}

    for (const child of node.children as readonly ComponentNode[]) {
      const value = child.variantProperties?.[prop]
      if (!value) continue

      const target = extractMotionTarget(child)
      // Only include properties that differ from the default.
      const diff: Record<string, unknown> = {}
      for (const [k, v] of Object.entries(target)) {
        if (JSON.stringify(v) !== JSON.stringify(defaultTarget[k])) {
          diff[k] = v
        }
      }

      if (Object.keys(diff).length > 0) {
        values[value] = diff
      } else {
        // Even if identical, include it so the axis is complete.
        values[value] = {}
      }
    }

    return { name: safeName, values }
  })
}

function extractMotionTarget(node: ComponentNode): Record<string, unknown> {
  const target: Record<string, unknown> = {}

  // Opacity
  if (node.opacity < 0.999) {
    target.opacity = Math.round(node.opacity * 100) / 100
  }

  // Background color
  const fills = node.fills
  if (fills !== figma.mixed && Array.isArray(fills)) {
    for (const f of fills) {
      if (f.type === "SOLID" && f.visible !== false) {
        const { r, g, b } = f.color
        const toHex = (v: number) => Math.round(v * 255).toString(16).padStart(2, "0")
        target.backgroundColor = `#${toHex(r)}${toHex(g)}${toHex(b)}`
        break
      }
    }
  }

  // Corner radius → borderRadius
  if ("cornerRadius" in node && node.cornerRadius !== figma.mixed && node.cornerRadius > 0) {
    target.borderRadius = node.cornerRadius
  }

  // Scale (from Figma scale isn't directly available, but we can detect
  // size changes relative to the component set)
  // Note: not directly mappable from static Figma data, so skip.

  return target
}

function formatTarget(target: Record<string, unknown>): string {
  const entries = Object.entries(target)
    .map(([k, v]) => `${k}: ${typeof v === "string" ? `"${v}"` : v}`)
    .join(", ")
  return `{ ${entries} }`
}

function sanitizeName(name: string): string {
  // Remove spaces, slashes, etc. and PascalCase it.
  return name
    .replace(/[^a-zA-Z0-9]/g, " ")
    .split(" ")
    .filter(Boolean)
    .map(w => w[0].toUpperCase() + w.slice(1))
    .join("")
}

function camelCase(name: string): string {
  const pascal = sanitizeName(name)
  return pascal[0].toLowerCase() + pascal.slice(1)
}

// ---------------------------------------------------------------------------
// Motion variant generator (visual-only diffs between variants)
// ---------------------------------------------------------------------------

async function generateMotionVariant(
  node: ComponentSetNode,
  componentName: string,
  variantProps: string[],
  axes: VariantAxis[],
  defaultChild: ComponentNode,
): Promise<VariantGenResult> {
  const ctx = createContext("fragments")
  const baseJsx = await convertNode(defaultChild, ctx)

  const lines: string[] = []

  const imports = generateImports(ctx.imports)
  lines.push(`import { createVariants, motion, defineIntrinsicComponent } from "@qt-solid/solid"`)
  lines.push(`import type { CanvasRectProps } from "@qt-solid/solid"`)
  if (imports) lines.push(imports)
  lines.push("")

  lines.push(`const Base = motion(defineIntrinsicComponent<CanvasRectProps>("rect"))`)
  lines.push("")

  lines.push(`export const ${componentName} = createVariants(Base, {`)

  const baseTarget = extractMotionTarget(defaultChild)
  if (Object.keys(baseTarget).length > 0) {
    lines.push(`  initial: ${formatTarget(baseTarget)},`)
  }

  lines.push(`  variants: {`)
  for (const axis of axes) {
    lines.push(`    ${axis.name}: {`)
    for (const [value, target] of Object.entries(axis.values)) {
      lines.push(`      ${JSON.stringify(value)}: ${formatTarget(target)},`)
    }
    lines.push(`    },`)
  }
  lines.push(`  },`)

  const defaults = variantProps
    .map(function(vp) {
      const safe = camelCase(vp)
      const defaultVal = defaultChild.variantProperties ? defaultChild.variantProperties[vp] : undefined
      return defaultVal ? `    ${safe}: ${JSON.stringify(defaultVal)}` : null
    })
    .filter(Boolean)
  if (defaults.length > 0) {
    lines.push(`  defaultVariants: {`)
    for (const d of defaults) lines.push(`${d},`)
    lines.push(`  },`)
  }

  lines.push(`  transition: { type: "tween", duration: 0.15, ease: "ease-out" },`)
  lines.push(`})`)
  lines.push("")

  lines.push(...generatePropsInterface(componentName, variantProps, node))

  lines.push("")
  lines.push(`/*`)
  lines.push(` * Base structure (default variant):`)
  lines.push(` *`)
  for (const line of baseJsx.split("\n")) {
    lines.push(` * ${line}`)
  }
  lines.push(` */`)

  return { code: lines.join("\n"), componentName }
}

// ---------------------------------------------------------------------------
// Structural variant generator (conditional rendering per variant)
// ---------------------------------------------------------------------------

async function generateStructuralVariant(
  node: ComponentSetNode,
  componentName: string,
  variantProps: string[],
): Promise<VariantGenResult> {
  const lines: string[] = []

  // Generate JSX for each variant child
  const variantBodies: Array<{ props: Record<string, string>; jsx: string; imports: string }> = []
  for (const child of node.children as readonly ComponentNode[]) {
    const ctx = createContext("fragments")
    const jsx = await convertNode(child, ctx)
    const imports = generateImports(ctx.imports)
    const vProps = child.variantProperties ?? {}
    variantBodies.push({ props: vProps, jsx, imports })
  }

  // Collect all imports
  const allImports = new Set<string>()
  for (const v of variantBodies) {
    if (v.imports) {
      for (const line of v.imports.split("\n")) {
        allImports.add(line)
      }
    }
  }

  lines.push(`import { Show, Switch, Match } from "solid-js"`)
  for (const imp of allImports) {
    if (imp) lines.push(imp)
  }
  lines.push("")

  // Props interface
  lines.push(...generatePropsInterface(componentName, variantProps, node))
  lines.push("")

  // Component function
  lines.push(`export function ${componentName}(props: ${componentName}Props) {`)

  if (variantProps.length === 1) {
    // Single axis — use Switch/Match
    const axis = variantProps[0]
    const propName = camelCase(axis)
    lines.push(`  return (`)
    lines.push(`    <Switch>`)
    for (const v of variantBodies) {
      const value = v.props[axis]
      if (!value) continue
      lines.push(`      <Match when={props.${propName} === ${JSON.stringify(value)}}>`)
      for (const line of v.jsx.split("\n")) {
        lines.push(`        ${line}`)
      }
      lines.push(`      </Match>`)
    }
    lines.push(`    </Switch>`)
    lines.push(`  )`)
  } else {
    // Multi-axis — use nested conditions. Emit a lookup + Switch on compound key.
    const propNames = variantProps.map(camelCase)
    const keyExpr = propNames.map(p => `props.${p}`).join(` + ":" + `)
    lines.push(`  const key = () => ${keyExpr}`)
    lines.push(`  return (`)
    lines.push(`    <Switch>`)
    for (const v of variantBodies) {
      const keyVal = variantProps.map(vp => v.props[vp] ?? "").join(":")
      lines.push(`      <Match when={key() === ${JSON.stringify(keyVal)}}>`)
      for (const line of v.jsx.split("\n")) {
        lines.push(`        ${line}`)
      }
      lines.push(`      </Match>`)
    }
    lines.push(`    </Switch>`)
    lines.push(`  )`)
  }

  lines.push(`}`)

  return { code: lines.join("\n"), componentName }
}

// ---------------------------------------------------------------------------
// Shared: generate props interface
// ---------------------------------------------------------------------------

function generatePropsInterface(
  componentName: string,
  variantProps: string[],
  node: ComponentSetNode,
): string[] {
  const lines: string[] = []
  const propTypes = variantProps.map(function(vp) {
    const safe = camelCase(vp)
    const values = new Set<string>()
    for (const child of node.children as readonly ComponentNode[]) {
      const v = child.variantProperties ? child.variantProperties[vp] : undefined
      if (v) values.add(v)
    }
    return `  ${safe}?: ${[...values].map(v => JSON.stringify(v)).join(" | ")}`
  })
  lines.push(`export interface ${componentName}Props {`)
  for (const pt of propTypes) lines.push(pt)
  lines.push(`}`)
  return lines
}
