use std::sync::Once;

/// Writes panic details to the local application log.
pub(crate) fn init() {
    static PANIC_HOOK_INSTALLED: Once = Once::new();
    PANIC_HOOK_INSTALLED.call_once(|| {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let backtrace = std::backtrace::Backtrace::force_capture();
            log::error!("{panic_info}\nbacktrace:\n{backtrace}");
            original_hook(panic_info);
        }));
    });
}

pub fn crash() {
    panic!("Local panic logging smoke test");
}
