# Layout API Redesign

Unify Figma intent props + Taffy props into a single layout API.

## Prop Groups

`CommonProps` is composed from semantic sub-interfaces:

```ts
interface CommonProps extends
  EventProps,
  MotionProps,
  TransformProps,
  VisualProps,
  FilterProps,
  LayoutProps {
  ref?: (node: FragmentRendererNode) => void
  pointerEvents?: boolean
  focusable?: boolean
  children?: unknown
}
```

### TransformProps

`transformX`, `transformY`, `scale`, `scaleX`, `scaleY`, `rotate`, `originX`, `originY`, `perspective`

These are paint-level transforms, not layout.

### VisualProps

`opacity`, `visible`, `blendMode`, `clip`, `clipPath`, `cursor`, `mask`

### FilterProps

`backdropBlur`, `filterGrayscale`, `filterSaturate`, `filterBrightness`, `filterContrast`, `filterHueRotate`, `filterInvert`, `filterSepia`, `vibrancyDesaturation`, `vibrancyBlendMode`

### LayoutProps

`row`, `column`, `w`, `h`, `align`, `spacing`, `gap`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `minWidth`, `maxWidth`, `minHeight`, `maxHeight`, `absolute`, `top`, `right`, `bottom`, `left`, `layoutVisible`, `overflow`, `zIndex`, `gridRow`, `gridColumn`, `rowSpan`, `colSpan`

## Direction

- `row` / `column` — boolean props on the element, default is column (no prop = column)
- `<rect row />` = horizontal, `<rect column />` = vertical (explicit), `<rect />` = vertical

## Sizing (`w` / `h`)

| Syntax | Meaning |
|--------|---------|
| omitted / `"hug"` | Wrap content (default, both axes) |
| `{100}` | Fixed 100px |
| `"50%"` | Percentage of parent |
| `"fill"` / `"1fr"` | Equal-weight fill |
| `"2fr"` | Weighted fill (main-axis: flex_grow=N; cross-axis: treated as fill) |

Cross-axis defaults to hug (no stretch).

## Alignment (`align`)

Screen-space directions, never flips with row/column:

- Single: `align="center"` (both axes centered)
- Dual: `align="top right"` (vertical horizontal)

Vertical values: `top` | `center` | `bottom` | `stretch`
Horizontal values: `left` | `center` | `right` | `stretch`

Parse rule: order is always `vertical horizontal`. Single value applies to both.

## Spacing distribution (`spacing`)

Main-axis distribution strategy (replaces justify-content):

`between` | `around` | `evenly`

Only meaningful on containers. Overrides the main-axis component of `align` when set.

## Gap & Padding & Margin

```tsx
<rect gap={8} padding={12} />
<rect paddingTop={4} paddingLeft={8} />
<rect margin={8} marginTop={4} />
```

`gap` applies uniformly. For grid, maps to both rowGap and columnGap.

## Absolute Positioning

```tsx
<rect absolute top={10} right={10} w={32} h={32} />
```

`absolute` is a boolean prop. `top`/`right`/`bottom`/`left` are inset values.
`transformX`/`transformY` are paint-level offset, not layout.

## Visibility

- `visible` — paint/hit-test visibility (does not affect layout)
- `layoutVisible` — layout participation (false = Display::None)

## Grid

```tsx
<grid columns={[100, "1fr", "2fr", "hug"]} rows={["hug", "1fr"]} gap={8}>
  <rect gridRow={0} gridColumn={1} />
  <rect gridRow={1} gridColumn={0} colSpan={2} />
</grid>
```

Grid child props: `gridRow`, `gridColumn`, `rowSpan`, `colSpan` (0-based indices).

## Constraints

```tsx
<rect minWidth={100} maxWidth={400} minHeight={50} maxHeight={300} />
```

## Overflow

Values: `"visible"` | `"clip"` | `"hidden"` | `"scroll"`

```tsx
<rect overflow="clip" />
<rect overflowX="scroll" overflowY="clip" />
```

`overflow` sets both axes. `overflowX`/`overflowY` set individual axes (override `overflow` when specified after it).

## zIndex

```tsx
<rect zIndex={10} />
```

## Props removed (from old API)

- `x`, `y` (paint offset) → `transformX`, `transformY`
- `primaryAlign`, `crossAlign`, `wrapDistribute` → `align` + `spacing`
- `direction` (string "horizontal"/"vertical") → `row`/`column` booleans
- `flexDirection`, `flexGrow`, `flexShrink`, `flexBasis`, `flexWrap` → `w`/`h` sizing + `row`/`column`
- `alignItems`, `alignSelf`, `justifyContent` → `align` + `spacing`
- `position` → `absolute` boolean
- `crossGap` → `gap`
- `width`/`height` (old taffy-style) → `w`/`h`

## Props renamed

- `row`/`column` (grid child number) → `gridRow`/`gridColumn`
- `x`/`y` → `transformX`/`transformY`

## Implementation

JS-side: rewrite `CommonProps` type (split into sub-interfaces) + `patchFragmentProp`/`writeFigmaLayoutProp` mapping.

Rust-side changes:
- Add `Sizing::Flex(f32)` variant for weighted fill
- `derive_taffy_style` stays, JS layer translates new props into existing Placement/Container/SizeIntent
- Add proper unset/reset handling for all layout intent props
- Rename `x`/`y` handling to `transformX`/`transformY` in fragment paint

The JS mapping layer translates:
- `row`/`column` booleans → `Container::Flex { direction }`
- `align` string → split into primary_align + cross_align based on direction
- `spacing` → overrides primary_align with distribution value
- `w`/`h` strings → `Sizing` enum variants
- `absolute` + insets → `Placement::Absolute`
- `gridRow`/`gridColumn`/`rowSpan`/`colSpan` → `Placement::GridCell`
