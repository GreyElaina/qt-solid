import type { PropFieldSchema, VariantAxisSchema } from "./protocol.ts"

export interface PreviewEntryOptions {
  /** Absolute path to the source file containing the component */
  filePath: string
  /** Named export to preview. If "default", uses default import */
  exportName: string
  /** Window title override */
  title?: string
  /** Window dimensions */
  width?: number
  height?: number
  /** Enable IPC-based props control runtime */
  propsControl?: boolean
  /** Pre-extracted props schema */
  propsSchema?: PropFieldSchema[]
  /** Pre-extracted variant axes */
  variantAxes?: VariantAxisSchema[]
}

export function generatePreviewEntry(options: PreviewEntryOptions): string {
  const { filePath, exportName, width = 400, height = 300, propsControl = false } = options
  const isDefault = exportName === "default"
  const componentName = isDefault ? "TargetComponent" : exportName
  const title = options.title ?? `Preview: ${componentName}`

  const importStatement = isDefault
    ? `import ${componentName} from ${JSON.stringify(filePath)}`
    : `import { ${componentName} } from ${JSON.stringify(filePath)}`

  const shellImport = `import { PreviewShell } from "@qt-solid/solid/preview/shell"`

  if (!propsControl) {
    return `${importStatement}
${shellImport}
import { createApp, createWindow } from "@qt-solid/solid"

export default createApp(() => {
  const preview = createWindow(
    { title: ${JSON.stringify(title)}, width: ${width}, height: ${height} },
    () => (
      <PreviewShell componentName=${JSON.stringify(componentName)}>
        <${componentName} />
      </PreviewShell>
    ),
  )

  return {
    render: () => preview.render(),
    onWindowAllClosed: ({ quit }) => quit(),
  }
})
`
  }

  const propsJson = JSON.stringify(options.propsSchema ?? [])
  const axesJson = JSON.stringify(options.variantAxes ?? [])

  return `${importStatement}
${shellImport}
import { createApp, createWindow } from "@qt-solid/solid"
import { createPreviewWrapper } from "@qt-solid/solid/preview/runtime"

const PreviewWrapped = createPreviewWrapper(${componentName}, {
  componentName: ${JSON.stringify(componentName)},
  props: ${propsJson},
  variantAxes: ${axesJson},
})

export default createApp(() => {
  const preview = createWindow(
    { title: ${JSON.stringify(title)}, width: ${width}, height: ${height} },
    () => (
      <PreviewShell componentName=${JSON.stringify(componentName)}>
        <PreviewWrapped />
      </PreviewShell>
    ),
  )

  return {
    render: () => preview.render(),
    onWindowAllClosed: ({ quit }) => quit(),
  }
})
`
}
