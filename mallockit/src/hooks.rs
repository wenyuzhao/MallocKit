use std::panic::PanicInfo;

use crate::Plan;

fn panic_handler(panic_info: &PanicInfo<'_>) {
    println!("{}", panic_info);
    std::intrinsics::abort();
}

pub fn set_panic_handler() {
    std::panic::set_hook(unsafe {
        Box::from_raw(&mut panic_handler)
    });
}


pub extern fn process_start(plan: &impl Plan) {
    set_panic_handler();
    plan.init();
}

pub extern fn process_exit() {
    crate::stat::report();
}
