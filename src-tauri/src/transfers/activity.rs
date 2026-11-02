//! Keeps a running transfer at full speed while the window is hidden.
//!
//! macOS App Nap throttles an app whose windows are not visible, network
//! included: a download behind another window dropped from ~60 MB/s to
//! ~12 MB/s. A user-initiated activity opts out of App Nap for as long as it
//! is held. Idle system sleep stays allowed; an interrupted download resumes.
//! Windows and Linux don't throttle background apps' I/O, so this is a no-op
//! there.

/// Held for as long as a transfer runs; dropping it ends the activity.
pub(super) struct TransferActivity {
    #[cfg(target_os = "macos")]
    token:
        objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2::runtime::NSObjectProtocol>>,
}

// SAFETY: NSProcessInfo is thread-safe, and the token is only passed back to
// `endActivity:`, which may be called from any thread.
#[cfg(target_os = "macos")]
unsafe impl Send for TransferActivity {}

impl TransferActivity {
    #[cfg(target_os = "macos")]
    pub(super) fn begin() -> Self {
        use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

        let token = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
            NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
            &NSString::from_str("Transferring files"),
        );
        Self { token }
    }

    #[cfg(not(target_os = "macos"))]
    pub(super) fn begin() -> Self {
        Self {}
    }
}

#[cfg(target_os = "macos")]
impl Drop for TransferActivity {
    fn drop(&mut self) {
        // SAFETY: the token came from `beginActivityWithOptions:reason:`.
        unsafe { objc2_foundation::NSProcessInfo::processInfo().endActivity(&self.token) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begins_and_ends_off_the_main_thread() {
        std::thread::spawn(|| {
            let first = TransferActivity::begin();
            let second = TransferActivity::begin();
            drop(first);
            drop(second);
        })
        .join()
        .unwrap();
    }
}
