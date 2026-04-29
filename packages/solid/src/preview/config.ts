// ---------------------------------------------------------------------------
// qt-solid project configuration
// ---------------------------------------------------------------------------

export interface QtSolidConfig {
  /** Path to the design token / theme source file */
  theme?: string
  /** Component library entry point(s) — used for export discovery */
  components?: string | string[]
  /** Preview defaults */
  preview?: {
    width?: number
    height?: number
    /** WebSocket port for Figma plugin bridge. Default: 9230 */
    wsPort?: number
  }
  /** Figma integration */
  figma?: {
    /** Figma file key (for REST API, optional) */
    fileKey?: string
    /** Variable collection name mapping */
    collectionName?: string
  }
}

export function defineConfig(config: QtSolidConfig): QtSolidConfig {
  return config
}
