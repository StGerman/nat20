use std::{
    collections::HashMap,
    fmt::{self},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use crate::registry::serialize::schema::impl_string_schema;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Currency {
    Copper,
    Silver,
    Electrum,
    Gold,
    Platinum,
}

impl Currency {
    /// Every currency, from the smallest coin to the largest
    pub const ASCENDING: [Currency; 5] = [
        Currency::Copper,
        Currency::Silver,
        Currency::Electrum,
        Currency::Gold,
        Currency::Platinum,
    ];

    /// What one coin is worth in copper pieces, by the SRD 5.2.1 exchange rates
    /// (Equipment, Coins): 1 SP = 10 CP, 1 EP = 50 CP, 1 GP = 100 CP, 1 PP = 1,000 CP
    pub fn in_copper(&self) -> u64 {
        match self {
            Currency::Copper => 1,
            Currency::Silver => 10,
            Currency::Electrum => 50,
            Currency::Gold => 100,
            Currency::Platinum => 1000,
        }
    }

    pub fn to_gold(&self, amount: u32) -> f32 {
        let amount = amount as f32;
        match self {
            Currency::Copper => amount / 100.0,
            Currency::Silver => amount / 10.0,
            Currency::Electrum => amount / 2.0,
            Currency::Gold => amount,
            Currency::Platinum => amount * 10.0,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let acronym = match self {
            Currency::Copper => "CP",
            Currency::Silver => "SP",
            Currency::Electrum => "EP",
            Currency::Gold => "GP",
            Currency::Platinum => "PP",
        };
        write!(f, "{}", acronym)
    }
}

impl FromStr for Currency {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "CP" => Ok(Currency::Copper),
            "SP" => Ok(Currency::Silver),
            "EP" => Ok(Currency::Electrum),
            "GP" => Ok(Currency::Gold),
            "PP" => Ok(Currency::Platinum),
            _ => Err(format!("Invalid currency format: {}", s)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MonetaryValue {
    pub values: HashMap<Currency, u32>,
}

#[derive(Debug, Clone)]
pub enum MonetaryValueError {
    InsufficientFunds,
}

impl_string_schema!(
    MonetaryValue,
    "MonetaryValue",
    "description": "A monetary value: comma-separated `<amount> <currency>` parts, where the \
         currency is `CP`, `SP`, `EP`, `GP` or `PP`.",
    "examples": ["10 GP", "1 PP, 5 SP"]
);

impl Default for MonetaryValue {
    fn default() -> Self {
        Self::new()
    }
}

impl MonetaryValue {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn add(&mut self, currency: Currency, amount: u32) {
        *self.values.entry(currency).or_insert(0) += amount;
    }

    pub fn remove(&mut self, currency: Currency, amount: u32) -> Result<(), MonetaryValueError> {
        if let Some(current_amount) = self.values.get_mut(&currency)
            && *current_amount >= amount
        {
            *current_amount -= amount;
            if *current_amount == 0 {
                self.values.remove(&currency);
            }
            return Ok(());
        }
        Err(MonetaryValueError::InsufficientFunds)
    }

    /// The whole value in copper pieces, exact
    pub fn total_in_copper(&self) -> u64 {
        self.values
            .iter()
            .map(|(currency, &amount)| currency.in_copper() * u64::from(amount))
            .sum()
    }

    /// Pays `amount` out of this value, making change the way a shopkeeper would.
    ///
    /// Only the total of `amount` counts, not its coins: 5 SP can be paid with a
    /// gold piece. Coins go from the largest down, as many of each as still
    /// fit in what is owed; if something is still owed, the smallest coin left
    /// (each one is worth more than the rest owed) is handed over and the change comes back in gold, silver and
    /// copper (never electrum), largest first. Atomic: on
    /// [`MonetaryValueError::InsufficientFunds`] nothing is spent.
    pub fn spend(&mut self, amount: &MonetaryValue) -> Result<(), MonetaryValueError> {
        let mut owed = amount.total_in_copper();
        if owed > self.total_in_copper() {
            return Err(MonetaryValueError::InsufficientFunds);
        }
        let mut purse = self.clone();
        for currency in Currency::ASCENDING.into_iter().rev() {
            let value = currency.in_copper();
            let held = u64::from(purse.values.get(&currency).copied().unwrap_or(0));
            let taken = held.min(owed / value);
            if taken > 0 {
                purse.remove(currency.clone(), taken as u32)?;
                owed -= taken * value;
            }
        }
        if owed > 0 {
            // Every coin left is worth more than what is owed, and the total
            // check above means at least one is left
            let coin = Currency::ASCENDING
                .into_iter()
                .find(|currency| purse.values.get(currency).is_some_and(|&held| held > 0))
                .ok_or(MonetaryValueError::InsufficientFunds)?;
            purse.remove(coin.clone(), 1)?;
            let mut change = coin.in_copper() - owed;
            for currency in [Currency::Gold, Currency::Silver, Currency::Copper] {
                let value = currency.in_copper();
                let coins = change / value;
                if coins > 0 {
                    purse.add(currency, coins as u32);
                    change -= coins * value;
                }
            }
        }
        *self = purse;
        Ok(())
    }

    pub fn total_in_gold(&self) -> f32 {
        self.values
            .iter()
            .map(|(currency, &amount)| currency.to_gold(amount))
            .sum()
    }
}

impl fmt::Display for MonetaryValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        let sorted_keys: Vec<_> = {
            let mut keys: Vec<_> = self.values.keys().collect();
            keys.sort_by_key(|k| *k);
            keys.reverse();
            keys
        };
        for currency in sorted_keys {
            if let Some(&amount) = self.values.get(currency)
                && amount > 0
            {
                parts.push(format!("{} {}", amount, currency));
            }
        }
        if parts.is_empty() {
            parts.push("0 GP".to_string());
        }
        write!(f, "{}", parts.join(", "))
    }
}

impl FromStr for MonetaryValue {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut values = HashMap::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let mut parts = part.split_whitespace();
            let amount = parts
                .next()
                .ok_or_else(|| format!("Invalid monetary value format: {}", s))?
                .parse::<u32>()
                .map_err(|_| format!("Invalid amount in monetary value: {}", s))?;
            let currency_str = parts.next().unwrap_or("GP");
            let currency = Currency::from_str(currency_str)?;
            values.insert(currency, amount);
        }
        Ok(Self { values })
    }
}

impl TryFrom<String> for MonetaryValue {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<MonetaryValue> for String {
    fn from(spec: MonetaryValue) -> Self {
        spec.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_acronym() {
        assert_eq!(Currency::Copper.to_string(), "CP");
        assert_eq!(Currency::Silver.to_string(), "SP");
        assert_eq!(Currency::Electrum.to_string(), "EP");
        assert_eq!(Currency::Gold.to_string(), "GP");
        assert_eq!(Currency::Platinum.to_string(), "PP");
    }

    #[test]
    fn currency_to_gold() {
        assert!((Currency::Copper.to_gold(100) - 1.0).abs() < f32::EPSILON);
        assert!((Currency::Silver.to_gold(10) - 1.0).abs() < f32::EPSILON);
        assert!((Currency::Electrum.to_gold(2) - 1.0).abs() < f32::EPSILON);
        assert!((Currency::Gold.to_gold(1) - 1.0).abs() < f32::EPSILON);
        assert!((Currency::Platinum.to_gold(1) - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn currency_from_str() {
        assert_eq!(Currency::from_str("GP").unwrap(), Currency::Gold);
        assert_eq!(Currency::from_str("gp").unwrap(), Currency::Gold);
        assert_eq!(Currency::from_str("SP").unwrap(), Currency::Silver);
        assert_eq!(Currency::from_str("pp").unwrap(), Currency::Platinum);
    }

    #[test]
    fn currency_from_invalid_str() {
        assert!(Currency::from_str("ZZ").is_err());
    }

    #[test]
    fn value_from_str_single() {
        let value = MonetaryValue::from_str("10 GP").unwrap();
        assert_eq!(value.values.get(&Currency::Gold), Some(&10));
    }

    #[test]
    fn value_from_str_multiple() {
        let value = MonetaryValue::from_str("5 GP, 20 SP, 100 CP").unwrap();
        assert_eq!(value.values.get(&Currency::Gold), Some(&5));
        assert_eq!(value.values.get(&Currency::Silver), Some(&20));
        assert_eq!(value.values.get(&Currency::Copper), Some(&100));
    }

    #[test]
    fn value_from_str_with_whitespace() {
        let value = MonetaryValue::from_str("  3 GP ,  7 SP ").unwrap();
        assert_eq!(value.values.get(&Currency::Gold), Some(&3));
        assert_eq!(value.values.get(&Currency::Silver), Some(&7));
    }

    #[test]
    fn value_from_str_default_currency() {
        let value = MonetaryValue::from_str("42").unwrap();
        assert_eq!(value.values.get(&Currency::Gold), Some(&42));
    }

    #[test]
    fn value_from_invalid_str() {
        assert!(MonetaryValue::from_str("ten GP").is_err());
        assert!(MonetaryValue::from_str("5 ZZ").is_err());
    }

    #[test]
    fn value_zero() {
        let value = MonetaryValue::from_str("0 GP").unwrap();
        assert_eq!(value.values.get(&Currency::Gold), Some(&0));
    }

    #[test]
    fn add_remove_money() {
        let mut value = MonetaryValue::new();
        value.add(Currency::Gold, 10);
        value.add(Currency::Silver, 50);
        assert_eq!(value.values.get(&Currency::Gold), Some(&10));
        assert_eq!(value.values.get(&Currency::Silver), Some(&50));

        value.remove(Currency::Silver, 20).unwrap();
        assert_eq!(value.values.get(&Currency::Silver), Some(&30));

        let result = value.remove(Currency::Gold, 15);
        assert!(matches!(result, Err(MonetaryValueError::InsufficientFunds)));
    }

    fn value(s: &str) -> MonetaryValue {
        MonetaryValue::from_str(s).unwrap()
    }

    #[test]
    fn total_in_copper_uses_the_srd_exchange_rates() {
        assert_eq!(
            value("1 PP, 1 GP, 1 EP, 1 SP, 1 CP").total_in_copper(),
            1161
        );
        assert_eq!(MonetaryValue::new().total_in_copper(), 0);
    }

    #[test]
    fn spend_makes_change_from_a_larger_coin() {
        let mut purse = value("1 GP");
        purse.spend(&value("5 SP")).unwrap();
        assert_eq!(purse, value("5 SP"));
    }

    #[test]
    fn spend_pays_with_the_largest_coins_that_fit() {
        let mut purse = value("2 GP, 30 SP, 7 CP");
        purse.spend(&value("3 GP")).unwrap();
        assert_eq!(purse, value("20 SP, 7 CP"));
    }

    #[test]
    fn spend_breaks_the_smallest_coin_that_covers_the_rest() {
        let mut purse = value("1 PP, 1 GP, 3 CP");
        purse.spend(&value("25 CP")).unwrap();
        // 3 CP paid, 22 CP still owed: the gold piece is broken, not the platinum
        assert_eq!(purse, value("1 PP, 7 SP, 8 CP"));
        assert_eq!(purse.total_in_copper(), 1078);
    }

    #[test]
    fn spend_gives_change_without_electrum() {
        let mut purse = value("1 PP");
        purse.spend(&value("1 EP")).unwrap();
        assert_eq!(purse, value("9 GP, 5 SP"));
    }

    #[test]
    fn spend_is_atomic_on_insufficient_funds() {
        let mut purse = value("1 GP, 5 SP");
        let before = purse.clone();
        assert!(matches!(
            purse.spend(&value("2 GP")),
            Err(MonetaryValueError::InsufficientFunds)
        ));
        assert_eq!(purse, before);
    }

    #[test]
    fn spend_the_exact_total_empties_the_purse() {
        let mut purse = value("1 GP, 5 SP");
        purse.spend(&value("15 SP")).unwrap();
        assert_eq!(purse.total_in_copper(), 0);
    }
}
