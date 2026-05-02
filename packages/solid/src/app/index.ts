export { createRuntimeElement as createIntrinsicElement } from "./props.ts";
export { defineIntrinsicComponent } from "./intrinsic.ts";
export { createApp, renderQt } from "./app.ts";
export { withQtSourceMeta } from "./source-meta.ts";

// motion
export {
  motion,
  __testMotionInternals,
  useMotionValue,
  createVariants,
  AnimatePresence,
  setLayoutId,
  unsetLayoutId,
} from "./motion/index.ts";
export type {
  MotionValueConfig,
  PresenceContextState,
  OrchestrationConfig,
  OrchestrationContextState,
  OrchestrationParentControl,
  MotionComponentProps,
  MotionTarget,
  MotionTransition,
  MotionValue,
  MotionProps,
  NamedEasing,
  BezierEasing,
  TransitionSpec,
  DragConstraints,
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
export { ScrollView, Image, Canvas } from "./components/index.ts";
export type {
  ScrollViewProps,
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
  WidgetProps,
  WindowAllClosedContext,
  WindowHandle,
  WindowFrameState,
} from "./types.ts";

export type {
  CanvasCommonProps,
  CanvasGroupProps,
  CanvasRectProps,
  CanvasCircleProps,
  CanvasTextProps,
  CanvasTextInputProps,
  CanvasPathProps,
  CanvasNodeHandle,
  WheelEventPayload,
} from "../qt-intrinsics.ts";
