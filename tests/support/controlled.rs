//! Controlled external time and weather only; policy, persistence and rendering stay in the SUT.
use chrono::{DateTime, FixedOffset, TimeZone};
use quiet_launcher::{
    runtime::{Clock, Environment},
    weather::{Location, Weather, WeatherProvider},
};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
pub struct Time {
    pub ms: AtomicU64,
    pub wall: Mutex<DateTime<FixedOffset>>,
}
impl Default for Time {
    fn default() -> Self {
        Self {
            ms: AtomicU64::new(86_400_000),
            wall: Mutex::new(
                FixedOffset::east_opt(9 * 3600)
                    .unwrap()
                    .with_ymd_and_hms(2026, 11, 16, 9, 0, 0)
                    .unwrap(),
            ),
        }
    }
}
impl Clock for Time {
    fn monotonic(&self) -> Duration {
        Duration::from_millis(self.ms.load(Ordering::SeqCst))
    }
    fn wall_time(&self) -> DateTime<FixedOffset> {
        *self.wall.lock().unwrap()
    }
}
impl Time {
    pub fn advance(&self, seconds: i64) {
        self.ms.fetch_add((seconds * 1000) as u64, Ordering::SeqCst);
        let mut w = self.wall.lock().unwrap();
        *w += chrono::Duration::seconds(seconds);
    }
}
#[derive(Default)]
pub struct Forecast {
    pub calls: Mutex<Vec<Location>>,
    pub replies: Mutex<std::collections::VecDeque<Result<Weather, String>>>,
}
impl Forecast {
    pub fn reply(
        &self,
        temp: f64,
        code: u16,
        fetched: DateTime<FixedOffset>,
        observed: DateTime<FixedOffset>,
    ) {
        self.replies.lock().unwrap().push_back(Ok(Weather {
            temperature: temp,
            code,
            fetched_at: fetched.with_timezone(&chrono::Local),
            observed_at: Some(observed.with_timezone(&chrono::Local)),
        }));
    }
    pub fn fail(&self) {
        self.replies
            .lock()
            .unwrap()
            .push_back(Err("fixture network unavailable".into()));
    }
    pub fn environment(self: &Arc<Self>, clock: Arc<Time>) -> Environment {
        Environment {
            clock,
            weather: self.clone(),
        }
    }
}
impl WeatherProvider for Forecast {
    fn current(&self, l: &Location) -> anyhow::Result<Weather> {
        self.calls.lock().unwrap().push(l.clone());
        match self.replies.lock().unwrap().pop_front() {
            Some(Ok(w)) => Ok(w),
            Some(Err(e)) => Err(anyhow::anyhow!(e)),
            None => anyhow::bail!("unscripted weather call"),
        }
    }
}
