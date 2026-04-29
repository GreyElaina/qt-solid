// ---------------------------------------------------------------------------
// Preview Protocol — IPC messages between host and preview child
// ---------------------------------------------------------------------------

// --- Props schema ---

export interface PropFieldSchema {
  name: string
  type: "string" | "number" | "boolean" | "enum" | "color"
  /** For enum type: possible values */
  values?: string[]
  /** Current default / initial value */
  defaultValue?: unknown
}

export interface VariantAxisSchema {
  name: string
  values: string[]
  defaultValue?: string
}

export interface PreviewPropsSchema {
  componentName: string
  props: PropFieldSchema[]
  variantAxes: VariantAxisSchema[]
}

// --- Host → Child messages ---

export type HostToPreviewMessage =
  | { type: "set-prop"; name: string; value: unknown }
  | { type: "set-variant"; axis: string; value: string }
  | { type: "get-schema" }
  | { type: "reset-props" }

// --- Child → Host messages ---

export type PreviewToHostMessage =
  | { type: "schema"; schema: PreviewPropsSchema }
  | { type: "ready" }
  | { type: "error"; message: string }
