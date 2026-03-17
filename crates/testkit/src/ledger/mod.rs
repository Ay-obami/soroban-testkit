//! Ledger time control: advancing, warping, and evaluating contract state
//! at specific points in time, without reasoning about the
//! `timestamp`/`sequence` relationship by hand.

mod clock;

pub use clock::LEDGER_CLOSE_TIME_SECS;
