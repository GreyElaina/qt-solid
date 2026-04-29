import {
  createApp,
  createWindow,
  ScrollView,
  type AppHandle,
  type WindowHandle,
} from "@qt-solid/solid"
import {
  createSignal,
  createMemo,
  For,
  Show,
  type Component,
  type JSX,
  type Accessor,
} from "solid-js"

import {
  ThemeProvider,
  useTheme,
  fluentDark,
  fluentLight,
  type FluentTokens,

  Button,
  Toggle,
  CheckBox,
  Card,
  InfoBar,
  Slider,
  LineEdit,
  RadioButton,
  ProgressBar,
  ProgressRing,
  InfoBadge,
  HorizontalSeparator,
  VerticalSeparator,
  TransparentButton,
  HyperlinkButton,
  ToggleButton,
  PillButton,
  CaptionLabel,
  BodyLabel,
  SubtitleLabel,
  TitleLabel,
  DisplayLabel,
} from "@qt-solid/fluent"

// ---------------------------------------------------------------------------
// Chrome theming helper — maps FluentTokens to storyboard chrome roles
// ---------------------------------------------------------------------------

function useChromeColors() {
  const theme = useTheme()
  return {
    bg:      () => theme().backgroundDefault,
    sidebar: () => theme().backgroundSecondary,
    border:  () => theme().strokeDefault,
    text:    () => theme().foregroundPrimary,
    dim:     () => theme().foregroundSecondary,
    accent:  () => theme().accentDefault,
    hover:   () => theme().controlHover,
  }
}

// ---------------------------------------------------------------------------
// Story definition
// ---------------------------------------------------------------------------

interface StoryVariant {
  label: string
  props: Record<string, unknown>
}

interface StoryDef {
  name: string
  render: (props: Record<string, unknown>) => JSX.Element
  axes: Record<string, unknown[]>
  defaults: Record<string, unknown>
  scenarios?: Record<string, Record<string, unknown>>
}

function cartesian(axes: Record<string, unknown[]>): Record<string, unknown>[] {
  const keys = Object.keys(axes)
  if (keys.length === 0) return [{}]
  const results: Record<string, unknown>[] = []

  function recurse(index: number, current: Record<string, unknown>) {
    if (index >= keys.length) {
      results.push({ ...current })
      return
    }
    const key = keys[index]!
    for (const value of axes[key]!) {
      current[key] = value
      recurse(index + 1, current)
    }
  }

  recurse(0, {})
  return results
}

function axisLabel(combo: Record<string, unknown>, axes: Record<string, unknown[]>): string {
  return Object.keys(axes)
    .map((k) => `${k}=${String(combo[k])}`)
    .join("  ")
}

// ---------------------------------------------------------------------------
// Stories registry
// ---------------------------------------------------------------------------

const STORIES: StoryDef[] = [
  {
    name: "Button",
    render: (p) => <Button {...p as any}>{(p.children as string) ?? "Button"}</Button>,
    axes: { accent: [false, true], disabled: [false, true] },
    defaults: { children: "Click me" },
    scenarios: {
      "wide": { children: "Wide Button", width: 200 },
    },
  },
  {
    name: "Toggle",
    render: (p) => <Toggle {...p as any} />,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: {},
  },
  {
    name: "CheckBox",
    render: (p) => <CheckBox {...p as any} />,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: { label: "Option" },
  },
  {
    name: "RadioButton",
    render: (p) => <RadioButton {...p as any} />,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: { label: "Choice" },
  },
  {
    name: "Slider",
    render: (p) => <Slider {...p as any} />,
    axes: { disabled: [false, true] },
    defaults: { value: 40, width: 160 },
    scenarios: {
      "empty": { value: 0, width: 160 },
      "full": { value: 100, width: 160 },
    },
  },
  {
    name: "LineEdit",
    render: (p) => <LineEdit {...p as any} />,
    axes: { disabled: [false, true], error: [false, true] },
    defaults: { placeholder: "Type here...", width: 180 },
  },
  {
    name: "ProgressBar",
    render: (p) => <ProgressBar {...p as any} />,
    axes: { paused: [false, true], error: [false, true] },
    defaults: { value: 60, width: 160 },
  },
  {
    name: "ProgressRing",
    render: (p) => <ProgressRing {...p as any} />,
    axes: {},
    defaults: { value: 65 },
  },
  {
    name: "Card",
    render: (p) => (
      <Card {...p as any}>
        <BodyLabel text="Card content" />
      </Card>
    ),
    axes: { clickable: [false, true], disabled: [false, true] },
    defaults: { width: 160, padding: 12 },
  },
  {
    name: "InfoBar",
    render: (p) => <InfoBar {...p as any} />,
    axes: { severity: ["info", "success", "warning", "error"] },
    defaults: { title: "Title", message: "Description text", closable: true, width: 280 },
  },
  {
    name: "InfoBadge",
    render: (p) => <InfoBadge {...p as any} />,
    axes: { level: ["info", "success", "caution", "critical", "attention"] },
    defaults: {},
  },
  {
    name: "TransparentButton",
    render: (p) => <TransparentButton {...p as any}>{(p.children as string) ?? "Transparent"}</TransparentButton>,
    axes: { disabled: [false, true] },
    defaults: { children: "Transparent" },
  },
  {
    name: "HyperlinkButton",
    render: (p) => <HyperlinkButton {...p as any}>{(p.children as string) ?? "Link"}</HyperlinkButton>,
    axes: { disabled: [false, true] },
    defaults: { children: "Link text" },
  },
  {
    name: "ToggleButton",
    render: (p) => <ToggleButton {...p as any}>{(p.children as string) ?? "Toggle"}</ToggleButton>,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: { children: "Toggle" },
  },
  {
    name: "PillButton",
    render: (p) => <PillButton {...p as any}>{(p.children as string) ?? "Pill"}</PillButton>,
    axes: { disabled: [false, true] },
    defaults: { children: "Pill" },
  },
  {
    name: "Labels",
    render: (p) => (
      <group flexDirection="column" gap={4}>
        <CaptionLabel text="Caption (12px)" />
        <BodyLabel text="Body (14px)" />
        <SubtitleLabel text="Subtitle (20px)" />
        <TitleLabel text="Title (28px)" />
      </group>
    ),
    axes: {},
    defaults: {},
  },
  {
    name: "Separators",
    render: () => (
      <group flexDirection="row" gap={16} alignItems="center" height={40}>
        <BodyLabel text="Left" />
        <VerticalSeparator height={30} />
        <BodyLabel text="Right" />
      </group>
    ),
    axes: {},
    defaults: {},
  },
]

// ---------------------------------------------------------------------------
// Sidebar item
// ---------------------------------------------------------------------------

const SidebarItem: Component<{
  name: string
  selected: boolean
  onSelect: () => void
}> = (props) => {
  const chrome = useChromeColors()
  const [hovered, setHovered] = createSignal(false)

  const bg = () => {
    if (props.selected) return chrome.accent()
    if (hovered()) return chrome.hover()
    return "transparent"
  }

  const fg = () => props.selected ? "#000000" : chrome.text()

  return (
    <rect
      height={32}
      flexDirection="row"
      alignItems="center"
      paddingLeft={12}
      paddingRight={12}
      fill={bg()}
      cornerRadius={6}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onPointerUp={props.onSelect}
      onClick={() => {}}
    >
      <text text={props.name} fontSize={13} color={fg()} />
    </rect>
  )
}

// ---------------------------------------------------------------------------
// Variant cell — renders one combination in the matrix
// ---------------------------------------------------------------------------

const VariantCell: Component<{
  story: StoryDef
  combo: Record<string, unknown>
  label: string
}> = (props) => {
  const chrome = useChromeColors()
  const merged = createMemo(() => ({ ...props.story.defaults, ...props.combo }))

  return (
    <group flexDirection="column" gap={6} padding={8} minWidth={CELL_MIN_WIDTH} flexBasis={CELL_MIN_WIDTH} flexGrow={1}>
      <text text={props.label} fontSize={10} color={chrome.dim()} />
      <rect
        fill="transparent"
        stroke={chrome.border()}
        strokeWidth={1}
        cornerRadius={6}
        padding={12}
        flexDirection="column"
        alignItems="flex-start"
      >
        {props.story.render(merged())}
      </rect>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Responsive grid — row wrap when fits, column when not
// ---------------------------------------------------------------------------

const CELL_MIN_WIDTH = 220

const ResponsiveGrid: Component<{
  count: number
  gap?: number
  children: JSX.Element
}> = (props) => {
  const [containerWidth, setContainerWidth] = createSignal(0)
  const gap = () => props.gap ?? 4

  const useColumn = () => {
    const w = containerWidth()
    if (w <= 0 || props.count <= 1) return false
    // If 2 cells + gap don't fit side by side, switch to column
    return w < CELL_MIN_WIDTH * 2 + gap()
  }

  return (
    <group
      flexDirection={useColumn() ? "column" : "row"}
      flexWrap={useColumn() ? "nowrap" : "wrap"}
      gap={gap()}
      width="100%"
      onLayout={(e: { width: number; height: number }) => setContainerWidth(e.width)}
    >
      {props.children}
    </group>
  )
}

// ---------------------------------------------------------------------------
// Story detail view
// ---------------------------------------------------------------------------

const StoryDetail: Component<{ story: StoryDef }> = (props) => {
  const chrome = useChromeColors()
  const combos = createMemo(() => cartesian(props.story.axes))
  const axisKeys = createMemo(() => Object.keys(props.story.axes))
  const scenarioEntries = createMemo(() =>
    props.story.scenarios ? Object.entries(props.story.scenarios) : [],
  )

  return (
    <group flexDirection="column" gap={16} padding={20} width="100%">
      {/* Header */}
      <text text={props.story.name} fontSize={24} fontWeight={600} color={chrome.text()} />
      <Show when={axisKeys().length > 0}>
        <text
          text={`Axes: ${axisKeys().join(", ")} · ${combos().length} combinations`}
          fontSize={12}
          color={chrome.dim()}
        />
      </Show>

      {/* Variant matrix */}
      <Show when={combos().length > 0}>
        <text text="Variant Matrix" fontSize={14} fontWeight={600} color={chrome.text()} />
        <ResponsiveGrid count={combos().length}>
          <For each={combos()}>
            {(combo) => (
              <VariantCell
                story={props.story}
                combo={combo}
                label={axisLabel(combo, props.story.axes)}
              />
            )}
          </For>
        </ResponsiveGrid>
      </Show>

      {/* Default (for stories with no axes) */}
      <Show when={combos().length === 0}>
        <VariantCell
          story={props.story}
          combo={{}}
          label="default"
        />
      </Show>

      {/* Scenarios */}
      <Show when={scenarioEntries().length > 0}>
        <rect height={1} fill={chrome.border()} />
        <text text="Scenarios" fontSize={14} fontWeight={600} color={chrome.text()} />
        <ResponsiveGrid count={scenarioEntries().length}>
          <For each={scenarioEntries()}>
            {([name, overrides]) => (
              <VariantCell
                story={props.story}
                combo={overrides}
                label={name}
              />
            )}
          </For>
        </ResponsiveGrid>
      </Show>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Theme toggle
// ---------------------------------------------------------------------------

const ThemeToggle: Component<{
  isDark: Accessor<boolean>
  onToggle: () => void
}> = (props) => {
  const chrome = useChromeColors()
  const [hovered, setHovered] = createSignal(false)

  return (
    <rect
      height={28}
      flexDirection="row"
      alignItems="center"
      justifyContent="center"
      paddingLeft={10}
      paddingRight={10}
      fill={hovered() ? chrome.hover() : "transparent"}
      cornerRadius={6}
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onPointerUp={props.onToggle}
      onClick={() => {}}
    >
      <text
        text={props.isDark() ? "Dark" : "Light"}
        fontSize={11}
        color={chrome.dim()}
      />
    </rect>
  )
}

// ---------------------------------------------------------------------------
// Main storyboard chrome — must be inside ThemeProvider
// ---------------------------------------------------------------------------

const StoryboardChrome: Component<{
  selectedIndex: Accessor<number>
  setSelectedIndex: (i: number) => void
  isDark: Accessor<boolean>
  setIsDark: (fn: (v: boolean) => boolean) => void
}> = (props) => {
  const chrome = useChromeColors()
  const activeStory = createMemo(() => STORIES[props.selectedIndex()]!)

  return (
    <rect fill={chrome.bg()} flexGrow={1} flexDirection="row">
      {/* Sidebar */}
      <rect
        width={200}
        fill={chrome.sidebar()}
        flexDirection="column"
      >
        {/* Sidebar header */}
        <rect
          height={48}
          flexDirection="row"
          alignItems="center"
          paddingLeft={12}
          paddingRight={12}
          gap={8}
        >
          <rect fill={chrome.accent()} width={8} height={8} cornerRadius={4} />
          <text text="Storyboard" fontSize={13} fontWeight={600} color={chrome.text()} />
        </rect>
        <rect height={1} fill={chrome.border()} />

        {/* Story list */}
        <ScrollView flexGrow={1}>
          <group flexDirection="column" gap={2} padding={6}>
            <For each={STORIES}>
              {(story, index) => (
                <SidebarItem
                  name={story.name}
                  selected={index() === props.selectedIndex()}
                  onSelect={() => props.setSelectedIndex(index())}
                />
              )}
            </For>
          </group>
        </ScrollView>

        {/* Theme toggle at bottom */}
        <rect height={1} fill={chrome.border()} />
        <rect height={44} flexDirection="row" alignItems="center" justifyContent="center">
          <ThemeToggle isDark={props.isDark} onToggle={() => props.setIsDark((v) => !v)} />
        </rect>
      </rect>

      {/* Divider */}
      <rect width={1} fill={chrome.border()} />

      {/* Content area */}
      <rect fill={chrome.bg()} flexGrow={1} flexDirection="column">
        <ScrollView flexGrow={1}>
          <StoryDetail story={activeStory()} />
        </ScrollView>
      </rect>
    </rect>
  )
}

// ---------------------------------------------------------------------------
// Main storyboard window
// ---------------------------------------------------------------------------

function createStoryboardWindow(): WindowHandle {
  const [selectedIndex, setSelectedIndex] = createSignal(0)
  const [isDark, setIsDark] = createSignal(true)

  const theme = createMemo(() => isDark() ? fluentDark : fluentLight)

  return createWindow(
    {
      title: "qt-solid Storyboard",
      width: 900,
      height: 700,
    },
    () => (
      <ThemeProvider value={theme}>
        <StoryboardChrome
          selectedIndex={selectedIndex}
          setSelectedIndex={setSelectedIndex}
          isDark={isDark}
          setIsDark={setIsDark}
        />
      </ThemeProvider>
    ),
  )
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export function createStoryboardApp(): AppHandle {
  return createApp(() => {
    const mainWindow = createStoryboardWindow()

    return {
      render: () => mainWindow.render(),
      onActivate() {
        mainWindow.open()
      },
    }
  })
}
