//! The random number generator behind every dice roll and every random AI
//! choice. Each thread has its own generator, seeded from the operating system
//! by default, so behaviour is unchanged unless something calls [`seed`].
//!
//! Seeding makes a run repeatable: the same seed followed by the same sequence
//! of actions gives the same rolls, which is useful for tests and for
//! reproducing bugs. The sequence is only stable for a given version of `rand`,
//! since `StdRng` does not promise the same output across `rand` releases.

use std::{cell::RefCell, convert::Infallible};

use rand::{SeedableRng, TryRng, rngs::StdRng};

thread_local! {
    static RNG: RefCell<StdRng> = RefCell::new(rand::make_rng());
}

/// Re-seeds the current thread's generator. Every roll made on this thread
/// after the call follows from `seed`.
pub fn seed(seed: u64) {
    RNG.with_borrow_mut(|rng| *rng = StdRng::seed_from_u64(seed));
}

/// Handle to the current thread's generator. Use it wherever `rand::rng()`
/// would otherwise be used, so the roll follows [`seed`].
pub fn rng() -> Nat20Rng {
    Nat20Rng
}

/// See [`rng`]. Holds no state of its own, so it is cheap to create and each
/// call borrows the thread's generator only for the duration of that call.
#[derive(Debug, Clone, Copy, Default)]
pub struct Nat20Rng;

// rand 0.10: implementing the infallible `TryRng` gives `Rng` and `RngExt`
// (`random_range` and friends) through rand's blanket impls.
impl TryRng for Nat20Rng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        RNG.with_borrow_mut(|rng| rng.try_next_u32())
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        RNG.with_borrow_mut(|rng| rng.try_next_u64())
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Infallible> {
        RNG.with_borrow_mut(|rng| rng.try_fill_bytes(dest))
    }
}
