use std::time::Duration;

/// Deterministic exponential retry policy with a hard upper bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackoffPolicy {
    initial: Duration,
    maximum: Duration,
    multiplier: u32,
}

impl BackoffPolicy {
    /// Creates a retry policy. Zero multipliers are treated as one.
    #[must_use]
    pub const fn new(initial: Duration, maximum: Duration, multiplier: u32) -> Self {
        Self {
            initial,
            maximum,
            multiplier,
        }
    }

    /// Returns the delay for a zero-based failure attempt, saturated at the configured maximum.
    #[must_use]
    pub fn delay(self, attempt: u32) -> Duration {
        let multiplier = u128::from(self.multiplier.max(1));
        let maximum = self.maximum.as_millis();
        let mut milliseconds = self.initial.as_millis().min(maximum);
        for _ in 0..attempt {
            milliseconds = milliseconds.saturating_mul(multiplier);
            if milliseconds >= maximum {
                return self.maximum;
            }
        }
        Duration::from_millis(u64::try_from(milliseconds).unwrap_or(u64::MAX))
    }
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self::new(Duration::from_millis(250), Duration::from_secs(30), 2)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::BackoffPolicy;

    #[test]
    fn exponential_delays_are_bounded_and_saturating() {
        let policy = BackoffPolicy::new(Duration::from_millis(100), Duration::from_secs(1), 2);
        let delays = (0..6)
            .map(|attempt| policy.delay(attempt))
            .collect::<Vec<_>>();
        assert_eq!(
            delays,
            [
                Duration::from_millis(100),
                Duration::from_millis(200),
                Duration::from_millis(400),
                Duration::from_millis(800),
                Duration::from_secs(1),
                Duration::from_secs(1),
            ]
        );
        assert_eq!(policy.delay(u32::MAX), Duration::from_secs(1));
    }
}
