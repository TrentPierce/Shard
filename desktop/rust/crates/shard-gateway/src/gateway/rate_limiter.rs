//! Rate limiting module for Shard API Gateway
//!
//! Implements token bucket algorithm for API rate limiting.
//! Supports per-API-key limits and per-IP limits for unauthenticated requests.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute for default tier
    pub default_limit: u64,
    /// Requests per minute for enterprise tier
    pub enterprise_limit: u64,
    /// Burst allowance (additional tokens)
    pub burst: u64,
    /// Per-IP limit for unauthenticated requests
    pub ip_limit: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            default_limit: std::env::var("SHARD_RATE_LIMIT_DEFAULT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            enterprise_limit: std::env::var("SHARD_RATE_LIMIT_ENTERPRISE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(600),
            burst: std::env::var("SHARD_RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            ip_limit: std::env::var("SHARD_RATE_LIMIT_IP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// A token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    /// Current token count
    tokens: AtomicU64,
    /// Maximum tokens (capacity)
    capacity: u64,
    /// Last refill timestamp
    last_refill: std::sync::Mutex<Instant>,
    /// Refill rate per second
    rate_per_second: f64,
}

impl TokenBucket {
    /// Create a new token bucket with given capacity and refill rate
    pub fn new(capacity: u64, rate_per_second: f64) -> Self {
        Self {
            tokens: AtomicU64::new(capacity),
            last_refill: std::sync::Mutex::new(Instant::now()),
            capacity,
            rate_per_second,
        }
    }

    /// Try to consume a token, returns true if successful
    pub fn try_consume(&self) -> bool {
        self.refill();
        let current = self.tokens.load(Ordering::Relaxed);
        if current > 0 {
            self.tokens.store(current - 1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get current token count (for reporting)
    pub fn available(&self) -> u64 {
        self.refill();
        self.tokens.load(Ordering::Relaxed)
    }

    /// Get remaining capacity
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Refill tokens based on elapsed time
    fn refill(&self) {
        let now = Instant::now();
        let mut last = self.last_refill.lock().unwrap();
        let elapsed = now.duration_since(*last).as_secs_f64();

        if elapsed > 0.0 {
            let tokens_to_add = (elapsed * self.rate_per_second).floor() as u64;
            if tokens_to_add > 0 {
                let current = self.tokens.load(Ordering::Relaxed);
                let new_tokens = (current + tokens_to_add).min(self.capacity);
                self.tokens.store(new_tokens, Ordering::Relaxed);
                *last = now;
            }
        }
    }

    /// Get time until next token available (for Retry-After header)
    pub fn time_until_available(&self) -> Duration {
        let current = self.tokens.load(Ordering::Relaxed);
        if current > 0 {
            return Duration::ZERO;
        }

        // Calculate time needed to get 1 token
        let tokens_needed = 1;
        let time_needed = tokens_needed as f64 / self.rate_per_second;

        // Account for time since last refill
        let last = *self.last_refill.lock().unwrap();
        let elapsed = Instant::now().duration_since(last).as_secs_f64();
        let time_until = (time_needed - elapsed).max(0.0);

        Duration::from_secs_f64(time_until)
    }
}

/// Rate limiter state for all keys and IPs
#[derive(Debug)]
pub struct RateLimiter {
    /// Per-API-key buckets
    key_buckets: std::sync::Mutex<HashMap<String, TokenBucket>>,
    /// Per-IP buckets
    ip_buckets: std::sync::Mutex<HashMap<String, TokenBucket>>,
    /// API key tier storage (key -> is_enterprise)
    key_tiers: std::sync::Mutex<HashMap<String, bool>>,
    /// Configuration
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            key_buckets: std::sync::Mutex::new(HashMap::new()),
            ip_buckets: std::sync::Mutex::new(HashMap::new()),
            key_tiers: std::sync::Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Get or create a bucket for an API key
    fn get_key_bucket(&self, api_key: &str) -> TokenBucket {
        let mut buckets = self.key_buckets.lock().unwrap();

        // Check if key exists and get its tier
        let is_enterprise = self
            .key_tiers
            .lock()
            .unwrap()
            .get(api_key)
            .copied()
            .unwrap_or(false);

        let (capacity, rate) = if is_enterprise {
            (
                self.config.enterprise_limit + self.config.burst,
                self.config.enterprise_limit as f64 / 60.0,
            )
        } else {
            (
                self.config.default_limit + self.config.burst,
                self.config.default_limit as f64 / 60.0,
            )
        };

        buckets
            .entry(api_key.to_string())
            .or_insert_with(|| TokenBucket::new(capacity, rate))
            .clone()
    }

    /// Get or create a bucket for an IP address
    fn get_ip_bucket(&self, ip: &str) -> TokenBucket {
        let mut buckets = self.ip_buckets.lock().unwrap();
        let rate = self.config.ip_limit as f64 / 60.0;

        buckets
            .entry(ip.to_string())
            .or_insert_with(|| TokenBucket::new(self.config.ip_limit, rate))
            .clone()
    }

    /// Check if request is allowed for the given API key and IP
    /// Returns Some(Ok) if allowed, Some(Err) if rate limited, None if no limit applies
    pub fn check(&self, api_key: Option<&str>, ip: Option<&str>) -> RateLimitResult {
        // Check API key limit first (higher priority)
        if let Some(key) = api_key {
            let bucket = self.get_key_bucket(key);
            if !bucket.try_consume() {
                return RateLimitResult::RateLimited {
                    limit: bucket.capacity(),
                    remaining: bucket.available(),
                    reset_at: bucket.time_until_available(),
                };
            }
            return RateLimitResult::Allowed {
                limit: bucket.capacity(),
                remaining: bucket.available() - 1,
            };
        }

        // Check IP limit for unauthenticated requests
        if let Some(addr) = ip {
            let bucket = self.get_ip_bucket(addr);
            if !bucket.try_consume() {
                return RateLimitResult::RateLimited {
                    limit: bucket.capacity(),
                    remaining: bucket.available(),
                    reset_at: bucket.time_until_available(),
                };
            }
            return RateLimitResult::Allowed {
                limit: bucket.capacity(),
                remaining: bucket.available() - 1,
            };
        }

        // No rate limiting for completely unauthenticated local requests
        RateLimitResult::Allowed {
            limit: u64::MAX,
            remaining: u64::MAX,
        }
    }

    /// Get current status for an API key
    pub fn status(&self, api_key: &str) -> RateLimitStatus {
        let bucket = self.get_key_bucket(api_key);
        let tier = self
            .key_tiers
            .lock()
            .unwrap()
            .get(api_key)
            .copied()
            .unwrap_or(false);

        RateLimitStatus {
            limit: bucket.capacity(),
            remaining: bucket.available(),
            reset_after_seconds: bucket.time_until_available().as_secs() as u64,
            tier: if tier { "enterprise" } else { "default" }.to_string(),
        }
    }

    /// Register a new API key with a tier
    pub fn register_key(&self, api_key: &str, is_enterprise: bool) {
        self.key_tiers
            .lock()
            .unwrap()
            .insert(api_key.to_string(), is_enterprise);

        // Clear existing bucket to recreate with new limits
        self.key_buckets.lock().unwrap().remove(api_key);
    }

    /// Remove an API key
    pub fn remove_key(&self, api_key: &str) {
        self.key_tiers.lock().unwrap().remove(api_key);
        self.key_buckets.lock().unwrap().remove(api_key);
    }

    /// Get current timestamp for reset header
    pub fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Result of rate limit check
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RateLimitResult {
    Allowed {
        limit: u64,
        remaining: u64,
    },
    RateLimited {
        limit: u64,
        remaining: u64,
        reset_at: Duration,
    },
}

impl RateLimitResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, RateLimitResult::Allowed { .. })
    }

    pub fn limit(&self) -> u64 {
        match self {
            RateLimitResult::Allowed { limit, .. } => *limit,
            RateLimitResult::RateLimited { limit, .. } => *limit,
        }
    }

    pub fn remaining(&self) -> u64 {
        match self {
            RateLimitResult::Allowed { remaining, .. } => *remaining,
            RateLimitResult::RateLimited { remaining, .. } => *remaining,
        }
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            RateLimitResult::RateLimited { reset_at, .. } => Some(reset_at.as_secs().max(1)),
            RateLimitResult::Allowed { .. } => None,
        }
    }

    pub fn reset_timestamp(&self) -> Option<u64> {
        match self {
            RateLimitResult::RateLimited { reset_at, .. } => Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs() + reset_at.as_secs())
                    .unwrap_or(0),
            ),
            RateLimitResult::Allowed { .. } => None,
        }
    }
}

/// Status response for /v1/usage/rate-limit endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    pub limit: u64,
    pub remaining: u64,
    pub reset_after_seconds: u64,
    pub tier: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_basic() {
        let bucket = TokenBucket::new(10, 1.0); // 10 tokens, 1 per second

        // Should be able to consume 10 tokens
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }

        // 11th should fail
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_rate_limiter_key() {
        let limiter = RateLimitConfig::default().to_limiter();

        // First request should succeed
        let result = limiter.check(Some("test-key"), None);
        assert!(result.is_allowed());

        // Set as enterprise and recreate limiter
        limiter.register_key("enterprise-key", true);
        let result2 = limiter.check(Some("enterprise-key"), None);
        assert!(result2.is_allowed());
    }

    #[test]
    fn test_rate_limiter_ip() {
        let limiter = RateLimitConfig::default().to_limiter();

        // First IP request should succeed
        let result = limiter.check(None, Some("192.168.1.1"));
        assert!(result.is_allowed());
    }

    impl RateLimitConfig {
        fn to_limiter(&self) -> RateLimiter {
            RateLimiter::new(self.clone())
        }
    }
}
