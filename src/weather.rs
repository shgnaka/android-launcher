use anyhow::{bail, Result};
use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    pub fn parse(value: &str) -> Result<Self> {
        let Some((lat, lon)) = value.split_once(',') else {
            bail!("緯度,経度 の形式で入力してください");
        };
        let location = Self {
            latitude: lat.trim().parse()?,
            longitude: lon.trim().parse()?,
        };
        location.validate()?;
        Ok(location)
    }
    pub fn validate(&self) -> Result<()> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || !(-90.0..=90.0).contains(&self.latitude)
            || !(-180.0..=180.0).contains(&self.longitude)
        {
            bail!("緯度は -90〜90、経度は -180〜180 で指定してください");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Weather {
    pub observed_at: Option<chrono::DateTime<chrono::Local>>,
    pub temperature: f64,
    pub code: u16,
    pub fetched_at: chrono::DateTime<chrono::Local>,
}

/// New providers can implement the same contract without changing the home UI.
pub trait WeatherProvider: Send + Sync {
    fn current(&self, location: &Location) -> Result<Weather>;
}

pub struct OpenMeteo;
impl WeatherProvider for OpenMeteo {
    fn current(&self, location: &Location) -> Result<Weather> {
        location.validate()?;
        #[derive(Deserialize)]
        struct Response {
            current: Current,
        }
        #[derive(Deserialize)]
        struct Current {
            time: Option<String>,
            temperature_2m: f64,
            weather_code: u16,
        }
        let response: Response = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()
            .get("https://api.open-meteo.com/v1/forecast")
            .query("latitude", &location.latitude.to_string())
            .query("longitude", &location.longitude.to_string())
            .query("current", "temperature_2m,weather_code")
            .query("timezone", "UTC")
            .call()?
            .into_json()?;
        let fetched_at = chrono::Local::now();
        let observed_at = response
            .current
            .time
            .as_deref()
            .and_then(parse_open_meteo_time);
        Ok(Weather {
            observed_at,
            temperature: response.current.temperature_2m,
            code: response.current.weather_code,
            fetched_at,
        })
    }
}

fn parse_open_meteo_time(s: &str) -> Option<chrono::DateTime<chrono::Local>> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Local));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        return Some(
            chrono::Utc
                .from_utc_datetime(&naive)
                .with_timezone(&chrono::Local),
        );
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(
            chrono::Utc
                .from_utc_datetime(&naive)
                .with_timezone(&chrono::Local),
        );
    }
    None
}

impl Weather {
    pub fn description(&self) -> &'static str {
        match self.code {
            0 => "晴れ",
            1..=3 => "曇り",
            45 | 48 => "霧",
            51..=67 | 80..=82 => "雨",
            71..=77 | 85 | 86 => "雪",
            95..=99 => "雷雨",
            _ => "天気不明",
        }
    }
}

fn round_temperature(temp: f64) -> i64 {
    temp.round() as i64
}

fn validate_weather(weather: &Weather) -> Result<()> {
    if !weather.temperature.is_finite() {
        bail!("気温が有限ではありません");
    }
    if let Some(observed_at) = weather.observed_at {
        let diff_secs = observed_at
            .signed_duration_since(weather.fetched_at)
            .num_seconds();
        if diff_secs > 600 {
            bail!("観測時刻が10分を超えて未来です");
        }
        if diff_secs < -10800 {
            bail!("観測時刻が3時間を超えて過去です");
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct CachedWeatherFile {
    location: Location,
    weather: Weather,
}

fn save_cache_file(path: &Path, location: &Location, weather: &Weather) {
    let file_data = CachedWeatherFile {
        location: location.clone(),
        weather: weather.clone(),
    };
    if let Ok(json) = serde_json::to_vec(&file_data) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        } else {
            let _ = std::fs::write(path, &json);
        }
    }
}

fn load_cache_file(path: &Path) -> Option<(Location, Weather)> {
    let bytes = std::fs::read(path).ok()?;
    let data: CachedWeatherFile = serde_json::from_slice(&bytes).ok()?;
    Some((data.location, data.weather))
}

struct FetchResult {
    generation: u64,
    location: Location,
    duration: Duration,
    result: Result<Weather>,
}

pub struct WeatherService {
    provider: Arc<dyn WeatherProvider>,
    clock: Arc<dyn crate::runtime::Clock>,
    cache_path: PathBuf,
    wake: Arc<dyn Fn() + Send + Sync>,

    enabled: bool,
    location: Option<Location>,

    current_weather: Option<Weather>,
    is_failed: bool,
    error_message: Option<String>,

    last_success_mono: Option<Duration>,
    last_failure_mono: Option<Duration>,

    generation: u64,
    pending: bool,
    pending_generation: u64,
    pending_start_mono: Option<Duration>,

    tx: mpsc::Sender<FetchResult>,
    rx: mpsc::Receiver<FetchResult>,
}

impl WeatherService {
    pub fn new(
        provider: Arc<dyn WeatherProvider>,
        clock: Arc<dyn crate::runtime::Clock>,
        cache_path: PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            provider,
            clock,
            cache_path,
            wake,
            enabled: false,
            location: None,
            current_weather: None,
            is_failed: false,
            error_message: None,
            last_success_mono: None,
            last_failure_mono: None,
            generation: 0,
            pending: false,
            pending_generation: 0,
            pending_start_mono: None,
            tx,
            rx,
        }
    }

    pub fn set_config(&mut self, enabled: bool, location: Option<Location>) {
        if self.enabled == enabled && self.location == location {
            return;
        }

        let location_changed = self.location != location;
        self.enabled = enabled;
        self.location = location;

        // Invalidate in-flight request
        self.generation += 1;
        self.pending = false;

        if location_changed {
            self.current_weather = None;
            self.is_failed = false;
            self.error_message = None;
            self.last_success_mono = None;
            self.last_failure_mono = None;

            if let Some(target_loc) = &self.location {
                if let Some((cached_loc, cached_weather)) = load_cache_file(&self.cache_path) {
                    if &cached_loc == target_loc {
                        let now_local = self.clock.wall_time().with_timezone(&chrono::Local);
                        let age = (now_local - cached_weather.fetched_at).num_seconds();
                        if age < 86400 {
                            self.current_weather = Some(cached_weather);
                            let now_mono = self.clock.monotonic();
                            let age_secs = age.max(0) as u64;
                            self.last_success_mono =
                                Some(now_mono.saturating_sub(Duration::from_secs(age_secs)));
                        }
                    }
                }
            }
        }

        if self.enabled && self.location.is_some() {
            let should_fetch = match &self.current_weather {
                Some(w) => {
                    let now_local = self.clock.wall_time().with_timezone(&chrono::Local);
                    let age = (now_local - w.fetched_at).num_seconds();
                    age >= 1800
                }
                None => true,
            };
            if should_fetch {
                self.spawn_fetch();
            }
        }
    }

    pub fn manual_refresh(&mut self) {
        if !self.enabled || self.location.is_none() {
            return;
        }
        if self.pending {
            return;
        }
        self.spawn_fetch();
    }

    pub fn tick(&mut self) {
        while let Ok(outcome) = self.rx.try_recv() {
            if outcome.generation != self.generation {
                continue;
            }
            if Some(&outcome.location) != self.location.as_ref() {
                continue;
            }
            if !self.enabled {
                continue;
            }

            self.pending = false;

            if outcome.duration > Duration::from_secs(10) {
                self.is_failed = true;
                self.error_message = Some("応答時間が10秒を超えたため拒否しました".to_string());
                self.last_failure_mono = Some(self.clock.monotonic());
                continue;
            }

            match outcome.result {
                Ok(weather) => {
                    if let Err(e) = validate_weather(&weather) {
                        self.is_failed = true;
                        self.error_message = Some(format!("無効な天気データ: {e:#}"));
                        self.last_failure_mono = Some(self.clock.monotonic());
                        continue;
                    }

                    self.current_weather = Some(weather.clone());
                    self.is_failed = false;
                    self.error_message = None;
                    self.last_success_mono = Some(self.clock.monotonic());
                    save_cache_file(&self.cache_path, &outcome.location, &weather);
                }
                Err(e) => {
                    self.is_failed = true;
                    self.error_message = Some(format!("天気を取得できません: {e:#}"));
                    self.last_failure_mono = Some(self.clock.monotonic());
                }
            }
        }

        let now_mono = self.clock.monotonic();
        if self.pending {
            if let Some(start) = self.pending_start_mono {
                if now_mono.saturating_sub(start) > Duration::from_secs(10) {
                    self.pending = false;
                    self.generation += 1;
                    self.is_failed = true;
                    self.error_message = Some("リクエストタイムアウト (10秒)".to_string());
                    self.last_failure_mono = Some(now_mono);
                    (self.wake)();
                }
            }
        }

        if !self.enabled || self.location.is_none() || self.pending {
            return;
        }

        if self.current_weather.is_none() && !self.is_failed {
            self.spawn_fetch();
            return;
        }

        if self.is_failed {
            if let Some(last_fail) = self.last_failure_mono {
                if now_mono.saturating_sub(last_fail) >= Duration::from_secs(300) {
                    self.spawn_fetch();
                }
            } else {
                self.spawn_fetch();
            }
            return;
        }

        if let Some(last_succ) = self.last_success_mono {
            if now_mono.saturating_sub(last_succ) >= Duration::from_secs(1800) {
                self.spawn_fetch();
            }
        } else {
            self.spawn_fetch();
        }
    }

    pub fn display(&self) -> Option<String> {
        if !self.enabled || self.location.is_none() {
            return None;
        }

        if let Some(weather) = &self.current_weather {
            let now_local = self.clock.wall_time().with_timezone(&chrono::Local);
            let age_seconds = (now_local - weather.fetched_at).num_seconds();

            if age_seconds >= 86400 {
                return Some("天気 —".to_string());
            }

            let desc = weather.description();
            let temp = round_temperature(weather.temperature);
            let time_str = if now_local.date_naive() == weather.fetched_at.date_naive() {
                weather.fetched_at.format("%H:%M").to_string()
            } else {
                weather.fetched_at.format("%m/%d %H:%M").to_string()
            };

            let base = format!("{desc} {temp}°C · {time_str}");
            if self.is_failed {
                Some(format!("{base} · 更新待ち"))
            } else {
                Some(base)
            }
        } else if self.is_failed {
            Some("天気 —".to_string())
        } else {
            None
        }
    }

    pub fn error(&self) -> Option<String> {
        if !self.enabled || self.location.is_none() {
            return None;
        }
        self.error_message.clone()
    }

    fn spawn_fetch(&mut self) {
        if self.pending {
            return;
        }
        let Some(location) = self.location.clone() else {
            return;
        };
        if !self.enabled {
            return;
        }

        self.generation += 1;
        let generation = self.generation;
        self.pending = true;
        self.pending_generation = generation;
        let start_mono = self.clock.monotonic();
        self.pending_start_mono = Some(start_mono);

        let provider = self.provider.clone();
        let clock = self.clock.clone();
        let tx = self.tx.clone();
        let wake = self.wake.clone();

        std::thread::spawn(move || {
            let result = provider.current(&location);
            let end_mono = clock.monotonic();
            let duration = end_mono.saturating_sub(start_mono);
            let _ = tx.send(FetchResult {
                generation,
                location,
                duration,
                result,
            });
            wake();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_coordinates_before_network_request() {
        for bad in ["NaN,139", "91,0", "0,181", "35", "0,inf", "35,139,1"] {
            assert!(Location::parse(bad).is_err(), "{bad}");
        }
        assert_eq!(Location::parse("35.68, 139.76").unwrap().longitude, 139.76);
    }
}
