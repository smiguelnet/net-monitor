//! Low-level network probes. They only measure; classifying the results
//! into a `Status` is done in `checks`.

pub mod dns;
pub mod ping;
pub mod tcp;

use std::time::Instant;

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
