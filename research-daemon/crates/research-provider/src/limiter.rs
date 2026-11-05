use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

/// A token-bucket rate limiter combined with a concurrency semaphore.
///
/// The semaphore is acquired first so the provider never has more than
/// `max_concurrency` requests in flight. The bucket then spaces requests at
/// `requests_per_minute`, allowing a burst up to `burst`.
pub struct RateLimiter {
    bucket: Mutex<Bucket>,
    refill_per_second: f64,
    capacity: f64,
    permits: Arc<Semaphore>,
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// Held for the duration of a provider request.
#[derive(Debug)]
pub struct RateLimitPermit {
    _permit: OwnedSemaphorePermit,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32, burst: u32, max_concurrency: usize) -> Self {
        let rate = requests_per_minute.max(1) as f64;
        let capacity = burst.max(1) as f64;

        Self {
            bucket: Mutex::new(Bucket {
                tokens: capacity,
                last_refill: Instant::now(),
            }),
            refill_per_second: rate / 60.0,
            capacity,
            permits: Arc::new(Semaphore::new(max_concurrency.max(1))),
        }
    }

    /// Wait until a request is allowed, then return a permit.
    pub async fn acquire(&self) -> RateLimitPermit {
        // Acquire the concurrency slot first. This bounds waiters as well as
        // in-flight requests and keeps the token bucket simple.
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .expect("rate limiter semaphore closed");

        loop {
            let wait = {
                let mut bucket = self.bucket.lock().await;
                let now = Instant::now();
                let elapsed = now
                    .checked_duration_since(bucket.last_refill)
                    .unwrap_or_default();

                bucket.tokens = (bucket.tokens + elapsed.as_secs_f64() * self.refill_per_second)
                    .min(self.capacity);
                bucket.last_refill = now;

                if bucket.tokens >= 1.0 {
                    bucket.tokens -= 1.0;
                    None
                } else {
                    Some(Duration::from_secs_f64(
                        (1.0 - bucket.tokens) / self.refill_per_second,
                    ))
                }
            };

            match wait {
                Some(delay) => tokio::time::sleep(delay).await,
                None => break,
            }
        }

        RateLimitPermit { _permit: permit }
    }

    /// Run a future while holding a rate-limit permit.
    pub async fn run<F, T>(&self, future: F) -> T
    where
        F: Future<Output = T>,
    {
        let _permit = self.acquire().await;
        future.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn spaces_requests_after_burst() {
        let limiter = RateLimiter::new(60, 1, 4);
        let start = Instant::now();

        limiter.acquire().await;
        let first = start.elapsed();
        limiter.acquire().await;
        let second = start.elapsed();

        assert!(first < Duration::from_millis(10), "first was {first:?}");
        assert!(second >= Duration::from_secs(1), "second was {second:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn enforces_concurrency_ceiling() {
        let limiter = Arc::new(RateLimiter::new(6_000, 100, 1));
        let held = limiter.clone();
        let holder = tokio::spawn(async move {
            let _permit = held.acquire().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        });

        tokio::time::sleep(Duration::from_millis(1)).await;
        let start = Instant::now();
        let _permit = limiter.acquire().await;
        let waited = start.elapsed();

        assert!(waited >= Duration::from_millis(99), "waited {waited:?}");
        holder.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn burst_allows_immediate_requests() {
        let limiter = RateLimiter::new(60, 3, 4);
        let start = Instant::now();
        for _ in 0..3 {
            limiter.acquire().await;
        }
        assert!(start.elapsed() < Duration::from_millis(10));
    }
}
