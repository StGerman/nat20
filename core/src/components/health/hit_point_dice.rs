use std::collections::BTreeMap;

use crate::components::dice::DieSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitPointDiceError {
    /// The creature has no Hit Point Dice of this size at all
    NoSuchDie(DieSize),
    /// All Hit Point Dice of this size have been spent
    NoneRemaining(DieSize),
}

/// The Hit Point Dice of a creature, one pool per die size. A character has one
/// die per class level, of that class's Hit Die. They're spent during a Short
/// Rest to regain Hit Points and are all regained on a Long Rest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitPointDice {
    /// Die size -> (remaining, max)
    pools: BTreeMap<DieSize, (u8, u8)>,
}

impl HitPointDice {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max(&self, die: DieSize) -> u8 {
        self.pools.get(&die).map_or(0, |(_, max)| *max)
    }

    pub fn remaining(&self, die: DieSize) -> u8 {
        self.pools.get(&die).map_or(0, |(remaining, _)| *remaining)
    }

    /// All pools as (die size, remaining, max), smallest die first
    pub fn pools(&self) -> impl Iterator<Item = (DieSize, u8, u8)> + '_ {
        self.pools
            .iter()
            .map(|(die, (remaining, max))| (*die, *remaining, *max))
    }

    /// Sets the number of dice of this size, e.g. after a level up. Dice gained
    /// this way are unspent; dice lost are taken from the unspent ones first.
    pub fn set_max(&mut self, die: DieSize, max: u8) {
        if max == 0 {
            self.pools.remove(&die);
            return;
        }
        let (remaining, old_max) = self.pools.entry(die).or_insert((0, 0));
        let spent = old_max.saturating_sub(*remaining);
        *old_max = max;
        *remaining = max.saturating_sub(spent);
    }

    pub fn spend(&mut self, die: DieSize) -> Result<(), HitPointDiceError> {
        let (remaining, _) = self
            .pools
            .get_mut(&die)
            .ok_or(HitPointDiceError::NoSuchDie(die))?;
        if *remaining == 0 {
            return Err(HitPointDiceError::NoneRemaining(die));
        }
        *remaining -= 1;
        Ok(())
    }

    pub fn restore_all(&mut self) {
        for (remaining, max) in self.pools.values_mut() {
            *remaining = *max;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_has_no_dice() {
        let dice = HitPointDice::new();
        assert_eq!(dice.max(DieSize::D10), 0);
        assert_eq!(dice.remaining(DieSize::D10), 0);
    }

    #[test]
    fn set_max_adds_unspent_dice() {
        let mut dice = HitPointDice::new();
        dice.set_max(DieSize::D10, 2);
        assert_eq!(dice.remaining(DieSize::D10), 2);
        assert_eq!(dice.max(DieSize::D10), 2);
    }

    #[test]
    fn spend_until_empty() {
        let mut dice = HitPointDice::new();
        dice.set_max(DieSize::D8, 2);
        assert_eq!(dice.spend(DieSize::D8), Ok(()));
        assert_eq!(dice.spend(DieSize::D8), Ok(()));
        assert_eq!(
            dice.spend(DieSize::D8),
            Err(HitPointDiceError::NoneRemaining(DieSize::D8))
        );
        assert_eq!(dice.remaining(DieSize::D8), 0);
    }

    #[test]
    fn spend_unknown_die_fails() {
        let mut dice = HitPointDice::new();
        dice.set_max(DieSize::D8, 1);
        assert_eq!(
            dice.spend(DieSize::D12),
            Err(HitPointDiceError::NoSuchDie(DieSize::D12))
        );
    }

    #[test]
    fn level_up_keeps_spent_dice_spent() {
        let mut dice = HitPointDice::new();
        dice.set_max(DieSize::D10, 2);
        dice.spend(DieSize::D10).unwrap();
        dice.set_max(DieSize::D10, 3);
        assert_eq!(dice.remaining(DieSize::D10), 2);
        assert_eq!(dice.max(DieSize::D10), 3);
    }

    #[test]
    fn restore_all_refills_every_pool() {
        let mut dice = HitPointDice::new();
        dice.set_max(DieSize::D10, 2);
        dice.set_max(DieSize::D6, 1);
        dice.spend(DieSize::D10).unwrap();
        dice.spend(DieSize::D6).unwrap();
        dice.restore_all();
        assert_eq!(dice.remaining(DieSize::D10), 2);
        assert_eq!(dice.remaining(DieSize::D6), 1);
    }
}
