import type { Component } from "solid-js"

import { defineIntrinsicComponent } from "@qt-solid/solid"
import { registerExampleWidgetsLibrary } from "@qt-solid/example-widgets/widget-library"

import type { BannerIntrinsicProps, SpinTriangleIntrinsicProps } from "./qt-intrinsics.ts"

registerExampleWidgetsLibrary()

export type BannerProps = BannerIntrinsicProps
export type SpinTriangleProps = SpinTriangleIntrinsicProps

export const Banner: Component<BannerProps> = defineIntrinsicComponent<BannerProps>("banner")
export const SpinTriangle: Component<SpinTriangleProps> =
  defineIntrinsicComponent<SpinTriangleProps>("spinTriangle")
