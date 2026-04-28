// ---------------------------------------------------------------------------
// Component manifest — maps Figma component names to qt-solid imports + props
// ---------------------------------------------------------------------------

export interface ComponentMapping {
  /** Import specifier */
  from: string
  /** Exported component name */
  name: string
  /** Figma variant property → prop mapping */
  variantProps?: Record<string, Record<string, Record<string, unknown>>>
  /** How to handle children: "text" = use text content, "slot" = recurse */
  children?: "text" | "slot"
  /** Static props always applied */
  staticProps?: Record<string, unknown>
}

export const COMPONENT_MANIFEST: Record<string, ComponentMapping> = {}

// ---------------------------------------------------------------------------
// Name sanitization (shared with variant-gen)
// ---------------------------------------------------------------------------

function sanitizeComponentName(name: string): string {
  return name
    .replace(/[^a-zA-Z0-9]/g, " ")
    .split(" ")
    .filter(Boolean)
    .map(w => w[0].toUpperCase() + w.slice(1))
    .join("")
}

// ---------------------------------------------------------------------------
// Match: static manifest → auto-derive from ComponentSet name
// ---------------------------------------------------------------------------

/** Try to match a Figma node to a known component. */
export async function matchComponent(node: SceneNode): Promise<ComponentMapping | null> {
  if (node.type !== "INSTANCE") return null
  const instance = node as InstanceNode
  const mainComponent = await instance.getMainComponentAsync()
  if (!mainComponent) return null

  const parent = mainComponent.parent
  const setName = parent && parent.type === "COMPONENT_SET"
    ? parent.name
    : mainComponent.name

  // 1. Static manifest lookup (explicit overrides)
  if (COMPONENT_MANIFEST[setName]) {
    return COMPONENT_MANIFEST[setName]
  }

  // 2. Auto-derive: assume a same-named generated component exists
  //    Convention: ComponentSet "Button" → import { Button } from "./Button"
  const componentName = sanitizeComponentName(setName)
  if (!componentName) return null

  return {
    from: `./${componentName}`,
    name: componentName,
    children: "slot",
  }
}

/** Extract resolved variant props for a matched component instance. */
export function resolveVariantProps(
  node: InstanceNode,
  mapping: ComponentMapping,
): Record<string, unknown> {
  const result: Record<string, unknown> = {}

  if (mapping.staticProps) {
    Object.assign(result, mapping.staticProps)
  }

  if (!mapping.variantProps) return result

  const variantProperties = node.variantProperties
  if (!variantProperties) return result

  for (const [axis, valueMap] of Object.entries(mapping.variantProps)) {
    const figmaValue = variantProperties[axis]
    if (figmaValue && valueMap[figmaValue]) {
      Object.assign(result, valueMap[figmaValue])
    }
  }

  return result
}
