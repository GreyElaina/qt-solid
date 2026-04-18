use std::sync::atomic::{AtomicBool, Ordering};

use crate::host::PumpResult;

#[derive(Debug, Default)]
pub(crate) struct WakeFlagHost {
    wake_pending: AtomicBool,
}

impl WakeFlagHost {
    pub(crate) fn pump_zero_timeout(&self) -> PumpResult {
        let had_pending_wake = self.wake_pending.swap(false, Ordering::AcqRel);
        PumpResult {
            pumped_native: had_pending_wake,
        }
    }

    pub(crate) fn request_wake(&self) {
        self.wake_pending.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::WakeFlagHost;

    #[test]
    fn coalesces_pending_wake_until_pump_drains_it() {
        let host = WakeFlagHost::default();

        assert!(!host.pump_zero_timeout().pumped_native);

        host.request_wake();
        host.request_wake();
        assert!(host.pump_zero_timeout().pumped_native);
        assert!(!host.pump_zero_timeout().pumped_native);
    }
}
