use crate::Plan;



pub extern fn process_start(plan: &impl Plan) {
    plan.init();
}

pub extern fn process_exit() {}
