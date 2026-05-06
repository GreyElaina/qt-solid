import { createEffect, onCleanup, splitProps, type Component } from "solid-js"
import { readFileSync } from "node:fs"

import {
  canvasFragmentSetEncodedImage,
  canvasFragmentClearImage,
  canvasFragmentRequestRepaint,
} from "@qt-solid/core/native"

import type { ImageProps as ImageElementProps } from "../../intrinsics.ts"
import type { FragmentRendererNode } from "../../runtime/fragment.ts"

export interface ImageProps extends ImageElementProps {
  /** Path to an image file (PNG/JPEG/GIF/WebP). */
  src?: string
  /** Encoded image bytes (PNG/JPEG/GIF/WebP). Takes precedence over `src`. */
  data?: Buffer | Uint8Array
}

export const Image: Component<ImageProps> = (props) => {
  let nodeRef: FragmentRendererNode | undefined
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
      ref={(node: FragmentRendererNode) => { nodeRef = node }}
      {...intrinsic}
    />
  )
}
