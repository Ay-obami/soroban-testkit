//! Assert on contract events without hand-decoding `Val` topics.

mod assertions;
mod capture;

pub use capture::{CapturedEvent, EventLog};
