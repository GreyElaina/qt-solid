export { createApp, renderQt } from "./app.ts";
export { withQtSourceMeta } from "./source-meta.ts";

// motion
export {
  useMotionValue,
  createVariants,
  AnimatePresence,
} from "./motion/index.ts";
export type {
  MotionTarget,
  MotionTransition,
  MotionValue,
  MotionProps,
  NamedEasing,
  BezierEasing,
  TransitionSpec,
  DragConstraints,
  MotionValueConfig,
} from "./motion/index.ts";

// windowing
export {
  createWindow,
  createPopup,
  usePopup,
  useTooltip,
  openFileDialog,
  saveFileDialog,
} from "./windowing/index.ts";
export type {
  UseTooltipOptions,
  UseTooltipResult,
  OpenFileDialogOptions,
  SaveFileDialogOptions,
  WindowProps,
  PopupDismissEvent,
  PopupProps,
  PopupSource,
  TooltipProps,
  WindowSource,
  WindowComposable,
  WindowConfig,
  WindowConfigSource,
} from "./windowing/index.ts";

// components
export { ScrollView, VirtualList, Image, Canvas } from "./components/index.ts";
export type {
  ScrollViewProps,
  VirtualListProps,
  ImageProps,
  CanvasProps,
} from "./components/index.ts";

// platform
export {
  useClipboard,
  useColorScheme,
  useScreenDpi,
} from "./platform/index.ts";
export type {
  ClipboardEntry,
  UseClipboardResult,
  ColorScheme,
  ScreenDpi,
} from "./platform/index.ts";

// routing
export {
  Router,
  Outlet,
  useLocation,
  useNavigate,
  useParams,
  useCanGoBack,
  useStack,
  useBreadcrumbs,
  matchRoutes,
} from "./routing/index.ts";
export type {
  RouteDefinition,
  BranchEntry,
  StackEntry,
  NavigateFn,
  RouterContextState,
  OutletDepthState,
  RouterProps,
} from "./routing/index.ts";

// app-level types
export type {
  AppDefinition,
  AppFactory,
  AppHandle,
  AppMount,
  AppMountOptions,
  CreateAppOptions,
  RenderQtOptions,
  ViewProps,
  WindowAllClosedContext,
  WindowHandle,
  WindowFrameState,
} from "./types.ts";

export type {
  CommonProps,
  EventProps,
  TransformProps,
  VisualProps,
  FilterProps,
  LayoutProps,
  SizingValue,
  GroupProps,
  RectProps,
  CircleProps,
  TextProps,
  TextInputProps,
  PathProps,
  WheelEventPayload,
} from "../intrinsics.ts";

export type { FragmentRendererNode } from "../runtime/fragment.ts";
