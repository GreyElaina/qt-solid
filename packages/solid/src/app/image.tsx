import { createEffect, onCleanup, splitProps, type Component } from "solid-js"
import { readFileSync } from "node:fs"

import {
  canvasFragmentSetEncodedImage,
  canvasFragmentClearImage,
  canvasFragmentRequestRepaint,
} from "@qt-solid/core/native"

import type { CanvasNodeHandle, CanvasImageProps } from "../qt-intrinsics.ts"

export interface ImageProps extends CanvasImageProps {
  /** Path to an image file (PNG/JPEG/GIF/WebP). */
  src?: string
  /** Encoded image bytes (PNG/JPEG/GIF/WebP). Takes precedence over `src`. */
  data?: Buffer | Uint8Array
}

export const Image: Component<ImageProps> = (props) => {
  let nodeRef: CanvasNodeHandle | undefined
  const [local, intrinsic] = splitProps(props, ["src", "data"])

  createEffect(() => {
    if (!nodeRef) return

    const bytes = local.data ?? (local.src ? readFileSync(local.src) : null)
    if (bytes) {
      const buf = bytes instanceof Buffer ? bytes : Buffer.from(bytes)
      canvasFragmentSetEncodedImage(nodeRef.canvasNodeId, nodeRef.fragmentId, buf)
    } else {
      canvasFragmentClearImage(nodeRef.canvasNodeId, nodeRef.fragmentId)
    }
    canvasFragmentRequestRepaint(nodeRef.canvasNodeId)
  })

  onCleanup(() => {
    if (nodeRef) {
      canvasFragmentClearImage(nodeRef.canvasNodeId, nodeRef.fragmentId)
    }
  })

  return (
    <image
      ref={(node: CanvasNodeHandle) => { nodeRef = node }}
      {...intrinsic}
    />
  )
}
