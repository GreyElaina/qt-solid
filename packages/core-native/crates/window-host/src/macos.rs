use std::{cell::Cell, rc::Rc};

use block2::RcBlock;
use objc2::{MainThreadMarker, rc::autoreleasepool};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventSubtype, NSEventType};
use objc2_core_foundation::{
    CFIndex, CFRetained, CFRunLoop, CFRunLoopActivity, CFRunLoopObserver, kCFAllocatorDefault,
    kCFRunLoopCommonModes,
};
use objc2_foundation::NSPoint;

use crate::{
    WindowHostError,
    host::{BackendKind, HostIntegration, PumpResult, WaitBridgeKind},
};

fn trace_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(args: std::fmt::Arguments<'_>) {
    if !trace_enabled() {
        return;
    }
    println!("[qt-window-host] {args}");
}

#[derive(Debug)]
pub(crate) struct MacosWindowHost {
    app: objc2::rc::Retained<NSApplication>,
    state: Rc<HostState>,
    before_waiting_observer: MainRunLoopObserver,
    main_run_loop: CFRetained<CFRunLoop>,
}

impl MacosWindowHost {
    pub(crate) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::Macos,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        wait_bridge_kind: WaitBridgeKind::None,
    };

    pub(crate) fn new() -> Result<Self, WindowHostError> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| WindowHostError::new("window-host must start on macOS main thread"))?;
        let main_run_loop = CFRunLoop::main()
            .ok_or_else(|| WindowHostError::new("failed to get macOS main CFRunLoop"))?;
        let app = NSApplication::sharedApplication(mtm);
        let state = Rc::new(HostState::new(mtm));

        let state_clone = Rc::clone(&state);
        let before_waiting_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::BeforeWaiting,
            true,
            CFIndex::MAX - 1,
            move |_| state_clone.before_waiting(),
        )?;

        // SAFETY: Observer lives on main thread and only attaches to main loop.
        unsafe {
            main_run_loop.add_observer(Some(before_waiting_observer.raw()), kCFRunLoopCommonModes);
        }

        Ok(Self {
            app,
            state,
            before_waiting_observer,
            main_run_loop,
        })
    }

    pub(crate) fn backend_kind(&self) -> BackendKind {
        Self::INTEGRATION.backend_kind
    }

    pub(crate) fn integration(&self) -> HostIntegration {
        Self::INTEGRATION
    }

    pub(crate) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        if MainThreadMarker::new().is_none() {
            return Err(WindowHostError::new(
                "window-host pump must run on macOS main thread",
            ));
        }

        if self.state.take_native_wait_request() {
            trace(format_args!("pump-native-wait begin"));
            self.state.begin_native_wait_pump();
        } else {
            trace(format_args!("pump-zero-timeout begin"));
            self.state.begin_zero_timeout_pump();
        }
        autoreleasepool(|_| {
            self.app.run();
        });
        let pump_label = self.state.pump_label();
        self.state.end_pump();
        trace(format_args!("{pump_label} end"));
        Ok(PumpResult {
            pumped_native: true,
        })
    }

    pub(crate) fn request_wake(&self) {
        trace(format_args!("request-wake"));
        self.main_run_loop.wake_up();
    }

    pub(crate) fn request_native_wait_once(&self) {
        trace(format_args!("request-native-wait-once"));
        self.state.request_native_wait_once();
        self.main_run_loop.wake_up();
    }

    pub(crate) fn notify_native_frame_source(&self) {
        self.state.notify_native_frame_source();
    }
}

impl Drop for MacosWindowHost {
    fn drop(&mut self) {
        // SAFETY: Same reasoning as in `new`; observer only ever lives on main run loop.
        unsafe {
            self.main_run_loop.remove_observer(
                Some(self.before_waiting_observer.raw()),
                kCFRunLoopCommonModes,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpMode {
    Idle,
    ZeroTimeout,
    NativeWaitOnce,
}

#[derive(Debug)]
struct HostState {
    mtm: MainThreadMarker,
    pumping: Cell<bool>,
    pump_mode: Cell<PumpMode>,
    native_wait_requested: Cell<bool>,
    native_frame_source_delivered: Cell<bool>,
}

impl HostState {
    fn new(mtm: MainThreadMarker) -> Self {
        Self {
            mtm,
            pumping: Cell::new(false),
            pump_mode: Cell::new(PumpMode::Idle),
            native_wait_requested: Cell::new(false),
            native_frame_source_delivered: Cell::new(false),
        }
    }

    fn begin_zero_timeout_pump(&self) {
        self.pumping.set(true);
        self.pump_mode.set(PumpMode::ZeroTimeout);
        self.native_frame_source_delivered.set(false);
    }

    fn begin_native_wait_pump(&self) {
        self.pumping.set(true);
        self.pump_mode.set(PumpMode::NativeWaitOnce);
        self.native_frame_source_delivered.set(false);
    }

    fn end_pump(&self) {
        self.pump_mode.set(PumpMode::Idle);
        self.native_frame_source_delivered.set(false);
        self.pumping.set(false);
    }

    fn request_native_wait_once(&self) {
        self.native_wait_requested.set(true);
    }

    fn take_native_wait_request(&self) -> bool {
        let requested = self.native_wait_requested.get();
        self.native_wait_requested.set(false);
        requested
    }

    fn pump_label(&self) -> &'static str {
        match self.pump_mode.get() {
            PumpMode::Idle => "pump-idle",
            PumpMode::ZeroTimeout => "pump-zero-timeout",
            PumpMode::NativeWaitOnce => "pump-native-wait",
        }
    }

    fn should_stop_before_waiting(&self) -> bool {
        if !self.pumping.get() {
            return false;
        }

        match self.pump_mode.get() {
            PumpMode::Idle => false,
            PumpMode::ZeroTimeout => true,
            PumpMode::NativeWaitOnce => self.native_frame_source_delivered.get(),
        }
    }

    fn before_waiting(&self) {
        if !self.should_stop_before_waiting() {
            return;
        }

        stop_app_immediately(&NSApplication::sharedApplication(self.mtm));
    }

    fn notify_native_frame_source(&self) {
        if !self.pumping.get() {
            return;
        }
        if self.pump_mode.get() == PumpMode::NativeWaitOnce {
            self.native_frame_source_delivered.set(true);
        }
    }
}

#[derive(Debug)]
struct MainRunLoopObserver {
    observer: CFRetained<CFRunLoopObserver>,
}

impl MainRunLoopObserver {
    fn new(
        mtm: MainThreadMarker,
        activities: CFRunLoopActivity,
        repeats: bool,
        order: CFIndex,
        callback: impl Fn(CFRunLoopActivity) + 'static,
    ) -> Result<Self, WindowHostError> {
        let block = RcBlock::new(move |_: *mut _, activity| {
            debug_assert!(MainThreadMarker::new().is_some());
            callback(activity);
        });

        let _ = mtm;
        let observer = unsafe {
            CFRunLoopObserver::with_handler(
                kCFAllocatorDefault,
                activities.0,
                repeats,
                order,
                Some(&block),
            )
        }
        .ok_or_else(|| WindowHostError::new("failed to create CFRunLoopObserver"))?;

        Ok(Self { observer })
    }

    fn raw(&self) -> &CFRunLoopObserver {
        &self.observer
    }
}

impl Drop for MainRunLoopObserver {
    fn drop(&mut self) {
        self.observer.invalidate();
    }
}

fn stop_app_immediately(app: &NSApplication) {
    autoreleasepool(|_| {
        app.stop(None);
        if let Some(event) = dummy_event() {
            app.postEvent_atStart(&event, true);
        }
    });
}

fn dummy_event() -> Option<objc2::rc::Retained<NSEvent>> {
    NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags(0),
        0.0,
        0,
        None,
        NSEventSubtype::WindowExposed.0,
        0,
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::{HostState, PumpMode};
    use objc2::MainThreadMarker;

    #[test]
    fn native_wait_request_is_one_shot() {
        let mtm = MainThreadMarker::new().expect("tests run on main thread");
        let state = HostState::new(mtm);
        assert!(!state.take_native_wait_request());
        state.request_native_wait_once();
        assert!(state.take_native_wait_request());
        assert!(!state.take_native_wait_request());
    }

    #[test]
    fn zero_timeout_stops_before_waiting_immediately() {
        let mtm = MainThreadMarker::new().expect("tests run on main thread");
        let state = HostState::new(mtm);
        state.begin_zero_timeout_pump();
        assert_eq!(state.pump_mode.get(), PumpMode::ZeroTimeout);
        assert!(state.should_stop_before_waiting());
    }

    #[test]
    fn native_wait_stops_only_after_after_waiting() {
        let mtm = MainThreadMarker::new().expect("tests run on main thread");
        let state = HostState::new(mtm);
        state.begin_native_wait_pump();
        assert_eq!(state.pump_mode.get(), PumpMode::NativeWaitOnce);
        assert!(!state.should_stop_before_waiting());
        state.notify_native_frame_source();
        assert!(state.should_stop_before_waiting());
    }
}
