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

        self.state.begin_zero_timeout_pump();
        autoreleasepool(|_| {
            self.app.run();
        });
        self.state.end_pump();
        Ok(PumpResult {
            pumped_native: true,
        })
    }

    pub(crate) fn request_wake(&self) {
        self.main_run_loop.wake_up();
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

#[derive(Debug)]
struct HostState {
    mtm: MainThreadMarker,
    pumping: Cell<bool>,
    stop_before_wait: Cell<bool>,
}

impl HostState {
    fn new(mtm: MainThreadMarker) -> Self {
        Self {
            mtm,
            pumping: Cell::new(false),
            stop_before_wait: Cell::new(false),
        }
    }

    fn begin_zero_timeout_pump(&self) {
        self.pumping.set(true);
        self.stop_before_wait.set(true);
    }

    fn end_pump(&self) {
        self.stop_before_wait.set(false);
        self.pumping.set(false);
    }

    fn before_waiting(&self) {
        if !self.pumping.get() || !self.stop_before_wait.get() {
            return;
        }

        stop_app_immediately(&NSApplication::sharedApplication(self.mtm));
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
