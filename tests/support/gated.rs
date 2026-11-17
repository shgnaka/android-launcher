//! Explicit barriers keep reordered-response tests deterministic without sleeps.
use quiet_launcher::weather::{Location, Weather, WeatherProvider};
use std::sync::{Arc, Condvar, Mutex};
#[derive(Default)]
pub struct Gate(Mutex<bool>, Condvar);
impl Gate {
    pub fn release(&self) {
        *self.0.lock().unwrap() = true;
        self.1.notify_all();
    }
    fn wait(&self) {
        let mut open = self.0.lock().unwrap();
        while !*open {
            open = self.1.wait(open).unwrap();
        }
    }
}
pub struct ReleaseOnDrop(pub Vec<Arc<Gate>>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        for g in &self.0 {
            g.release()
        }
    }
}
#[derive(Default)]
pub struct GatedForecast {
    pub calls: Mutex<Vec<Location>>,
    pub responses: Mutex<std::collections::VecDeque<(Weather, Option<Arc<Gate>>)>>,
    pub changed: Condvar,
}
impl GatedForecast {
    pub fn push(&self, w: Weather, g: Option<Arc<Gate>>) {
        self.responses.lock().unwrap().push_back((w, g));
    }
    pub fn wait_calls(&self, n: usize) {
        let mut c = self.calls.lock().unwrap();
        while c.len() < n {
            let (next, timeout) = self
                .changed
                .wait_timeout(c, std::time::Duration::from_secs(3))
                .unwrap();
            c = next;
            assert!(
                !timeout.timed_out(),
                "weather boundary has {} calls, expected {n}",
                c.len()
            );
        }
    }
}
impl WeatherProvider for GatedForecast {
    fn current(&self, l: &Location) -> anyhow::Result<Weather> {
        self.calls.lock().unwrap().push(l.clone());
        self.changed.notify_all();
        let (w, gate) = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("unscripted call"))?;
        if let Some(g) = gate {
            g.wait()
        }
        Ok(w)
    }
}
