//! Test-only device boundary. Rendering, search, persistence and gesture handling stay real.
use super::*;
use eframe::egui::{self, Event, FullOutput, Pos2, Rect, Vec2};
use quiet_launcher::{
    catalog::AppCatalog, config::Settings, model::*, platform::Platform, ui::Launcher,
};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
#[derive(Default)]
pub struct Device {
    pub sessions: Mutex<Vec<MediaSession>>,
    pub media_denied: AtomicBool,
    pub ignore_media_commands: AtomicBool,
    pub media_commands: Mutex<Vec<(String, MediaAction)>>,

    pub apps: Mutex<Vec<AppEntry>>,
    pub revision: AtomicUsize,
    pub enumerations: AtomicUsize,
    pub snapshot: Mutex<DeviceSnapshot>,
    pub sampled: AtomicBool,
    pub launched: Mutex<Vec<String>>,
    pub widgets: Mutex<Vec<TextWidget>>,
    pub providers: Mutex<Vec<WidgetProvider>>,
    pub text: Mutex<std::collections::VecDeque<(String, String)>>,
    pub error: Mutex<Option<String>>,
    pub insets: Mutex<[f32; 4]>,
    pub keyboard: Mutex<f32>,
    pub source_error: AtomicBool,
    pub native: AtomicBool,
    pub field: Mutex<Option<SearchField>>,
    pub removed_widgets: Mutex<Vec<i32>>,
}
impl Platform for Device {
    fn media_sessions(&self) -> anyhow::Result<Vec<MediaSession>> {
        anyhow::ensure!(
            !self.media_denied.load(Ordering::SeqCst),
            "notification access denied"
        );
        Ok(self.sessions.lock().unwrap().clone())
    }
    fn control_session(&self, id: &str, action: MediaAction) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.media_denied.load(Ordering::SeqCst),
            "notification access denied"
        );
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow::anyhow!("session disappeared"))?;
        let supported = match action {
            MediaAction::Previous => session.previous,
            MediaAction::Play => session.play,
            MediaAction::Pause => session.pause,
            MediaAction::Next => session.next,
        };
        anyhow::ensure!(supported, "unsupported action");
        self.media_commands
            .lock()
            .unwrap()
            .push((id.to_owned(), action));
        if self.ignore_media_commands.load(Ordering::SeqCst) {
            return Ok(());
        }
        match action {
            MediaAction::Play => session.state = Playback::Playing,
            MediaAction::Pause => session.state = Playback::Paused,
            MediaAction::Next => {
                session.title = Some(if id == "p1:1" { "TRACK C" } else { "TRACK Y" }.into())
            }
            MediaAction::Previous => session.title = Some("TRACK A".into()),
        };
        Ok(())
    }

    fn apps(&self) -> anyhow::Result<Vec<AppEntry>> {
        self.enumerations.fetch_add(1, Ordering::SeqCst);
        anyhow::ensure!(
            !self.source_error.load(Ordering::SeqCst),
            "catalog unavailable"
        );
        Ok(self.apps.lock().unwrap().clone())
    }
    fn catalog_revision(&self) -> anyhow::Result<Option<String>> {
        Ok(Some(format!("v1:{}", self.revision.load(Ordering::SeqCst))))
    }
    fn snapshot(&self) -> anyhow::Result<DeviceSnapshot> {
        self.sampled.store(true, Ordering::SeqCst);
        Ok(self.snapshot.lock().unwrap().clone())
    }
    fn launch(&self, id: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.apps.lock().unwrap().iter().any(|a| a.id == id),
            "uninstalled"
        );
        self.launched.lock().unwrap().push(id.into());
        Ok(())
    }
    fn settings(&self, _: SystemSettings) -> anyhow::Result<()> {
        Ok(())
    }
    fn media(&self, _: MediaCommand) -> anyhow::Result<()> {
        Ok(())
    }
    fn widget_providers(&self) -> anyhow::Result<Vec<WidgetProvider>> {
        Ok(self.providers.lock().unwrap().clone())
    }
    fn add_widget(&self, _: &str) -> anyhow::Result<i32> {
        Ok(200)
    }
    fn remove_widget(&self, id: i32) -> anyhow::Result<()> {
        self.removed_widgets.lock().unwrap().push(id);
        self.widgets.lock().unwrap().retain(|w| w.id != id);
        Ok(())
    }
    fn poll_widgets(&self, _: &[i32], _: &WidgetNames) {}
    fn widgets(&self) -> Vec<TextWidget> {
        self.widgets.lock().unwrap().clone()
    }
    fn request_text(&self, _: &str, _: &str, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn take_text(&self) -> Option<(String, String)> {
        self.text.lock().unwrap().pop_front()
    }
    fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap().take()
    }
    fn window_insets(&self) -> [f32; 4] {
        *self.insets.lock().unwrap()
    }
    fn keyboard_inset(&self) -> f32 {
        *self.keyboard.lock().unwrap()
    }
    fn is_android(&self) -> bool {
        self.native.load(Ordering::SeqCst)
    }
    fn set_search_field(&self, f: Option<SearchField>) {
        *self.field.lock().unwrap() = f;
    }
}
pub struct Screen {
    pub dir: Directory,
    pub device: Arc<Device>,
    pub app: Launcher,
    pub ctx: egui::Context,
    pub size: Vec2,
    pub now: f64,
    pub focused: bool,
    pub output: FullOutput,
}
impl Screen {
    pub fn new(config: serde_json::Value, apps: Vec<AppEntry>) -> Self {
        Self::with_environment(
            config,
            apps,
            quiet_launcher::runtime::Environment::default(),
        )
    }
    pub fn with_environment(
        mut config: serde_json::Value,
        apps: Vec<AppEntry>,
        environment: quiet_launcher::runtime::Environment,
    ) -> Self {
        let dir = Directory::new("screen");
        for (k, v) in
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(BASIC).unwrap()
        {
            if config.get(&k).is_none() {
                config[k] = v;
            }
        }
        dir.write(&serde_json::to_vec(&config).unwrap());
        let mut cache = AppCatalog::default();
        cache.refresh("v1:0", || Ok(apps.clone())).unwrap();
        cache
            .save(&dir.settings().with_extension("apps.json"))
            .unwrap();
        let device = Arc::new(Device {
            apps: Mutex::new(apps),
            ..Default::default()
        });
        let ctx = egui::Context::default();
        let app = Launcher::with_environment(&ctx, device.clone(), dir.settings(), environment);
        let mut screen = Self {
            dir,
            device,
            app,
            ctx,
            size: Vec2::new(360., 640.),
            now: 0.,
            focused: true,
            output: FullOutput::default(),
        };
        screen.frame(vec![]);
        screen
    }
    pub fn frame(&mut self, events: Vec<Event>) {
        self.now += 1. / 60.;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.now),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .focused = Some(self.focused);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut input);
        self.output = self.ctx.run(input, |ctx| self.app.render(ctx));
    }
    pub fn settle(&mut self) {
        for _ in 0..30 {
            self.frame(vec![]);
        }
    }
    pub fn pointer(&mut self, p: Pos2, down: bool) {
        self.frame(vec![
            Event::PointerMoved(p),
            Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: down,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    pub fn drag(&mut self, start: Pos2, end: Pos2) {
        self.pointer(start, true);
        for i in 1..=8 {
            self.frame(vec![Event::PointerMoved(
                start + (end - start) * (i as f32 / 8.),
            )]);
        }
        self.pointer(end, false);
        self.settle();
    }
    pub fn drawer(&mut self) {
        self.drag(Pos2::new(160., self.size.y - 100.), Pos2::new(160., 120.));
    }
    pub fn back(&mut self) {
        self.frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.frame(vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        self.settle();
    }
    pub fn search(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.into())]);
        self.settle();
    }
    pub fn labels(&self) -> Vec<String> {
        self.texts().into_iter().map(|(t, _)| t).collect()
    }
    pub fn texts(&self) -> Vec<(String, Rect)> {
        let mut result = vec![];
        for s in &self.output.shapes {
            collect(
                &s.shape,
                s.clip_rect
                    .intersect(Rect::from_min_size(Pos2::ZERO, self.size)),
                &mut result,
            );
        }
        result
    }
    pub fn has(&self, text: &str) -> bool {
        self.labels().iter().any(|t| t.contains(text))
    }
    pub fn position(&self, text: &str) -> Pos2 {
        self.texts()
            .into_iter()
            .rev()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("visible text missing {text:?}: {:?}", self.labels()))
            .1
            .center()
    }
    pub fn click(&mut self, text: &str) {
        let p = self.position(text);
        self.pointer(p, true);
        self.pointer(p, false);
        self.settle();
    }
    pub fn secondary(&mut self, text: &str) {
        let p = self.position(text);
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }
    pub fn settings(&mut self) {
        let p = Pos2::new(self.size.x - 30., 60.);
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
        assert!(
            self.has("文字サイズ") || self.has("設定を読み取れません"),
            "settings failed to open: {:?}",
            self.labels()
        );
    }
    pub fn find_scroll(&mut self, text: &str) {
        for _ in 0..25 {
            if self.labels().iter().any(|t| t == text) {
                return;
            }
            self.frame(vec![
                Event::PointerMoved(Pos2::new(self.size.x * 0.8, self.size.y * 0.6)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0., -140.),
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            self.settle();
        }
        panic!("scroll target missing: {text}; {:?}", self.labels());
    }
    pub fn saved(&self) -> Settings {
        Settings::load(&self.dir.settings()).unwrap()
    }
    pub fn wait_snapshot(&mut self) {
        let start = std::time::Instant::now();
        while !self.device.sampled.load(Ordering::SeqCst) {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(3),
                "device boundary did not reply"
            );
            std::thread::yield_now();
        }
        self.settle();
    }
    pub fn widget_poll(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(360));
        self.frame(vec![]);
    }
}
fn collect(s: &egui::Shape, clip: Rect, result: &mut Vec<(String, Rect)>) {
    match s {
        egui::Shape::Text(t) => {
            let r = t.galley.rect.translate(t.pos.to_vec2());
            if clip.intersects(r) && r.width() > 0. && r.height() > 0. {
                result.push((t.galley.text().into(), r.intersect(clip)));
            }
        }
        egui::Shape::Vec(v) => {
            for s in v {
                collect(s, clip, result)
            }
        }
        _ => {}
    }
}
