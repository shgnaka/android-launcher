//! Runtime dependencies: the same ports support alternate providers and deterministic acceptance.
use crate::weather::{OpenMeteo, WeatherProvider};
use chrono::{DateTime, FixedOffset};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub trait Clock: Send + Sync {
    fn monotonic(&self) -> Duration;
    fn wall_time(&self) -> DateTime<FixedOffset>;
}
struct SystemClock(Instant);
impl Clock for SystemClock {
    fn monotonic(&self) -> Duration {
        self.0.elapsed() + Duration::from_secs(4)
    }
    fn wall_time(&self) -> DateTime<FixedOffset> {
        chrono::Local::now().fixed_offset()
    }
}
#[derive(Clone)]
pub struct Environment {
    pub clock: Arc<dyn Clock>,
    pub weather: Arc<dyn WeatherProvider>,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            clock: Arc::new(SystemClock(Instant::now())),
            weather: Arc::new(OpenMeteo),
        }
    }
}
