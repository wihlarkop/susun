#![no_main]

use libfuzzer_sys::fuzz_target;
use susun_planner::ExecutionPlan;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<ExecutionPlan>(data);
});
