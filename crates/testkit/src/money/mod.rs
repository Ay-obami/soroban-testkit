//! Adversarial value generation and conservation checking for money math —
//! the arithmetic bugs hand-written tests tend to miss.

mod generators;
mod invariants;

pub use generators::{adversarial_amounts, amounts_in, bps_values, overflow_edge_triples};
pub use invariants::{assert_conserved_over, Conservation};
