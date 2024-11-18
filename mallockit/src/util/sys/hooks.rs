use std::panic::PanicHookInfo;

use crate::Plan;

fn panic_handler(panic_info: &PanicHookInfo<'_>) {
    crate::println!("{}", panic_info);
    std::process::abort();
}

pub fn set_panic_handler() {
    std::panic::set_hook(unsafe { Box::from_raw(&mut panic_handler) });
}

pub extern "C" fn process_start(plan: &'static impl Plan) {
    set_panic_handler();
    crate::mutator::init_pthread_key();
    unsafe {
        libc::atexit(process_exit);
    }
    #[cfg(target_os = "macos")]
    crate::util::malloc::macos_malloc_zone::init();
    plan.init();
}

extern "C" fn process_exit() {
    crate::stat::report();
}
