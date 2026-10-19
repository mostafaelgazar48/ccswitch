//! Claude API list prices, for estimating what token usage would cost. Subscription plans are not
//! billed per token, so these are only an estimate. Update this table when prices change.

/// USD per million tokens. Cache writes cost 1.25× input for the 5-minute cache and 2× for the
/// 1-hour cache; cache reads have their own rate per model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

/// Matched by model-name prefix, first match wins, so newer and longer names come first.
/// Prices as of 2026-10-06.
const PRICES: &[(&str, Price)] = &[
    ("claude-fable-5-1", price(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", price(10.0, 50.0, 0.25)),
    ("claude-fable-5", price(10.0, 50.0, 1.0)),
    ("claude-mythos-5", price(10.0, 50.0, 1.0)),
    ("claude-opus-5-5", price(4.0, 20.0, 0.20)),
    ("claude-opus-5", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-8", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-7", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-6", price(5.0, 25.0, 0.50)),
    ("claude-opus-4-5", price(5.0, 25.0, 0.50)),
    ("claude-opus-4", price(15.0, 75.0, 1.50)),
    ("claude-sonnet-5", price(2.0, 10.0, 0.20)),
    ("claude-sonnet-4", price(3.0, 15.0, 0.30)),
    ("claude-haiku-5-5", price(0.10, 0.50, 0.01)),
    ("claude-haiku-4-5", price(1.0, 5.0, 0.10)),
];

const fn price(input: f64, output: f64, cache_read: f64) -> Price {
    Price {
        input,
        output,
        cache_read,
    }
}

pub fn lookup(model: &str) -> Option<Price> {
    PRICES
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, p)| *p)
}

impl Price {
    /// Fast mode bills at twice the standard rate.
    pub fn cost(
        &self,
        input: u64,
        output: u64,
        write_5m: u64,
        write_1h: u64,
        read: u64,
        fast: bool,
    ) -> f64 {
        let per_token = |n: u64, rate: f64| n as f64 * rate / 1_000_000.0;
        let cost = per_token(input, self.input)
            + per_token(output, self.output)
            + per_token(write_5m, self.input * 1.25)
            + per_token(write_1h, self.input * 2.0)
            + per_token(read, self.cache_read);
        if fast {
            cost * 2.0
        } else {
            cost
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_prefix_wins() {
        assert_eq!(lookup("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(lookup("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(lookup("claude-fable-5-1").unwrap().cache_read, 0.25);
        assert_eq!(lookup("claude-sonnet-4-5-20250929").unwrap().output, 15.0);
        assert_eq!(lookup("claude-opus-4-1-20250805").unwrap().input, 15.0);
        assert!(lookup("<synthetic>").is_none());
        assert!(lookup("gpt-5").is_none());
    }

    #[test]
    fn costs_each_token_kind() {
        let p = lookup("claude-sonnet-5-5").unwrap(); // $2 in, $10 out, $0.20 read
        let m = 1_000_000;
        assert!((p.cost(m, m, m, m, m, false) - (2.0 + 10.0 + 2.5 + 4.0 + 0.2)).abs() < 1e-9);
        assert!((p.cost(m, 0, 0, 0, 0, true) - 4.0).abs() < 1e-9);
    }
}
