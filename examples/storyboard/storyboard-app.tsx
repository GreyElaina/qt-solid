import {
  createApp,
  createWindow,
  ScrollView,
  VirtualList,
  AnimatePresence,
  Router,
  Outlet,
  useNavigate,
  useLocation,
  useParams,
  useBreadcrumbs,
  useCanGoBack,
  type AppHandle,
  type WindowHandle,
  type RouteDefinition,
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
  useContextMenu,
  Breadcrumb,
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
  interactive?: () => JSX.Element
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
// Motion · Basics — spring/tween, scale/rotate/opacity
// ---------------------------------------------------------------------------

const MotionBasicsDemo: Component = () => {
  const [toggled, setToggled] = createSignal(false)

  return (
    <group gap={16}>
      <Button onClick={() => setToggled(v => !v)}>Toggle</Button>
      <group row gap={16} align="center left">
        <group gap={4} align="top center">
          <rect
            w={60} h={60} cornerRadius={8}
            fill="#0078d4"
            initial={{ scale: 1, rotate: 0 }}
            animate={{ scale: toggled() ? 1.3 : 1, rotate: toggled() ? 45 : 0 }}
            transition={{ type: "spring", stiffness: 300, damping: 20 }}
          />
          <CaptionLabel text="Spring" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={60} h={60} cornerRadius={30}
            fill="#e74856"
            initial={{ opacity: 1, y: 0 }}
            animate={{ opacity: toggled() ? 0.3 : 1, y: toggled() ? -20 : 0 }}
            transition={{ type: "tween", duration: 0.4, ease: "ease-in-out" }}
          />
          <CaptionLabel text="Tween" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={60} h={60} cornerRadius={8}
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
    <group row gap={16} align="center left">
      <group gap={4} align="top center">
        <rect
          w={80} h={80} cornerRadius={12}
          fill="#744da9"
          animate={{ scale: 1 }}
          whileHover={{ scale: 1.1, rotate: 5 }}
          whileTap={{ scale: 0.9 }}
          transition={{ type: "spring", stiffness: 400, damping: 20 }}
          hitTest
        />
        <CaptionLabel text="Hover + Tap" />
      </group>
      <group gap={4} align="top center">
        <rect
          w={80} h={80} cornerRadius={40}
          fill="#f7630c"
          animate={{ scale: 1, opacity: 1 }}
          whileHover={{ scale: 1.15, opacity: 0.8 }}
          whileTap={{ scale: 0.85 }}
          transition={{ type: "spring", stiffness: 500, damping: 25 }}
          hitTest
        />
        <CaptionLabel text="Circle" />
      </group>
      <group gap={4} align="top center">
        <rect
          w={100} h={50} cornerRadius={25}
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
    <group gap={12}>
      <Button onClick={() => setShow(v => !v)}>
        {show() ? "Remove" : "Add"}
      </Button>
      <AnimatePresence when={show()}>
        {() => (
          <rect
            w={120} h={80} cornerRadius={12}
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
    <group gap={12}>
      <Button onClick={replay}>Replay</Button>
      <AnimatePresence when={visible()}>
        {() => (
          <group
            row gap={8}
            initial={{ opacity: 1 }}
            animate={{ opacity: 1 }}
            transition={{ staggerChildren: 0.08, delayChildren: 0.1 }}
          >
            <Index each={items}>
              {(_, i) => (
                <rect
                  w={40} h={40} cornerRadius={6}
                  fill={["#0078d4", "#e74856", "#00cc6a", "#f7630c", "#744da9"][i % 5]!}
                  initial={{ opacity: 0, y: 30, scale: 0.5 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  transition={{ type: "spring", stiffness: 350, damping: 20 }}
                />
              )}
            </Index>
          </group>
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
    <group gap={12}>
      <Button onClick={() => setPlaying(v => !v)}>
        {playing() ? "Reset" : "Play"}
      </Button>
      <group row gap={16} align="center left">
        <group gap={4} align="top center">
          <rect
            w={50} h={50} cornerRadius={8}
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
        <group gap={4} align="top center">
          <rect
            w={50} h={50} cornerRadius={25}
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
    <group gap={8}>
      <group row gap={24} align="center left">
        <group gap={4} align="top center">
          <rect w={200} h={120} fill="#1a1a2e" cornerRadius={12} padding={8}>
            <rect
              w={50} h={50} cornerRadius={8}
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
        <group gap={4} align="top center">
          <rect w={200} h={120} fill="#1a1a2e" cornerRadius={12} padding={8}>
            <rect
              w={50} h={50} cornerRadius={25}
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
    <group row gap={24} align="center left">
      <group gap={4} align="top center">
        <rect
          w={50} h={50} cornerRadius={25}
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
      <group gap={4} align="top center">
        <rect
          w={50} h={50} cornerRadius={8}
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
      <group gap={4} align="top center">
        <rect
          w={50} h={50} cornerRadius={8}
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
    <group gap={12}>
      <Button onClick={() => setIndex(i => (i + 1) % colors.length)}>Next Color</Button>
      <group row gap={16} align="center left">
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={12}
            fill="#333"
            initial={{ opacity: 1 }}
            animate={{ backgroundColor: colors[index()]!, opacity: 1 }}
            transition={{ type: "tween", duration: 0.5, ease: "ease-in-out" }}
          />
          <CaptionLabel text="Color shift" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={12}
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
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={12}
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
    <group gap={12}>
      <group row gap={0}>
        <Index each={tabs}>
          {(tab, i) => (
            <rect
              w={80} h={36}
             
              align="center"
             
              onPointerUp={() => setSelected(i)}
              onClick={() => {}}
            >
              <text text={tab()} fontSize={13} color={selected() === i ? "#0078d4" : "#888"} />
              <Show when={selected() === i}>
                <rect
                  w={40} h={3} cornerRadius={2}
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
    <group gap={12}>
      <Button onClick={() => setExpanded(v => !v)}>
        {expanded() ? "Collapse" : "Expand"}
      </Button>
      <group row gap={12}>
        <rect
          w={expanded() ? 200 : 80}
          h={expanded() ? 120 : 80}
          cornerRadius={40}
          fill="#0078d4"
          clip
          layout
          animate={{
            scale: 1,
            borderRadius: expanded() ? 16 : 40,
          }}
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          transition={{ type: "spring", stiffness: 300, damping: 25 }}
          layoutTransition={{ type: "spring", stiffness: 400, damping: 28 }}
          hitTest
        >
          <AnimatePresence when={!expanded()}>
            {() => (
              <text
                text="Hi"
                fontSize={12}
                color="#ffffff"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ type: "tween", duration: 0.18, ease: "ease-in-out" }}
              />
            )}
          </AnimatePresence>
          <AnimatePresence when={expanded()}>
            {() => (
              <text
                text="I'm expanded!"
                fontSize={16}
                color="#ffffff"
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -8 }}
                transition={{ type: "tween", duration: 0.22, ease: "ease-in-out" }}
              />
            )}
          </AnimatePresence>
        </rect>
        <rect
          w={80} h={80} cornerRadius={8}
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
// Effect · 3D Transform — perspective rotateX/rotateY
// ---------------------------------------------------------------------------

const Effect3DTransformDemo: Component = () => {
  const [flipped, setFlipped] = createSignal(false)

  return (
    <group gap={12}>
      <Button onClick={() => setFlipped(v => !v)}>Flip</Button>
      <group row gap={24} align="center left">
        <group gap={4} align="top center">
          <rect
            w={100} h={100} cornerRadius={12}
            fill="#0078d4"
            perspective={800}
            layer
            initial={{ rotateY: 0 }}
            animate={{ rotateY: flipped() ? 180 : 0 }}
            transition={{ type: "spring", stiffness: 200, damping: 20 }}
          >
            <text text="Y-axis" fontSize={14} color="#ffffff" />
          </rect>
          <CaptionLabel text="rotateY" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={100} h={100} cornerRadius={12}
            fill="#e74856"
            perspective={800}
            layer
            initial={{ rotateX: 0 }}
            animate={{ rotateX: flipped() ? 180 : 0 }}
            transition={{ type: "spring", stiffness: 200, damping: 20 }}
          >
            <text text="X-axis" fontSize={14} color="#ffffff" />
          </rect>
          <CaptionLabel text="rotateX" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={100} h={100} cornerRadius={12}
            fill="#00cc6a"
            perspective={600}
            layer
            initial={{ rotateX: 0, rotateY: 0 }}
            animate={{
              rotateX: flipped() ? 25 : 0,
              rotateY: flipped() ? -35 : 0,
            }}
            transition={{ type: "spring", stiffness: 180, damping: 18 }}
          >
            <text text="Both" fontSize={14} color="#ffffff" />
          </rect>
          <CaptionLabel text="rotateX + Y" />
        </group>
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Effect · Content Filters — CSS-style filter props
// ---------------------------------------------------------------------------

const EffectContentFiltersDemo: Component = () => {
  const [active, setActive] = createSignal(false)

  return (
    <group gap={12}>
      <Button onClick={() => setActive(v => !v)}>
        {active() ? "Remove filters" : "Apply filters"}
      </Button>
      <group row gap={16} align="center left">
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={8}
            fill="#0078d4"
            layer
            filterGrayscale={active() ? 1.0 : 0.0}
          >
            <text text="Aa" fontSize={24} color="#ffffff" />
          </rect>
          <CaptionLabel text="Grayscale" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={8}
            fill="#e74856"
            layer
            filterSepia={active() ? 1.0 : 0.0}
          >
            <text text="Aa" fontSize={24} color="#ffffff" />
          </rect>
          <CaptionLabel text="Sepia" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={8}
            fill="#00cc6a"
            layer
            filterInvert={active() ? 1.0 : 0.0}
          >
            <text text="Aa" fontSize={24} color="#ffffff" />
          </rect>
          <CaptionLabel text="Invert" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={8}
            fill="#744da9"
            layer
            filterHueRotate={active() ? 180 : 0}
          >
            <text text="Aa" fontSize={24} color="#ffffff" />
          </rect>
          <CaptionLabel text="Hue +180°" />
        </group>
        <group gap={4} align="top center">
          <rect
            w={80} h={80} cornerRadius={8}
            fill="#0078d4"
            layer
            filterBrightness={active() ? 1.5 : 1.0}
            filterContrast={active() ? 1.5 : 1.0}
          >
            <text text="Aa" fontSize={24} color="#ffffff" />
          </rect>
          <CaptionLabel text="Bright+Contrast" />
        </group>
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Effect · Layer Mask — alpha mask with mask={<element>}
// ---------------------------------------------------------------------------

const EffectLayerMaskDemo: Component = () => {
  return (
    <group row wrap gap={24} align="center left">
      <group gap={4} align="top center">
        <rect
          w={120} h={120}
          fill="#0078d4"
          layer
          mask={
            <rect
              w={120} h={120}
              cornerRadius={60}
              fill="#ffffff"
            />
          }
        >
          <text text="Masked!" fontSize={16} color="#ffffff" />
        </rect>
        <CaptionLabel text="Circle mask" />
      </group>
      <group gap={4} align="top center">
        <rect
          w={120} h={120}
          fill="#e74856"
          layer
          mask={
            <rect
              w={120} h={120}
              cornerRadius={24}
              fill="#ffffff"
            />
          }
        >
          <text text="Rounded" fontSize={16} color="#ffffff" />
        </rect>
        <CaptionLabel text="Rounded mask" />
      </group>
      {/* Small mask on larger content — tests mask smaller than parent */}
      <group gap={4} align="top center">
        <rect
          w={120} h={120}
          fill="#00cc6a"
          layer
          mask={
            <rect
              w={60} h={60}
              cornerRadius={30}
              fill="#ffffff"
              transformX={30} transformY={30}
            />
          }
        >
          <text text="Small" fontSize={14} color="#ffffff" />
        </rect>
        <CaptionLabel text="Small centered" />
      </group>
      {/* Offset mask — tests mask not at origin */}
      <group gap={4} align="top center">
        <rect
          w={120} h={120}
          fill="#744da9"
          layer
          mask={
            <rect
              w={80} h={80}
              cornerRadius={12}
              fill="#ffffff"
              transformX={20} transformY={20}
            />
          }
        >
          <text text="Offset" fontSize={14} color="#ffffff" />
        </rect>
        <CaptionLabel text="Offset 20,20" />
      </group>
    </group>
  )
}

// ---------------------------------------------------------------------------
// Effect · Vibrancy — frosted glass compositing
// ---------------------------------------------------------------------------

const EffectVibrancyDemo: Component = () => {
  return (
    <group gap={16}>
      {/* Hero scene — dark workspace with vibrancy panels */}
      <rect
        w={560}
        h={340}
        cornerRadius={16}
        clip
        fill={{
          type: "linearGradient",
          startX: 0, startY: 0, endX: 560, endY: 340,
          stops: [
            { offset: 0.0, color: "#0c0c18" },
            { offset: 0.4, color: "#141432" },
            { offset: 0.75, color: "#0a2a4a" },
            { offset: 1.0, color: "#061820" },
          ],
        }}
      >
        {/* Background shapes for blur to catch */}
        <circle cx={80} cy={60} r={72} fill="#e8456b" opacity={0.7} />
        <circle cx={440} cy={50} r={90} fill="#22c8e8" opacity={0.6} />
        <circle cx={320} cy={280} r={100} fill="#7c3aed" opacity={0.55} />
        <rect transformX={200} transformY={40} w={240} h={60} cornerRadius={12} fill="#ffffff14" />
        <rect transformX={200} transformY={120} w={300} h={12} cornerRadius={6} fill="#ffffff0c" />
        <rect transformX={200} transformY={144} w={260} h={12} cornerRadius={6} fill="#ffffff08" />

        {/* Sidebar — plus-lighter, heavy desaturation */}
        <rect
          transformX={16} transformY={16}
          w={160} h={308}
          cornerRadius={14}
          fill="#ffffff08"
          stroke="#ffffff30"
          strokeWidth={0.5}
          backdropBlur={20}
          layer
          vibrancyDesaturation={0.85}
          vibrancyBlendMode={3}
        >
          <text text="Library" transformX={14} transformY={14} fontSize={14} fontWeight={700} color="#ffffff" />
          <text text="plus-lighter · blur 20" transformX={14} transformY={34} fontSize={9} color="#ffffff80" />
          <rect transformX={14} transformY={52} w={132} h={0.5} fill="#ffffff20" />
          <text text="Recents" transformX={14} transformY={64} fontSize={11} color="#ffffff" />
          <text text="Shared" transformX={14} transformY={84} fontSize={11} color="#ffffffcc" />
          <text text="Notes" transformX={14} transformY={104} fontSize={11} color="#ffffffcc" />
        </rect>

        {/* Floating card — screen blend */}
        <rect
          transformX={360} transformY={130}
          w={170} h={120}
          cornerRadius={14}
          fill="#ffffff08"
          stroke="#ffffff30"
          strokeWidth={0.5}
          backdropBlur={14}
          layer
          vibrancyDesaturation={0.5}
          vibrancyBlendMode={1}
        >
          <text text="Signal" transformX={14} transformY={14} fontSize={13} fontWeight={700} color="#ffffff" />
          <text text="screen · blur 14" transformX={14} transformY={32} fontSize={9} color="#ffffff80" />
          <rect transformX={14} transformY={48} w={142} h={0.5} fill="#ffffff20" />
          <text text="Catches light" transformX={14} transformY={60} fontSize={11} color="#ffffff" />
          <text text="without mud" transformX={14} transformY={78} fontSize={11} color="#ffffffcc" />
        </rect>

        {/* Bottom tray — overlay blend */}
        <rect
          transformX={200} transformY={260}
          w={340} h={64}
          cornerRadius={14}
          fill="#ffffff08"
          stroke="#ffffff30"
          strokeWidth={0.5}
          backdropBlur={16}
          layer
          vibrancyDesaturation={0.65}
          vibrancyBlendMode={2}
        >
          <text text="Overlay tray" transformX={14} transformY={12} fontSize={13} fontWeight={700} color="#ffffff" />
          <text text="desaturation 0.65 · blur 16" transformX={14} transformY={32} fontSize={9} color="#ffffff80" />
        </rect>
      </rect>

      {/* Blend mode comparison strip */}
      <group row gap={8}>
        <For each={[
          { label: "multiply", mode: 0, blur: 10 },
          { label: "screen", mode: 1, blur: 14 },
          { label: "overlay", mode: 2, blur: 18 },
          { label: "plus-lighter", mode: 3, blur: 22 },
        ]}>
          {(item) => (
            <rect
              w={130} h={64}
              cornerRadius={12}
              fill={{
                type: "linearGradient",
                startX: 0, startY: 0, endX: 130, endY: 64,
                stops: [
                  { offset: 0.0, color: "#e8456b" },
                  { offset: 0.5, color: "#22c8e8" },
                  { offset: 1.0, color: "#34d399" },
                ],
              }}
            >
              <rect
                transformX={8} transformY={8}
                w={114} h={48}
                cornerRadius={10}
                fill="#ffffff08"
                stroke="#ffffff30"
                strokeWidth={0.5}
                backdropBlur={item.blur}
                layer
                vibrancyDesaturation={0.6}
                vibrancyBlendMode={item.mode}
              >
                <text text={item.label} transformX={10} transformY={10} fontSize={10} fontWeight={700} color="#ffffff" />
                <text text={`blur ${item.blur}`} transformX={10} transformY={26} fontSize={9} color="#ffffffaa" />
              </rect>
            </rect>
          )}
        </For>
      </group>
      <CaptionLabel text="Vibrancy: blur backdrop → desaturate → blend promoted layer foreground." />
    </group>
  )
}

// ---------------------------------------------------------------------------
// Effect · Fluent Materials — Acrylic & Mica approximations
// ---------------------------------------------------------------------------

const FluentMaterialsDemo: Component = () => {
  // Fluent Acrylic recipe: backdrop blur → exclusion blend (≈ screen) →
  // tint color overlay → noise texture (not yet available, use grain-like fill).
  // Mica: heavy blur + heavy desaturation, near-opaque tint, wallpaper-derived.

  return (
    <group gap={16}>
      {/* --- Acrylic: dark theme --- */}
      <group gap={4}>
        <text text="Acrylic · Dark" fontSize={13} fontWeight={700} color="#ffffffcc" />
        <rect
          w={480} h={260}
          cornerRadius={12}
          clip
          fill={{
            type: "linearGradient",
            startX: 0, startY: 0, endX: 480, endY: 260,
            stops: [
              { offset: 0.0, color: "#1b1b3a" },
              { offset: 0.5, color: "#2d1b69" },
              { offset: 1.0, color: "#0d4f6e" },
            ],
          }}
        >
          {/* Rich backdrop content */}
          <circle cx={60} cy={50} r={56} fill="#c026d3" opacity={0.5} />
          <circle cx={380} cy={40} r={70} fill="#06b6d4" opacity={0.45} />
          <circle cx={240} cy={200} r={80} fill="#4f46e5" opacity={0.4} />
          <rect transformX={140} transformY={30} w={200} h={40} cornerRadius={8} fill="#ffffff12" />
          <rect transformX={140} transformY={80} w={280} h={8} cornerRadius={4} fill="#ffffff08" />
          <rect transformX={140} transformY={96} w={220} h={8} cornerRadius={4} fill="#ffffff06" />

          {/* Acrylic sidebar panel — blur 30, screen blend, dark tint */}
          <rect
            transformX={12} transformY={12}
            w={180} h={236}
            cornerRadius={10}
            fill="#2a2a3a90"
            stroke="#ffffff18"
            strokeWidth={0.5}
            backdropBlur={30}
            layer
            vibrancyDesaturation={0.3}
            vibrancyBlendMode={1}
          >
            <text text="Navigation" transformX={14} transformY={14} fontSize={12} fontWeight={600} color="#ffffff" />
            <rect transformX={14} transformY={36} w={152} h={0.5} fill="#ffffff15" />
            <text text="Home" transformX={14} transformY={48} fontSize={11} color="#ffffffcc" />
            <text text="Projects" transformX={14} transformY={68} fontSize={11} color="#ffffffcc" />
            <text text="Settings" transformX={14} transformY={88} fontSize={11} color="#ffffff88" />
          </rect>

          {/* Acrylic flyout — transient surface */}
          <rect
            transformX={220} transformY={140}
            w={200} h={100}
            cornerRadius={10}
            fill="#3a3a5090"
            stroke="#ffffff20"
            strokeWidth={0.5}
            backdropBlur={30}
            layer
            vibrancyDesaturation={0.25}
            vibrancyBlendMode={1}
          >
            <text text="Quick actions" transformX={14} transformY={14} fontSize={11} fontWeight={600} color="#ffffff" />
            <rect transformX={14} transformY={34} w={172} h={0.5} fill="#ffffff15" />
            <text text="New file" transformX={14} transformY={46} fontSize={10} color="#ffffffcc" />
            <text text="Open recent" transformX={14} transformY={64} fontSize={10} color="#ffffffcc" />
            <text text="Import..." transformX={14} transformY={82} fontSize={10} color="#ffffff88" />
          </rect>
        </rect>
      </group>

      {/* --- Acrylic: light theme --- */}
      <group gap={4}>
        <text text="Acrylic · Light" fontSize={13} fontWeight={700} color="#ffffffcc" />
        <rect
          w={480} h={200}
          cornerRadius={12}
          clip
          fill={{
            type: "linearGradient",
            startX: 0, startY: 0, endX: 480, endY: 200,
            stops: [
              { offset: 0.0, color: "#e0e7ff" },
              { offset: 0.5, color: "#fce7f3" },
              { offset: 1.0, color: "#cffafe" },
            ],
          }}
        >
          <circle cx={100} cy={60} r={50} fill="#818cf8" opacity={0.3} />
          <circle cx={380} cy={140} r={60} fill="#22d3ee" opacity={0.25} />
          <rect transformX={40} transformY={30} w={180} h={30} cornerRadius={6} fill="#00000008" />
          <rect transformX={40} transformY={70} w={300} h={8} cornerRadius={4} fill="#00000006" />
          <rect transformX={40} transformY={86} w={240} h={8} cornerRadius={4} fill="#00000005" />

          {/* Light acrylic panel — tinted white */}
          <rect
            transformX={240} transformY={16}
            w={220} h={168}
            cornerRadius={10}
            fill="#ffffffb0"
            stroke="#00000012"
            strokeWidth={0.5}
            backdropBlur={30}
            layer
            vibrancyDesaturation={0.2}
            vibrancyBlendMode={1}
          >
            <text text="Properties" transformX={14} transformY={14} fontSize={12} fontWeight={600} color="#1a1a2e" />
            <rect transformX={14} transformY={34} w={192} h={0.5} fill="#00000010" />
            <text text="Name: Document.md" transformX={14} transformY={48} fontSize={10} color="#333333" />
            <text text="Size: 4.2 KB" transformX={14} transformY={66} fontSize={10} color="#333333" />
            <text text="Modified: today" transformX={14} transformY={84} fontSize={10} color="#555555" />
          </rect>
        </rect>
      </group>

      {/* --- Mica approximation --- */}
      <group gap={4}>
        <text text="Mica (approximation)" fontSize={13} fontWeight={700} color="#ffffffcc" />
        <rect
          w={480} h={160}
          cornerRadius={12}
          clip
          fill={{
            type: "linearGradient",
            startX: 0, startY: 0, endX: 480, endY: 160,
            stops: [
              { offset: 0.0, color: "#6366f1" },
              { offset: 0.35, color: "#a855f7" },
              { offset: 0.7, color: "#ec4899" },
              { offset: 1.0, color: "#f97316" },
            ],
          }}
        >
          {/* Mica: near-opaque, heavy desaturation — wallpaper subtly tints */}
          <rect
            transformX={0} transformY={0}
            w={480} h={160}
            fill="#20202880"
            backdropBlur={60}
            layer
            vibrancyDesaturation={0.92}
            vibrancyBlendMode={0}
          >
            {/* App content on mica base */}
            <text text="App title bar" transformX={16} transformY={14} fontSize={12} fontWeight={600} color="#ffffffcc" />
            <rect transformX={16} transformY={38} w={448} h={0.5} fill="#ffffff15" />
            {/* Content layer card */}
            <rect transformX={16} transformY={50} w={448} h={94} cornerRadius={8} fill="#ffffff08" stroke="#ffffff10" strokeWidth={0.5}>
              <text text="Content area" transformX={14} transformY={14} fontSize={11} fontWeight={600} color="#ffffffcc" />
              <text text="Mica provides a subtle, personalized backdrop" transformX={14} transformY={34} fontSize={10} color="#ffffff88" />
              <text text="derived from the desktop wallpaper." transformX={14} transformY={50} fontSize={10} color="#ffffff88" />
            </rect>
          </rect>
        </rect>
      </group>

      <CaptionLabel text="Fluent materials: Acrylic = blur 30 + screen blend + tint overlay. Mica = heavy blur + desaturation." />
    </group>
  )
}

// ---------------------------------------------------------------------------
// Context Menu demo
// ---------------------------------------------------------------------------

const ContextMenuDemo: Component = () => {
  const theme = useTheme()
  const [lastAction, setLastAction] = createSignal("(right-click the box)")

  const { onContextMenu, menu } = useContextMenu({
    items: () => [
      { text: "Cut", icon: "M19 3l-6 6m-1 1L4 18V20H6L14 12M19 3l2 2L14 12M19 3l-4 0M5 12H3v2", onClick: () => setLastAction("Cut") },
      { text: "Copy", onClick: () => setLastAction("Copy") },
      { text: "Paste", onClick: () => setLastAction("Paste") },
      { text: "Delete", disabled: true },
    ],
    width: 160,
  })

  return (
    <group gap={12}>
      <rect
        fill={theme().backgroundSecondary}
        stroke={theme().strokeDefault}
        strokeWidth={1}
        cornerRadius={8}
        w={280}
        h={160}
       
        align="center"
       
        gap={8}
        onContextMenu={onContextMenu}
      >
        <text text="Right-click here" fontSize={14} color={theme().foregroundPrimary} />
        <text text={lastAction()} fontSize={12} color={theme().foregroundSecondary} />
      </rect>
      {menu()}
    </group>
  )
}

// ---------------------------------------------------------------------------
// Interactive demos — stateful wrappers for verifying real behavior
// ---------------------------------------------------------------------------

const InteractiveToggle: Component = () => {
  const [checked, setChecked] = createSignal(false)
  return (
    <group row gap={12} align="center left">
      <Toggle checked={checked()} onChange={setChecked} />
      <BodyLabel text={checked() ? "ON" : "OFF"} />
    </group>
  )
}

const InteractiveCheckBox: Component = () => {
  const [a, setA] = createSignal(false)
  const [b, setB] = createSignal(true)
  return (
    <group gap={8}>
      <CheckBox label="Option A" checked={a()} onChange={setA} />
      <CheckBox label="Option B" checked={b()} onChange={setB} />
      <CaptionLabel text={`A=${a()}, B=${b()}`} />
    </group>
  )
}

const InteractiveRadioButton: Component = () => {
  const [selected, setSelected] = createSignal(0)
  const options = ["Alpha", "Beta", "Gamma"]
  return (
    <group gap={8}>
      <Index each={options}>
        {(label, i) => (
          <RadioButton
            label={label()}
            checked={selected() === i}
            onChange={() => setSelected(i)}
          />
        )}
      </Index>
      <CaptionLabel text={`Selected: ${options[selected()]}`} />
    </group>
  )
}

const InteractiveSlider: Component = () => {
  const [value, setValue] = createSignal(50)
  return (
    <group gap={8}>
      <Slider value={value()} width={200} onChange={setValue} />
      <CaptionLabel text={`Value: ${value().toFixed(0)}`} />
    </group>
  )
}

const InteractiveLineEdit: Component = () => {
  const [text, setText] = createSignal("")
  const [submitted, setSubmitted] = createSignal("")
  return (
    <group gap={8}>
      <LineEdit
        value={text()}
        placeholder="Type and press Enter..."
        width={220}
        onChange={setText}
        onSubmit={() => setSubmitted(text())}
      />
      <CaptionLabel text={`Live: "${text()}"`} />
      <Show when={submitted()}>
        <CaptionLabel text={`Submitted: "${submitted()}"`} />
      </Show>
    </group>
  )
}

const InteractiveToggleButton: Component = () => {
  const [checked, setChecked] = createSignal(false)
  return (
    <group row gap={12} align="center left">
      <ToggleButton checked={checked()} onChange={setChecked}>
        {checked() ? "Active" : "Inactive"}
      </ToggleButton>
      <CaptionLabel text={checked() ? "ON" : "OFF"} />
    </group>
  )
}

const VirtualListDemo: Component = () => {
  const theme = useTheme()

  const itemCount = 10_000
  const itemHeight = 42

  const rowColor = (index: number) =>
    index % 2 === 0 ? theme().backgroundDefault : theme().backgroundSecondary

  return (
    <group gap={10}>
      <group row gap={8} align="center left">
        <InfoBadge level="attention" />
        <CaptionLabel text={`${itemCount.toLocaleString()} rows · ${itemHeight}px row height · overscan 4`} />
      </group>
      <rect
        w={520}
        h={320}
        cornerRadius={8}
        clip
        fill={theme().backgroundSecondary}
        stroke={theme().strokeDefault}
        strokeWidth={1}
      >
        <VirtualList
          itemCount={itemCount}
          itemHeight={itemHeight}
          overscan={4}
          w="fill"
          h="fill"
          renderItem={(index) => (
            <rect
              h={itemHeight}
              w="fill"
              row
              align="center left"
              paddingLeft={12}
              paddingRight={12}
              fill={rowColor(index)}
            >
              <text
                text={`Row ${index.toString().padStart(5, "0")}`}
                fontSize={13}
                fontWeight={index % 100 === 0 ? 700 : 400}
                color={theme().foregroundPrimary}
              />
              <text
                text={index % 100 === 0 ? "checkpoint" : "virtualized item"}
                fontSize={11}
                color={theme().foregroundSecondary}
                transformX={156}
              />
            </rect>
          )}
        />
      </rect>
      <CaptionLabel text="Wheel inside the frame. First and last rows should clamp without blank space." />
    </group>
  )
}

// ---------------------------------------------------------------------------
// Routing · Router + Outlet + Breadcrumb demo
// ---------------------------------------------------------------------------

const RoutingHomeView: Component = () => {
  const theme = useTheme()
  const nav = useNavigate()
  return (
    <group gap={12} padding={16}>
      <SubtitleLabel text="Home" />
      <BodyLabel text="Welcome to the routing demo. Pick a section:" />
      <group row gap={8}>
        <Button onClick={() => nav.push("/settings/general")}>Settings</Button>
        <Button onClick={() => nav.push("/users/42")}>User #42</Button>
        <Button onClick={() => nav.push("/users/7")}>User #7</Button>
      </group>
    </group>
  )
}

const RoutingSettingsView: Component = () => {
  const theme = useTheme()
  return (
    <group gap={8} padding={16}>
      <SubtitleLabel text="Settings" />
      <Outlet />
    </group>
  )
}

const RoutingGeneralView: Component = () => (
  <group gap={8}>
    <BodyLabel text="General settings page" />
    <group row gap={8} align="center left">
      <BodyLabel text="Notifications" />
      <Toggle checked={true} />
    </group>
    <group row gap={8} align="center left">
      <BodyLabel text="Dark mode auto-switch" />
      <Toggle checked={false} />
    </group>
  </group>
)

const RoutingAccountsView: Component = () => (
  <group gap={8}>
    <BodyLabel text="Accounts settings page" />
    <BodyLabel text="user@example.com" />
  </group>
)

const RoutingAboutView: Component = () => (
  <group gap={8}>
    <BodyLabel text="About this app" />
    <CaptionLabel text="qt-solid storyboard v0.0.0" />
  </group>
)

const RoutingUserView: Component = () => {
  const params = useParams()
  return (
    <group gap={8} padding={16}>
      <SubtitleLabel text={`User Profile: #${params().id ?? "?"}`} />
      <BodyLabel text={`Viewing user with id = ${params().id ?? "unknown"}`} />
    </group>
  )
}

const ROUTING_DEMO_ROUTES: RouteDefinition[] = [
  { path: "/", component: RoutingHomeView },
  {
    path: "/settings",
    component: RoutingSettingsView,
    children: [
      { path: "/general", component: RoutingGeneralView },
      { path: "/accounts", component: RoutingAccountsView },
      { path: "/about", component: RoutingAboutView },
    ],
  },
  { path: "/users/:id", component: RoutingUserView },
]

const RoutingDemo: Component = () => {
  const theme = useTheme()

  return (
    <Router routes={ROUTING_DEMO_ROUTES} initial="/">
      <RoutingDemoChrome />
    </Router>
  )
}

const RoutingDemoChrome: Component = () => {
  const theme = useTheme()
  const nav = useNavigate()
  const location = useLocation()
  const canGoBack = useCanGoBack()
  const crumbs = useBreadcrumbs()

  const breadcrumbItems = createMemo(() =>
    crumbs().map((c) => ({ key: c.path, text: c.label })),
  )

  // Sidebar navigation items for the demo
  const sidebarItems = [
    { key: "/", label: "Home" },
    { key: "/settings/general", label: "General" },
    { key: "/settings/accounts", label: "Accounts" },
    { key: "/settings/about", label: "About" },
  ]

  return (
    <group gap={8}>
      {/* Top bar: back button + breadcrumb */}
      <group row align="center left" gap={8}>
        <Button
          onClick={() => nav.pop()}
          disabled={!canGoBack()}
        >
          ← Back
        </Button>
        <Breadcrumb
          items={breadcrumbItems()}
          onSelect={(key) => nav.push(key)}
        />
      </group>

      <group row gap={8}>
        {/* Mini sidebar */}
        <group gap={4} w={140}>
          <For each={sidebarItems}>
            {(item) => (
              <Button
                accent={location() === item.key || location().startsWith(item.key + "/")}
                onClick={() => nav.push(item.key)}
              >
                {item.label}
              </Button>
            )}
          </For>
        </group>

        {/* Outlet area */}
        <rect
          fill={theme().backgroundSecondary}
          cornerRadius={8}
          h="fill"
          minHeight={200}
         
        >
          <Outlet />
        </rect>
      </group>

      <CaptionLabel text={`Current location: ${location()}`} />
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
    interactive: () => <InteractiveToggle />,
  },
  {
    name: "CheckBox",
    render: (p) => <CheckBox {...p as any} />,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: { label: "Option" },
    interactive: () => <InteractiveCheckBox />,
  },
  {
    name: "RadioButton",
    render: (p) => <RadioButton {...p as any} />,
    axes: { checked: [false, true], disabled: [false, true] },
    defaults: { label: "Choice" },
    interactive: () => <InteractiveRadioButton />,
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
    interactive: () => <InteractiveSlider />,
  },
  {
    name: "LineEdit",
    render: (p) => <LineEdit {...p as any} />,
    axes: { disabled: [false, true], error: [false, true] },
    defaults: { placeholder: "Type here...", width: 180 },
    interactive: () => <InteractiveLineEdit />,
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
    interactive: () => <InteractiveToggleButton />,
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
      <group gap={4}>
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
      <group row gap={16} align="center left" h={40}>
        <BodyLabel text="Left" />
        <VerticalSeparator length={30} />
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
  {
    name: "Effect · 3D Transform",
    render: () => <Effect3DTransformDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Effect · Content Filters",
    render: () => <EffectContentFiltersDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Effect · Layer Mask",
    render: () => <EffectLayerMaskDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Effect · Vibrancy",
    render: () => <EffectVibrancyDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Effect · Fluent Materials",
    render: () => <FluentMaterialsDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Context Menu",
    render: () => <ContextMenuDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Virtual List",
    render: () => <VirtualListDemo />,
    axes: {},
    defaults: {},
  },
  {
    name: "Routing",
    render: () => <RoutingDemo />,
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
      h={32}
      row
      align="center left"
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
    <group gap={6} padding={8} minWidth={CELL_MIN_WIDTH} w="fill">
      <text text={props.label} fontSize={10} color={chrome.dim()} />
      <rect
        fill="transparent"
        stroke={chrome.border()}
        strokeWidth={1}
        cornerRadius={6}
        padding={12}
       
       
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
      row={!useColumn()}
      wrap={!useColumn()}
      gap={gap()}
      w="100%"
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
    <group gap={16} padding={20} w="100%">
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
        <rect h={1} fill={chrome.border()} />
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

      {/* Interactive */}
      <Show when={props.story.interactive}>
        <rect h={1} fill={chrome.border()} />
        <text text="Interactive" fontSize={14} fontWeight={600} color={chrome.text()} />
        <rect
          fill="transparent"
          stroke={chrome.border()}
          strokeWidth={1}
          cornerRadius={6}
          padding={16}
         
         
        >
          {props.story.interactive!()}
        </rect>
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
      h={28}
      row
      align="center"
     
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
    <rect fill={chrome.bg()} w="100%" h="100%" row>
      {/* Sidebar */}
      <rect
        w={200}
        fill={chrome.sidebar()}
       
      >
        {/* Sidebar header */}
        <rect
          h={48}
          row
          align="center left"
          paddingLeft={12}
          paddingRight={12}
          gap={8}
        >
          <rect fill={chrome.accent()} w={8} h={8} cornerRadius={4} />
          <text text="Storyboard" fontSize={13} fontWeight={600} color={chrome.text()} />
        </rect>
        <rect h={1} fill={chrome.border()} />

        {/* Story list */}
        <ScrollView h="fill">
          <group gap={2} padding={6}>
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
        <rect h={1} fill={chrome.border()} />
        <rect h={44} row align="center">
          <ThemeToggle isDark={props.isDark} onToggle={() => props.setIsDark((v) => !v)} />
        </rect>
      </rect>

      {/* Divider */}
      <rect w={1} fill={chrome.border()} />

      {/* Content area */}
      <rect fill={chrome.bg()} w="fill">
        <ScrollView h="fill">
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
