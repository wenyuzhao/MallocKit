#[macro_use]
mod runner;

include!(concat!(env!("OUT_DIR"), "/generated_tests.rs"));
