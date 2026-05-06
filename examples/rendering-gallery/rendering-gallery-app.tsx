import {
  createApp,
  createWindow,
  ScrollView,
  type AppHandle,
  type WindowHandle,
} from "@qt-solid/solid"

// ---------------------------------------------------------------------------
// Palette (Catppuccin Mocha)
// ---------------------------------------------------------------------------

const BG      = "#1e1e2e"
const SURFACE = "#313244"
const OVERLAY = "#45475a"
const TEXT_   = "#cdd6f4"
const SUBTEXT = "#a6adc8"
const RED     = "#f38ba8"
const GREEN   = "#a6e3a1"
const BLUE    = "#89b4fa"
const MAUVE   = "#cba6f7"
const PEACH   = "#fab387"
const TEAL    = "#94e2d5"
const PINK    = "#f5c2e7"
const YELLOW  = "#f9e2af"

// ---------------------------------------------------------------------------
// Rendering Gallery Window
// ---------------------------------------------------------------------------

function createRenderingGalleryWindow(): WindowHandle {
  return createWindow(
    {
      title: "Rendering Feature Gallery",
      width: 640,
      height: 800,
    },
    () => (
      <rect fill={BG} h="fill">
        <ScrollView h="fill">
        <group padding={20} gap={16}>

            <text text="qt-solid · Rendering Feature Gallery" fontSize={14} color={TEXT_} />

            {/* ============================================================= */}
            {/* Section 1: Gradients (T2.1)                                   */}
            {/* ============================================================= */}
            <text text="Gradients" fontSize={18} color={MAUVE} />
            <text text="Linear, radial, and sweep gradient fills" fontSize={12} color={SUBTEXT} />

            <group row wrap gap={20}>
              {/* Linear gradient — diagonal blue→purple */}
              <group gap={4}>
                <rect w={120} h={80} fill={{
                  type: "linearGradient",
                  startX: 0, startY: 0,
                  endX: 120, endY: 80,
                  stops: [{ offset: 0, color: BLUE }, { offset: 1, color: MAUVE }],
                }} cornerRadius={8} />
                <text text="Linear" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Radial gradient — warm center→edge */}
              <group gap={4}>
                <rect w={120} h={120} fill={{
                  type: "radialGradient",
                  centerX: 60, centerY: 60,
                  radius: 60,
                  stops: [{ offset: 0, color: YELLOW }, { offset: 1, color: RED }],
                }} cornerRadius={8} />
                <text text="Radial" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Sweep gradient — rainbow ring */}
              <group gap={4}>
                <rect w={120} h={120} fill={{
                  type: "sweepGradient",
                  centerX: 60, centerY: 60,
                  startAngle: 0, endAngle: 360,
                  stops: [
                    { offset: 0, color: RED },
                    { offset: 0.33, color: GREEN },
                    { offset: 0.66, color: BLUE },
                    { offset: 1, color: RED },
                  ],
                }} cornerRadius={60} />
                <text text="Sweep" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Horizontal pill */}
              <group gap={4}>
                <rect w={180} h={80} fill={{
                  type: "linearGradient",
                  startX: 0, startY: 0,
                  endX: 180, endY: 0,
                  stops: [
                    { offset: 0, color: TEAL },
                    { offset: 0.5, color: PINK },
                    { offset: 1, color: PEACH },
                  ],
                }} cornerRadius={40} />
                <text text="Horizontal pill" fontSize={10} color={SUBTEXT} />
              </group>
            </group>

            {/* Separator */}
            <rect h={1} fill={OVERLAY} />

            {/* ============================================================= */}
            {/* Section 2: Shadows (T2.2)                                     */}
            {/* ============================================================= */}
            <text text="Shadows" fontSize={18} color={MAUVE} />
            <text text="Outer shadow vs inner (inset) shadow" fontSize={12} color={SUBTEXT} />

            <group row gap={40}>
              {/* Outer shadow */}
              <group gap={8}>
                <rect w={160} h={80} fill={SURFACE} cornerRadius={10}
                  shadow={{ offsetX: 4, offsetY: 4, blur: 12, color: "#00000060" }} />
                <text text="Outer shadow" fontSize={11} color={SUBTEXT} />
              </group>

              {/* Inner shadow */}
              <group gap={8}>
                <rect w={160} h={80} fill={SURFACE} cornerRadius={10}
                  shadow={{ offsetX: 2, offsetY: 2, blur: 8, color: "#00000080", inset: true }} />
                <text text="Inner shadow" fontSize={11} color={SUBTEXT} />
              </group>

              {/* Comparison card */}
              <group gap={8}>
                <rect w={160} h={80} fill={SURFACE} cornerRadius={10}
                  shadow={{ offsetX: 3, offsetY: 3, blur: 10, color: "#00000070" }} />
                <text text="Card with drop shadow" fontSize={11} color={SUBTEXT} />
              </group>
            </group>

            {/* Separator */}
            <rect h={1} fill={OVERLAY} />

            {/* ============================================================= */}
            {/* Section 3: Backdrop Blur (T2.3)                               */}
            {/* ============================================================= */}
            <text text="Backdrop Blur" fontSize={18} color={MAUVE} />
            <text text="Frosted glass effect over colorful background" fontSize={12} color={SUBTEXT} />

            <group row gap={20}>
              {/* Demo 1: moderate blur */}
              <group w={280} h={120}>
                <rect w={280} h={120} fill={RED} cornerRadius={8} />
                <rect transformX={40} transformY={20} w={80} h={80} fill={BLUE} cornerRadius={40} />
                <rect transformX={140} transformY={10} w={100} h={60} fill={GREEN} cornerRadius={12} />
                <circle cx={240} cy={80} r={30} fill={YELLOW} />

                <rect transformX={30} transformY={20} w={220} h={80} fill="#1e1e2e80"
                  cornerRadius={12} backdropBlur={12} />
                <text transformX={50} transformY={46} text="Frosted Glass" fontSize={14} color={TEXT_} />
                <text transformX={50} transformY={66} text="backdropBlur={12}" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Demo 2: heavy blur */}
              <group w={240} h={120}>
                <rect w={240} h={120} fill={MAUVE} cornerRadius={8} />
                <circle cx={50} cy={60} r={40} fill={PEACH} />
                <circle cx={150} cy={50} r={35} fill={TEAL} />

                <rect transformX={20} transformY={20} w={200} h={80} fill="#1e1e2e60"
                  cornerRadius={12} backdropBlur={24} />
                <text transformX={40} transformY={48} text="Heavy Blur" fontSize={14} color={TEXT_} />
                <text transformX={40} transformY={68} text="backdropBlur={24}" fontSize={10} color={SUBTEXT} />
              </group>
            </group>

            {/* Separator */}
            <rect h={1} fill={OVERLAY} />

            {/* ============================================================= */}
            {/* Section 4: Blend Modes (T2.4)                                 */}
            {/* ============================================================= */}
            <text text="Blend Modes" fontSize={18} color={MAUVE} />
            <text text="Overlapping shapes with different compositing" fontSize={12} color={SUBTEXT} />

            <group row gap={30}>
              {/* Multiply */}
              <group gap={4}>
                <group w={105} h={70}>
                  <rect w={70} h={70} fill={RED} cornerRadius={6} />
                  <rect transformX={35} transformY={0} w={70} h={70} fill={BLUE} cornerRadius={6} blendMode="multiply" />
                </group>
                <text text="multiply" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Screen */}
              <group gap={4}>
                <group w={105} h={70}>
                  <rect w={70} h={70} fill={RED} cornerRadius={6} />
                  <rect transformX={35} transformY={0} w={70} h={70} fill={BLUE} cornerRadius={6} blendMode="screen" />
                </group>
                <text text="screen" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Overlay */}
              <group gap={4}>
                <group w={105} h={70}>
                  <rect w={70} h={70} fill={GREEN} cornerRadius={6} />
                  <rect transformX={35} transformY={0} w={70} h={70} fill={MAUVE} cornerRadius={6} blendMode="overlay" />
                </group>
                <text text="overlay" fontSize={10} color={SUBTEXT} />
              </group>

              {/* Difference */}
              <group gap={4}>
                <group w={105} h={70}>
                  <rect w={70} h={70} fill={PEACH} cornerRadius={6} />
                  <rect transformX={35} transformY={0} w={70} h={70} fill={TEAL} cornerRadius={6} blendMode="difference" />
                </group>
                <text text="difference" fontSize={10} color={SUBTEXT} />
              </group>
            </group>

            {/* Separator */}
            <rect h={1} fill={OVERLAY} />

            {/* ============================================================= */}
            {/* Section 5: Rich Text (T2.7)                                   */}
            {/* ============================================================= */}
            <text text="Rich Text" fontSize={18} color={MAUVE} />
            <text text="Mixed styles via span children" fontSize={12} color={SUBTEXT} />

            <text fontSize={16}>
              <span text="Bold " fontWeight={700} color={RED} />
              <span text="and " color={TEXT_} />
              <span text="Italic " fontStyle="italic" color={BLUE} />
              <span text="mixed " fontSize={20} color={GREEN} />
              <span text="text" color={MAUVE} />
            </text>

            <text fontSize={14}>
              <span text="Small " fontSize={11} color={SUBTEXT} />
              <span text="Medium " fontSize={14} color={TEXT_} />
              <span text="Large " fontSize={20} color={PEACH} fontWeight={600} />
              <span text="Tiny" fontSize={9} color={TEAL} />
            </text>

            {/* Bottom padding */}
            <group h={20} />
          </group>
        </ScrollView>
      </rect>
    ),
  )
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

export function createRenderingGalleryApp(): AppHandle {
  return createApp(() => {
    const mainWindow = createRenderingGalleryWindow()

    return {
      render: () => mainWindow.render(),
      onActivate() {
        mainWindow.open()
      },
    }
  })
}
