//! SRD 5.2.1, D20 Tests > Advantage/Disadvantage: a roll with both has
//! neither, "even if multiple circumstances impose Disadvantage and only one
//! grants Advantage or vice versa".

extern crate nat20_core;

use nat20_core::components::{
    d20::{AdvantageTracker, AdvantageType, RollMode},
    modifier::ModifierSource,
};

fn tracker(sources: &[(&str, AdvantageType)]) -> AdvantageTracker {
    let mut tracker = AdvantageTracker::new();
    for (name, kind) in sources {
        tracker.add(*kind, ModifierSource::Custom(name.to_string()));
    }
    tracker
}

#[test]
fn one_side_only_keeps_its_mode() {
    use AdvantageType::*;
    assert_eq!(tracker(&[]).roll_mode(), RollMode::Normal);
    assert_eq!(
        tracker(&[("a", Advantage), ("b", Advantage)]).roll_mode(),
        RollMode::Advantage
    );
    assert_eq!(
        tracker(&[("a", Disadvantage), ("b", Disadvantage)]).roll_mode(),
        RollMode::Disadvantage
    );
}

#[test]
fn advantage_and_disadvantage_cancel_whatever_the_count() {
    use AdvantageType::*;
    assert_eq!(
        tracker(&[("a", Advantage), ("b", Disadvantage)]).roll_mode(),
        RollMode::Normal
    );
    // e.g. Pack Tactics and a Prone target within 5 feet, while the attacker is Prone itself
    assert_eq!(
        tracker(&[
            ("pack_tactics", Advantage),
            ("target_prone", Advantage),
            ("attacker_prone", Disadvantage)
        ])
        .roll_mode(),
        RollMode::Normal
    );
    assert_eq!(
        tracker(&[("a", Advantage), ("b", Disadvantage), ("c", Disadvantage)]).roll_mode(),
        RollMode::Normal
    );
}

#[test]
fn forgone_advantage_leaves_disadvantage() {
    use AdvantageType::*;
    let mut forgone = tracker(&[("a", Advantage), ("b", Advantage)]);
    forgone.forgo_advantage(ModifierSource::Custom("brutal_strike".to_string()));
    assert_eq!(forgone.roll_mode(), RollMode::Normal);

    let mut forgone = tracker(&[("a", Advantage), ("b", Disadvantage)]);
    forgone.forgo_advantage(ModifierSource::Custom("brutal_strike".to_string()));
    assert_eq!(forgone.roll_mode(), RollMode::Disadvantage);
}
