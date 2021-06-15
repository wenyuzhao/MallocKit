use std::panic::PanicInfo;

use crate::Plan;

fn panic_handler(panic_info: &PanicInfo<'_>) {
    println!("{}", panic_info);
    std::intrinsics::abort();
}

pub fn set_panic_handler() {
    std::panic::set_hook(unsafe { Box::from_raw(&mut panic_handler) });
}

pub extern "C" fn process_start(plan: &impl Plan) {
    set_panic_handler();
    #[cfg(target_os = "macos")]
    crate::util::macos_malloc_zone::init();
    plan.init();
}

pub extern "C" fn process_exit() {
    crate::stat::report();
}
