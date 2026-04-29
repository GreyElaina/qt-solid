// ---------------------------------------------------------------------------
// Preview runtime — injected into the virtual entry to enable props control.
//
// Wraps a target component with reactive signal-backed props, listens for
// IPC messages from the host to mutate those signals at runtime.
// ---------------------------------------------------------------------------

import { createSignal, type Component, type JSX } from "solid-js"
import { createComponent } from "../runtime/renderer.ts"
import type {
  HostToPreviewMessage,
  PreviewToHostMessage,
  PreviewPropsSchema,
  PropFieldSchema,
  VariantAxisSchema,
} from "./protocol.ts"

interface PropSignal {
  get: () => unknown
  set: (v: unknown) => void
  field: PropFieldSchema
}

interface PreviewRuntimeState {
  props: Map<string, PropSignal>
  variantAxes: VariantAxisSchema[]
  componentName: string
}

let state: PreviewRuntimeState | null = null

function send(msg: PreviewToHostMessage): void {
  process.send?.(msg)
}

function buildSchema(): PreviewPropsSchema {
  if (!state) return { componentName: "Unknown", props: [], variantAxes: [] }
  return {
    componentName: state.componentName,
    props: [...state.props.values()].map(s => ({
      ...s.field,
      defaultValue: s.get(),
    })),
    variantAxes: state.variantAxes,
  }
}

function handleMessage(msg: HostToPreviewMessage): void {
  if (!state) return

  switch (msg.type) {
    case "set-prop": {
      const signal = state.props.get(msg.name)
      if (signal) signal.set(msg.value)
      break
    }
    case "set-variant": {
      // Variant axes are just props with enum type
      const signal = state.props.get(msg.axis)
      if (signal) signal.set(msg.value)
      break
    }
    case "get-schema": {
      send({ type: "schema", schema: buildSchema() })
      break
    }
    case "reset-props": {
      for (const signal of state.props.values()) {
        signal.set(signal.field.defaultValue)
      }
      break
    }
  }
}

export interface PreviewWrapOptions {
  componentName: string
  props: PropFieldSchema[]
  variantAxes?: VariantAxisSchema[]
}

/**
 * Wrap a component for preview: each declared prop becomes a signal that
 * can be mutated from the host via IPC.
 */
export function createPreviewWrapper<P extends Record<string, unknown>>(
  component: Component<P>,
  options: PreviewWrapOptions,
): Component<Record<string, never>> {
  const propSignals = new Map<string, PropSignal>()

  for (const field of options.props) {
    const [get, set] = createSignal(field.defaultValue)
    propSignals.set(field.name, { get, set, field })
  }

  if (options.variantAxes) {
    for (const axis of options.variantAxes) {
      const field: PropFieldSchema = {
        name: axis.name,
        type: "enum",
        values: axis.values,
        defaultValue: axis.defaultValue,
      }
      const [get, set] = createSignal(axis.defaultValue)
      propSignals.set(axis.name, { get, set, field })
    }
  }

  state = {
    props: propSignals,
    variantAxes: options.variantAxes ?? [],
    componentName: options.componentName,
  }

  // Listen for IPC messages from host
  process.on("message", (msg: HostToPreviewMessage) => {
    handleMessage(msg)
  })

  // Notify host we're ready
  send({ type: "ready" })
  send({ type: "schema", schema: buildSchema() })

  return () => {
    const reactiveProps = {} as Record<string, unknown>
    for (const [name, signal] of propSignals) {
      Object.defineProperty(reactiveProps, name, {
        get: signal.get,
        enumerable: true,
      })
    }
    return createComponent(component, reactiveProps as P)
  }
}
