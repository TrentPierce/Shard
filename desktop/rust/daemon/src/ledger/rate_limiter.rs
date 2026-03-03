use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub type WalletId = String;

#[derive(Debug, Clone)]
pub struct RateLimitExceeded {
    pub wallet: WalletId,
    pub attempted_amount: f64,
    pub hourly_total: f64,
    pub max_hourly_shards: f64,
}

#[derive(Debug, Clone)]
struct WalletWindow {
    events: VecDeque<(Instant, f64)>,
    acceptance: VecDeque<bool>,
}

impl WalletWindow {
    fn new() -> Self {
        Self {
            events: VecDeque::new(),
            acceptance: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
pub struct WalletRateLimiter {
    max_hourly_shards: f64,
    min_acceptance_rate: f64,
    wallets: HashMap<WalletId, WalletWindow>,
}

impl WalletRateLimiter {
    pub fn new(max_hourly_shards: f64, min_acceptance_rate: f64) -> Self {
        Self {
            max_hourly_shards,
            min_acceptance_rate,
            wallets: HashMap::new(),
        }
    }

    pub fn check_and_record(
        &mut self,
        wallet: WalletId,
        amount: f64,
    ) -> Result<(), RateLimitExceeded> {
        self.check_and_record_at(wallet, amount, Instant::now())
    }

    pub fn check_and_record_at(
        &mut self,
        wallet: WalletId,
        amount: f64,
        now: Instant,
    ) -> Result<(), RateLimitExceeded> {
        let window = self
            .wallets
            .entry(wallet.clone())
            .or_insert_with(WalletWindow::new);

        Self::prune_old_events(&mut window.events, now);

        let hourly_total: f64 = window.events.iter().map(|(_, value)| *value).sum();
        if hourly_total + amount > self.max_hourly_shards {
            return Err(RateLimitExceeded {
                wallet,
                attempted_amount: amount,
                hourly_total,
                max_hourly_shards: self.max_hourly_shards,
            });
        }

        window.events.push_back((now, amount));
        Ok(())
    }

    pub fn record_submission_result(&mut self, wallet: WalletId, accepted: bool) {
        let window = self.wallets.entry(wallet).or_insert_with(WalletWindow::new);
        if window.acceptance.len() == 100 {
            let _ = window.acceptance.pop_front();
        }
        window.acceptance.push_back(accepted);
    }

    pub fn acceptance_rate(&self, wallet: WalletId) -> f64 {
        let Some(window) = self.wallets.get(&wallet) else {
            return 0.0;
        };
        if window.acceptance.is_empty() {
            return 0.0;
        }
        let accepted = window.acceptance.iter().filter(|entry| **entry).count();
        accepted as f64 / window.acceptance.len() as f64
    }

    pub fn is_quality_eligible(&self, wallet: WalletId) -> bool {
        self.acceptance_rate(wallet) >= self.min_acceptance_rate
    }

    fn prune_old_events(events: &mut VecDeque<(Instant, f64)>, now: Instant) {
        while let Some((ts, _)) = events.front() {
            if now.duration_since(*ts) > Duration::from_secs(3600) {
                let _ = events.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WalletRateLimiter, WalletId};
    use std::time::{Duration, Instant};

    #[test]
    fn ledger_rate_limit_window_slides_correctly() {
        let wallet: WalletId = "wallet-a".to_string();
        let mut limiter = WalletRateLimiter::new(10.0, 0.4);
        let start = Instant::now();

        limiter
            .check_and_record_at(wallet.clone(), 7.0, start)
            .expect("first event should fit");
        assert!(limiter.check_and_record_at(wallet.clone(), 4.0, start).is_err());

        let later = start + Duration::from_secs(3601);
        limiter
            .check_and_record_at(wallet.clone(), 4.0, later)
            .expect("expired event should be pruned");
    }

    #[test]
    fn ledger_acceptance_threshold_logic() {
        let wallet: WalletId = "wallet-b".to_string();
        let mut limiter = WalletRateLimiter::new(100.0, 0.4);

        for _ in 0..40 {
            limiter.record_submission_result(wallet.clone(), true);
        }
        for _ in 0..60 {
            limiter.record_submission_result(wallet.clone(), false);
        }

        assert_eq!(limiter.acceptance_rate(wallet.clone()), 0.4);
        assert!(limiter.is_quality_eligible(wallet));
    }
}
