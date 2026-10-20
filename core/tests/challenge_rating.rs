extern crate nat20_core;

use std::str::FromStr;

use nat20_core::components::level::{ChallengeRating, Level};

#[test]
fn fractional_challenge_ratings_parse() {
    assert_eq!(
        ChallengeRating::from_str("1/8"),
        Ok(ChallengeRating::EIGHTH)
    );
    assert_eq!(
        ChallengeRating::from_str("1/4"),
        Ok(ChallengeRating::QUARTER)
    );
    assert_eq!(ChallengeRating::from_str("1/2"), Ok(ChallengeRating::HALF));
    assert_eq!(ChallengeRating::from_str("0"), Ok(ChallengeRating::new(0)));
    assert_eq!(
        ChallengeRating::from_str("30"),
        Ok(ChallengeRating::new(30))
    );
}

#[test]
fn invalid_challenge_ratings_are_rejected() {
    for input in ["1/3", "2/4", "31", "-1", "", "quarter"] {
        assert!(
            ChallengeRating::from_str(input).is_err(),
            "{input:?} should not parse"
        );
    }
}

#[test]
fn challenge_rating_round_trips_through_display() {
    for input in ["0", "1/8", "1/4", "1/2", "1", "17"] {
        let cr = ChallengeRating::from_str(input).unwrap();
        assert_eq!(cr.to_string(), input);
    }
}

#[test]
fn fractional_challenge_ratings_give_srd_experience() {
    assert_eq!(ChallengeRating::EIGHTH.experience(), 25);
    assert_eq!(ChallengeRating::QUARTER.experience(), 50);
    assert_eq!(ChallengeRating::HALF.experience(), 100);
    assert_eq!(ChallengeRating::new(1).experience(), 200);
    assert_eq!(ChallengeRating::new(30).experience(), 155000);
}

#[test]
fn proficiency_bonus_follows_the_srd_steps() {
    let cases = [
        ("0", 2),
        ("1/8", 2),
        ("1/4", 2),
        ("1/2", 2),
        ("1", 2),
        ("4", 2),
        ("5", 3),
        ("8", 3),
        ("9", 4),
        ("29", 9),
        ("30", 9),
    ];
    for (input, bonus) in cases {
        let cr = ChallengeRating::from_str(input).unwrap();
        assert_eq!(cr.proficiency_bonus(), bonus, "CR {input}");
    }
}
