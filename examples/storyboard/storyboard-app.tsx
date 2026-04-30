import {
  createApp,
  createWindow,
  ScrollView,
  motion,
  AnimatePresence,
  defineIntrinsicComponent,
  type AppHandle,
  type WindowHandle,
  type CanvasRectProps,
  type CanvasGroupProps,
} from "@qt-solid/solid"
import {
  createSignal,
  createMemo,
  For,
  Show,
  Index,
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
// Motion primitives for demos
// ---------------------------------------------------------------------------

const MotionRect = motion(defineIntrinsicComponent<CanvasRectProps>("rect"))
const MotionGroup = motion(defineIntrinsicComponent<CanvasGroupProps>("group"))

// ---------------------------------------------------------------------------
// Motion · Basics — spring/tween, scale/rotate/opacity
// ---------------------------------------------------------------------------

const MotionBasicsDemo: Component = () => {
  const [toggled, setToggled] = createSignal(false)

  return (
    <group flexDirection="column" gap={16}>
      <Button onClick={() => setToggled(v => !v)}>Toggle</Button>
      <group flexDirection="row" gap={16} alignItems="center">
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={60} height={60} cornerRadius={8}
            fill="#0078d4"
            initial={{ scale: 1, rotate: 0 }}
            animate={{ scale: toggled() ? 1.3 : 1, rotate: toggled() ? 45 : 0 }}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
          />
          <CaptionLabel text="Spring" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={60} height={60} cornerRadius={30}
            fill="#e74856"
            initial={{ opacity: 1, y: 0 }}
            animate={{ opacity: toggled() ? 0.3 : 1, y: toggled() ? -20 : 0 }}
            transition={{ type: "tween", duration: 0.4, ease: "ease-in-out" }}
          />
          <CaptionLabel text="Tween" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={60} height={60} cornerRadius={8}
            fill="#00cc6a"
            initial={{ scaleX: 1, scaleY: 1 }}
            animate={{ scaleX: toggled() ? 1.4 : 1, scaleY: toggled() ? 0.6 : 1 }}
            transition={{ type: "spring", stiffness: 400, damping: 15 }}
          />
          <CaptionLabel text="Squash" />
        </group>
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Gestures — whileHover / whileTap
// ---------------------------------------------------------------------------

const MotionGesturesDemo: Component = () => {
  return (
    <group flexDirection="row" gap={16} alignItems="center">
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={80} height={80} cornerRadius={12}
          fill="#744da9"
          animate={{ scale: 1 }}
          whileHover={{ scale: 1.1, rotate: 5 }}
          whileTap={{ scale: 0.9 }}
          transition={{ type: "spring", stiffness: 400, damping: 20 }}
          hitTest
        />
        <CaptionLabel text="Hover + Tap" />
      </group>
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={80} height={80} cornerRadius={40}
          fill="#f7630c"
          animate={{ scale: 1, opacity: 1 }}
          whileHover={{ scale: 1.15, opacity: 0.8 }}
          whileTap={{ scale: 0.85 }}
          transition={{ type: "spring", stiffness: 500, damping: 25 }}
          hitTest
        />
        <CaptionLabel text="Circle" />
      </group>
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={100} height={50} cornerRadius={25}
          fill="#0099bc"
          animate={{ scaleX: 1 }}
          whileHover={{ scaleX: 1.2 }}
          whileTap={{ scaleX: 0.8, scaleY: 1.2 }}
          transition={{ type: "spring", stiffness: 350, damping: 18 }}
          hitTest
        />
        <CaptionLabel text="Stretch" />
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Presence — enter/exit lifecycle
// ---------------------------------------------------------------------------

const MotionPresenceDemo: Component = () => {
  const [show, setShow] = createSignal(true)

  return (
    <group flexDirection="column" gap={12}>
      <Button onClick={() => setShow(v => !v)}>
        {show() ? "Remove" : "Add"}
      </Button>
      <AnimatePresence when={show()}>
        {() => (
          <MotionRect
            width={120} height={80} cornerRadius={12}
            fill="#0078d4"
            initial={{ opacity: 0, scale: 0.8, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.6, y: -20 }}
            transition={{ type: "spring", stiffness: 300, damping: 22 }}
          />
        )}
      </AnimatePresence>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Stagger — orchestrated children
// ---------------------------------------------------------------------------

const MotionStaggerDemo: Component = () => {
  const [visible, setVisible] = createSignal(true)
  const items = [0, 1, 2, 3, 4]

  const replay = () => {
    setVisible(false)
    setTimeout(() => setVisible(true), 50)
  }

  return (
    <group flexDirection="column" gap={12}>
      <Button onClick={replay}>Replay</Button>
      <AnimatePresence when={visible()}>
        {() => (
          <MotionGroup
            flexDirection="row" gap={8}
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            transition={{ staggerChildren: 0.08, delayChildren: 0.1 }}
          >
            <Index each={items}>
              {(_, i) => (
                <MotionRect
                  width={40} height={40} cornerRadius={6}
                  fill={["#0078d4", "#e74856", "#00cc6a", "#f7630c", "#744da9"][i % 5]!}
                  initial={{ opacity: 0, y: 30, scale: 0.5 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  transition={{ type: "spring", stiffness: 350, damping: 20 }}
                />
              )}
            </Index>
          </MotionGroup>
        )}
      </AnimatePresence>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Keyframes — multi-step animations
// ---------------------------------------------------------------------------

const MotionKeyframesDemo: Component = () => {
  const [playing, setPlaying] = createSignal(false)

  return (
    <group flexDirection="column" gap={12}>
      <Button onClick={() => setPlaying(v => !v)}>
        {playing() ? "Reset" : "Play"}
      </Button>
      <group flexDirection="row" gap={16} alignItems="center">
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={50} height={50} cornerRadius={8}
            fill="#0078d4"
            animate={{
              rotate: playing() ? [0, 90, 180, 270, 360] : 0,
              scale: playing() ? [1, 1.2, 1, 0.8, 1] : 1,
            }}
            transition={{
              type: "tween",
              duration: 2,
              ease: "ease-in-out",
              times: [0, 0.25, 0.5, 0.75, 1],
            }}
          />
          <CaptionLabel text="Spin + pulse" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={50} height={50} cornerRadius={25}
            fill="#e74856"
            animate={{
              x: playing() ? [0, 40, 0, -40, 0] : 0,
              y: playing() ? [0, -20, 0, -20, 0] : 0,
            }}
            transition={{
              type: "tween",
              duration: 1.5,
              ease: "ease-in-out",
              times: [0, 0.25, 0.5, 0.75, 1],
            }}
          />
          <CaptionLabel text="Figure-8" />
        </group>
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Drag — constrained draggable elements
// ---------------------------------------------------------------------------

const MotionDragDemo: Component = () => {
  return (
    <group flexDirection="column" gap={8}>
      <group flexDirection="row" gap={24} alignItems="center">
        <group flexDirection="column" gap={4} alignItems="center">
          <rect width={200} height={120} fill="#1a1a2e" cornerRadius={12} padding={8}>
            <MotionRect
              width={50} height={50} cornerRadius={8}
              fill="#0078d4"
              animate={{ x: 0, y: 0 }}
              drag
              dragConstraints={{ left: 0, right: 140, top: 0, bottom: 60 }}
              dragElastic={0.2}
              transition={{ type: "spring", stiffness: 300, damping: 20 }}
              hitTest
            />
          </rect>
          <CaptionLabel text="Drag (constrained)" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <rect width={200} height={120} fill="#1a1a2e" cornerRadius={12} padding={8}>
            <MotionRect
              width={50} height={50} cornerRadius={25}
              fill="#f7630c"
              animate={{ x: 0, y: 0 }}
              drag="x"
              dragConstraints={{ left: -60, right: 60 }}
              dragElastic={0.5}
              transition={{ type: "spring", stiffness: 500, damping: 25 }}
              hitTest
            />
          </rect>
          <CaptionLabel text="Drag X only (elastic)" />
        </group>
      </group>
      <CaptionLabel text="⚠ Drag API declared but not yet wired in runtime" />
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Loop — repeating animations
// ---------------------------------------------------------------------------

const MotionLoopDemo: Component = () => {
  return (
    <group flexDirection="row" gap={24} alignItems="center">
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={50} height={50} cornerRadius={25}
          fill="#0078d4"
          initial={{ scale: 1 }}
          animate={{ scale: [1, 1.3, 1] }}
          transition={{
            type: "tween",
            duration: 1.2,
            ease: "ease-in-out",
            times: [0, 0.5, 1],
            repeat: Infinity,
            repeatType: "loop",
          }}
        />
        <CaptionLabel text="Pulse (loop)" />
      </group>
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={50} height={50} cornerRadius={8}
          fill="#e74856"
          initial={{ rotate: 0 }}
          animate={{ rotate: 360 }}
          transition={{
            type: "tween",
            duration: 2,
            ease: "linear",
            repeat: Infinity,
            repeatType: "loop",
          }}
        />
        <CaptionLabel text="Spin (∞)" />
      </group>
      <group flexDirection="column" gap={4} alignItems="center">
        <MotionRect
          width={50} height={50} cornerRadius={8}
          fill="#00cc6a"
          initial={{ y: 0 }}
          animate={{ y: -20 }}
          transition={{
            type: "tween",
            duration: 0.6,
            ease: "ease-in-out",
            repeat: Infinity,
            repeatType: "reverse",
          }}
        />
        <CaptionLabel text="Bounce (reverse)" />
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Colors — background color & blur transitions
// ---------------------------------------------------------------------------

const MotionColorsDemo: Component = () => {
  const [index, setIndex] = createSignal(0)
  const colors = ["#0078d4", "#e74856", "#00cc6a", "#f7630c", "#744da9"]

  return (
    <group flexDirection="column" gap={12}>
      <Button onClick={() => setIndex(i => (i + 1) % colors.length)}>Next Color</Button>
      <group flexDirection="row" gap={16} alignItems="center">
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={80} height={80} cornerRadius={12}
            fill="#333"
            initial={{ opacity: 1 }}
            animate={{ backgroundColor: colors[index()]!, opacity: 1 }}
            transition={{ type: "tween", duration: 0.5, ease: "ease-in-out" }}
          />
          <CaptionLabel text="Color shift" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={80} height={80} cornerRadius={12}
            fill="#0078d4"
            initial={{ blur: 0, borderRadius: 12 }}
            animate={{
              blur: index() % 2 === 0 ? 0 : 8,
              borderRadius: index() % 2 === 0 ? 12 : 40,
            }}
            transition={{ type: "tween", duration: 0.6, ease: "ease-in-out" }}
          />
          <CaptionLabel text="Blur + radius" />
        </group>
        <group flexDirection="column" gap={4} alignItems="center">
          <MotionRect
            width={80} height={80} cornerRadius={12}
            fill="#744da9"
            initial={{ shadowBlur: 0, shadowOffsetY: 0 }}
            animate={{
              shadowBlur: index() % 2 === 0 ? 0 : 16,
              shadowOffsetY: index() % 2 === 0 ? 0 : 6,
              shadowColor: "#00000066",
            }}
            transition={{ type: "tween", duration: 0.5, ease: "ease-out" }}
          />
          <CaptionLabel text="Shadow" />
        </group>
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Layout — layoutId shared element transition
// ---------------------------------------------------------------------------

const MotionLayoutDemo: Component = () => {
  const [selected, setSelected] = createSignal(0)
  const tabs = ["Home", "Search", "Profile"]

  return (
    <group flexDirection="column" gap={12}>
      <group flexDirection="row" gap={0}>
        <Index each={tabs}>
          {(tab, i) => (
            <rect
              width={80} height={36}
              flexDirection="column"
              alignItems="center"
              justifyContent="center"
              onPointerUp={() => setSelected(i)}
              onClick={() => {}}
            >
              <text text={tab()} fontSize={13} color={selected() === i ? "#0078d4" : "#888"} />
              <Show when={selected() === i}>
                <MotionRect
                  width={40} height={3} cornerRadius={2}
                  fill="#0078d4"
                  layoutId="tab-indicator"
                  layout="position"
                  layoutTransition={{ type: "spring", stiffness: 500, damping: 30 }}
                />
              </Show>
            </rect>
          )}
        </Index>
      </group>
      <CaptionLabel text="Click tabs — indicator animates between positions" />
    </group>
  )
}

// ---------------------------------------------------------------------------
// Motion · Compound — combining multiple techniques
// ---------------------------------------------------------------------------

const MotionCompoundDemo: Component = () => {
  const [expanded, setExpanded] = createSignal(false)

  return (
    <group flexDirection="column" gap={12}>
      <Button onClick={() => setExpanded(v => !v)}>
        {expanded() ? "Collapse" : "Expand"}
      </Button>
      <group flexDirection="row" gap={12} alignItems="flex-start">
        <MotionRect
          width={expanded() ? 200 : 80}
          height={expanded() ? 120 : 80}
          cornerRadius={expanded() ? 16 : 40}
          fill="#0078d4"
          layout
          animate={{
            scale: 1,
            rotate: expanded() ? 0 : 0,
          }}
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          transition={{ type: "spring", stiffness: 300, damping: 25 }}
          layoutTransition={{ type: "spring", stiffness: 400, damping: 28 }}
          hitTest
        >
          <text
            text={expanded() ? "I'm expanded!" : "Hi"}
            fontSize={expanded() ? 16 : 12}
            color="#ffffff"
          />
        </MotionRect>
        <MotionRect
          width={80} height={80} cornerRadius={8}
          fill="#e74856"
          animate={{
            x: expanded() ? 20 : 0,
            opacity: expanded() ? 0.5 : 1,
          }}
          whileHover={{ rotate: 10 }}
          transition={{ type: "spring", stiffness: 250, damping: 20 }}
          hitTest
        />
      </group>
    </group>
  )
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
  {
    name: "Motion · Basics",
    render: () => <MotionBasicsDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Gestures",
    render: () => <MotionGesturesDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Presence",
    render: () => <MotionPresenceDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Stagger",
    render: () => <MotionStaggerDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Keyframes",
    render: () => <MotionKeyframesDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Drag",
    render: () => <MotionDragDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Loop",
    render: () => <MotionLoopDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Colors",
    render: () => <MotionColorsDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Layout",
    render: () => <MotionLayoutDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Motion · Compound",
    render: () => <MotionCompoundDemo />,
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
        <ScrollView flexGrow={1} flexShrink={1}>
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
