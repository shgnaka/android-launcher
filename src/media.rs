//! Ephemeral media selection and targeted commands, independent of rendering and settings.
use crate::{
    model::{MediaAction, MediaSession, Playback},
    platform::Platform,
};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

pub fn supports(session: &MediaSession, action: MediaAction) -> bool {
    match action {
        MediaAction::Previous => session.previous,
        MediaAction::Next => session.next,
        MediaAction::Play => session.play && session.state == Playback::Paused,
        MediaAction::Pause => session.pause && session.state == Playback::Playing,
    }
}
fn rank(session: &MediaSession) -> u8 {
    match session.state {
        Playback::Playing => 0,
        Playback::Buffering | Playback::Connecting => 1,
        Playback::Paused => 2,
        _ => 3,
    }
}
enum Request {
    Refresh,
    Control(String, MediaAction),
}
struct Update {
    sessions: anyhow::Result<Vec<MediaSession>>,
    command_failed: bool,
}
pub struct MediaController {
    sessions: Vec<MediaSession>,
    selected: Option<String>,
    pub unavailable: bool,
    pub command_failed: bool,
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    pending: bool,
    last_poll: Option<Duration>,
    enabled: bool,
}
impl MediaController {
    pub fn new(platform: Arc<dyn Platform>, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, requests) = mpsc::channel();
        let (updates, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let mut command_failed = false;
                if let Request::Control(id, action) = request {
                    // Revalidate the captured target, never substitute the current selection.
                    let valid = platform.media_sessions().is_ok_and(|sessions| {
                        sessions
                            .iter()
                            .any(|s| s.id == id && rank(s) < 3 && supports(s, action))
                    });
                    command_failed = !valid || platform.control_session(&id, action).is_err();
                }
                if updates
                    .send(Update {
                        sessions: platform.media_sessions(),
                        command_failed,
                    })
                    .is_err()
                {
                    break;
                }
                wake();
            }
        });
        Self {
            sessions: vec![],
            selected: None,
            unavailable: false,
            command_failed: false,
            tx,
            rx,
            pending: false,
            last_poll: None,
            enabled: false,
        }
    }
    pub fn tick(&mut self, now: Duration, enabled: bool, resumed: bool) {
        while let Ok(update) = self.rx.try_recv() {
            self.pending = false;
            self.command_failed = update.command_failed;
            if enabled {
                self.unavailable = update.sessions.is_err();
                self.sessions = update.sessions.unwrap_or_default();
                self.sessions.retain(|s| rank(s) < 3);
                self.sessions.sort_by(|a, b| {
                    (rank(a), &a.package, &a.id).cmp(&(rank(b), &b.package, &b.id))
                });
                self.sessions.dedup_by(|a, b| a.id == b.id);
                if !self
                    .sessions
                    .iter()
                    .any(|s| Some(&s.id) == self.selected.as_ref())
                {
                    self.selected = self.sessions.first().map(|s| s.id.clone());
                }
            }
        }
        if !enabled {
            self.sessions.clear();
            self.selected = None;
            self.enabled = false;
            self.last_poll = None;
            return;
        }
        if !self.pending
            && (!self.enabled
                || resumed
                || self
                    .last_poll
                    .is_none_or(|last| now.saturating_sub(last) >= Duration::from_secs(1)))
        {
            self.pending = self.tx.send(Request::Refresh).is_ok();
            self.last_poll = Some(now);
        }
        self.enabled = true;
    }
    pub fn selected(&self) -> Option<&MediaSession> {
        self.sessions
            .iter()
            .find(|s| Some(&s.id) == self.selected.as_ref())
    }
    pub fn position(&self) -> (usize, usize) {
        (
            self.sessions
                .iter()
                .position(|s| Some(&s.id) == self.selected.as_ref())
                .unwrap_or(0),
            self.sessions.len(),
        )
    }
    pub fn switch(&mut self, forward: bool) {
        let (index, count) = self.position();
        if count > 0 {
            let index = if forward {
                (index + 1).min(count - 1)
            } else {
                index.saturating_sub(1)
            };
            self.selected = Some(self.sessions[index].id.clone());
        }
    }
    pub fn control(&mut self, id: &str, action: MediaAction) {
        if self
            .selected()
            .is_some_and(|s| s.id == id && supports(s, action))
        {
            // A command queues behind any refresh; the worker validates identity again.
            if self
                .tx
                .send(Request::Control(id.to_owned(), action))
                .is_ok()
            {
                self.pending = true;
            }
        }
    }
}
