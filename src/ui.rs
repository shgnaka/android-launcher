#[cfg(test)]
use crate::search::ExactSearch;
use crate::{
    catalog::AppCatalog,
    config::Settings,
    home::HomeLayout,
    mobile::{self, ROW_HEIGHT, TOUCH_TARGET},
    model::*,
    platform::Platform,
    runtime::Environment,
    search::{AppSearch, ReadingSearch},
    weather::{Location, WeatherService},
};
use anyhow::Result;
use chrono::Datelike;
use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Vec2};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{mpsc, Arc},
    time::Duration,
};

enum Update {
    Apps(Result<Arc<Vec<AppEntry>>>),
    Done,
    Device(bool, Result<DeviceSnapshot>),
    Providers(Result<Vec<WidgetProvider>>),
}
struct Refresh {
    force_apps: bool,
    last_24h: bool,
}

pub struct Launcher {
    media: crate::media::MediaController,
    environment: Environment,
    platform: Arc<dyn Platform>,
    path: PathBuf,
    settings: Settings,
    committed_settings: Settings,
    home_height: f32,
    applied_font: u16,
    /// Kept behind a trait so a future search engine can be injected here.
    search: Box<dyn AppSearch>,
    apps: Arc<Vec<AppEntry>>,
    app_indices: HashMap<String, usize>,
    app_name_counts: HashMap<String, usize>,
    results: Vec<usize>,
    searched_query: Option<String>,
    favorite_ids: HashSet<String>,
    widgets: Vec<TextWidget>,
    device: DeviceSnapshot,
    weather_service: WeatherService,
    providers: Vec<WidgetProvider>,
    query: String,
    location_text: String,
    settings_open: bool,
    app_menu: Option<String>,
    pending_reading: Option<String>,
    error: Option<String>,
    settings_read_failed: bool,
    reset_confirmation: bool,
    refreshing: bool,
    pending_refresh: bool,
    rx: mpsc::Receiver<Update>,
    refresh: mpsc::Sender<Refresh>,
    last_refresh: Duration,
    last_widgets: Duration,
    drawer_open: bool,
    reset_drawer_scroll: bool,
    reset_home_scroll: bool,
    search_focus: bool,
    search_field: Option<SearchField>,
    last_search_poll: Duration,
    gesture_start: Option<(egui::Pos2, bool, bool)>,
    reset_after_launch: bool,
    last_screen_size: Option<Vec2>,
    was_focused: bool,
    last_interaction: Duration,
    scrolling: bool,
    theme: Option<bool>,
    rendered_rows: usize,
    scroll_offset: f32,
    touch_input: mobile::TouchInput,
}

impl Launcher {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        platform: Arc<dyn Platform>,
        path: PathBuf,
    ) -> Self {
        Self::from_context(&cc.egui_ctx, platform, path)
    }

    pub fn from_context(ctx: &egui::Context, platform: Arc<dyn Platform>, path: PathBuf) -> Self {
        Self::with_environment(ctx, platform, path, Environment::default())
    }
    pub fn with_environment(
        ctx: &egui::Context,
        platform: Arc<dyn Platform>,
        path: PathBuf,
        environment: Environment,
    ) -> Self {
        install_fonts(ctx);
        let (settings, error, failed) = match Settings::load(&path) {
            Ok(settings) => (settings, None, false),
            Err(error) => (
                Settings::default(),
                Some(format!("{error:#}。元のファイルは保持しています。")),
                true,
            ),
        };
        let location_text = settings
            .weather_location
            .as_ref()
            .map(|location| format!("{}, {}", location.latitude, location.longitude))
            .unwrap_or_default();
        let (tx, rx) = mpsc::channel();
        let (refresh, commands) = mpsc::channel::<Refresh>();
        let worker = platform.clone();
        let media_context = ctx.clone();
        let weather_context = ctx.clone();
        let ctx = ctx.clone();
        let cache_path = path.with_extension("apps.json");
        let mut catalog = AppCatalog::load(&cache_path).unwrap_or_default();
        let initial_apps = catalog.apps();
        std::thread::spawn(move || {
            let mut providers_loaded = false;
            while let Ok(request) = commands.recv() {
                if request.force_apps {
                    catalog.invalidate();
                }
                let result = worker.catalog_revision().and_then(|revision| {
                    catalog.refresh(revision.as_deref().unwrap_or("session-once"), || {
                        worker.apps()
                    })
                });
                match result {
                    Ok(true) => {
                        if let Err(error) = catalog.save(&cache_path) {
                            log::warn!("アプリキャッシュ: {error:#}");
                        }
                        if tx.send(Update::Apps(Ok(catalog.apps()))).is_err() {
                            break;
                        }
                        ctx.request_repaint();
                        providers_loaded = false;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        let _ = tx.send(Update::Apps(Err(error)));
                    }
                }
                let _ = tx.send(Update::Device(
                    request.last_24h,
                    worker.snapshot_range(request.last_24h),
                ));
                if !providers_loaded {
                    let _ = tx.send(Update::Providers(worker.widget_providers()));
                    providers_loaded = true;
                }
                if tx.send(Update::Done).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let _ = refresh.send(Refresh {
            force_apps: false,
            last_24h: settings.usage_last_24h,
        });
        let started = environment.clock.monotonic();
        let media = crate::media::MediaController::new(
            platform.clone(),
            Arc::new(move || media_context.request_repaint()),
        );
        let mut weather_service = WeatherService::new(
            environment.weather.clone(),
            environment.clock.clone(),
            path.with_extension("weather.json"),
            Arc::new(move || weather_context.request_repaint()),
        );
        weather_service.set_config(settings.show_weather, settings.weather_location.clone());
        let mut launcher = Self {
            environment,
            platform,
            path,
            favorite_ids: settings.favorites.iter().cloned().collect(),
            committed_settings: settings.clone(),
            home_height: f32::INFINITY,
            applied_font: 0,
            search: Box::new(ReadingSearch::with_readings(
                settings.search_readings.clone(),
            )),
            settings,
            apps: Arc::new(vec![]),
            app_indices: HashMap::new(),
            app_name_counts: HashMap::new(),
            results: vec![],
            searched_query: None,
            widgets: vec![],
            device: DeviceSnapshot::default(),
            weather_service,
            providers: vec![],
            query: String::new(),
            location_text,
            settings_open: false,
            app_menu: None,
            pending_reading: None,
            error,
            settings_read_failed: failed,
            reset_confirmation: false,
            refreshing: true,
            pending_refresh: false,
            rx,
            refresh,
            last_refresh: started,
            last_widgets: started.saturating_sub(Duration::from_secs(3)),
            drawer_open: false,
            reset_drawer_scroll: false,
            reset_home_scroll: false,
            search_focus: false,
            search_field: None,
            last_search_poll: started,
            gesture_start: None,
            reset_after_launch: false,
            last_screen_size: None,
            was_focused: true,
            last_interaction: started,
            scrolling: false,
            theme: None,
            rendered_rows: 0,
            scroll_offset: 0.0,
            touch_input: mobile::TouchInput::default(),
            media,
        };
        launcher.replace_shared_apps(initial_apps);
        launcher
    }

    pub fn set_search_engine(&mut self, search: Box<dyn AppSearch>) {
        self.search = search;
        self.searched_query = None;
    }

    fn open_drawer(&mut self) {
        self.drawer_open = true;
        self.settings_open = false;
        self.reset_drawer_scroll = true;
        self.search_focus = true;
    }

    fn go_home(&mut self) {
        self.drawer_open = false;
        self.reset_home_scroll = true;
        self.settings_open = false;
        self.query.clear();
        self.app_menu = None;
        self.pending_reading = None;
        self.reset_confirmation = false;
        self.gesture_start = None;
        self.platform.dismiss_keyboard();
    }

    fn navigation_gesture(&mut self, ctx: &egui::Context) {
        let (pressed, released, position, cancelled) = ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_released(),
                input.pointer.interact_pos(),
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Touch {
                            phase: egui::TouchPhase::Cancel,
                            ..
                        }
                    )
                }),
            )
        });
        if cancelled {
            self.gesture_start = None;
            return;
        }
        if pressed {
            self.gesture_start =
                position.map(|pos| (pos, self.drawer_open, self.scroll_offset <= 1.0));
        }
        if released {
            if let (Some((start, drawer, at_top)), Some(end)) =
                (self.gesture_start.take(), position)
            {
                let delta = end - start;
                if !self.settings_open
                    && drawer == self.drawer_open
                    && delta.y.abs() >= delta.x.abs() * 1.5
                {
                    if !drawer && delta.y <= -48.0 {
                        self.open_drawer();
                    } else if drawer && at_top && delta.y >= 112.0 {
                        self.go_home();
                    }
                }
            }
        }
    }

    pub fn replace_apps(&mut self, apps: Vec<AppEntry>) {
        self.replace_shared_apps(Arc::new(apps));
    }

    fn replace_shared_apps(&mut self, apps: Arc<Vec<AppEntry>>) {
        use unicode_normalization::UnicodeNormalization;
        let mut normalized = apps.as_ref().clone();
        for app in &mut normalized {
            if app.label.trim().is_empty() {
                app.label = app.id.split('/').next().unwrap_or(&app.id).to_owned();
            }
        }
        normalized.sort_by_cached_key(|app| {
            (
                app.label
                    .nfkc()
                    .flat_map(char::to_lowercase)
                    .collect::<String>(),
                app.id.clone(),
            )
        });
        let apps = Arc::new(normalized);
        if self.apps != apps {
            self.apps = apps;
            self.app_name_counts.clear();
            for app in self.apps.iter() {
                *self.app_name_counts.entry(app.label.clone()).or_default() += 1;
            }
            self.app_indices = self
                .apps
                .iter()
                .enumerate()
                .map(|(index, app)| (app.id.clone(), index))
                .collect();
            self.searched_query = None;
        }
    }

    fn home_layout_for(&self, settings: &Settings) -> HomeLayout {
        HomeLayout {
            height: self.home_height,
            font_percent: settings.font_percent,
            favorites: settings
                .visible_favorites()
                .iter()
                .filter(|id| self.app_indices.contains_key(**id))
                .count(),
            widgets: settings.widget_ids.len().max(self.widgets.len()),
            weather: settings.show_weather,
            usage: settings.show_usage,
            media: settings.show_media,
        }
    }
    fn save(&mut self) {
        if self.settings_read_failed {
            self.error = Some("設定を読み取れません。バックアップから復元してください。".into());
            return;
        }
        let candidate = self.home_layout_for(&self.settings);
        if !candidate.fits()
            && candidate.required_height()
                >= self
                    .home_layout_for(&self.committed_settings)
                    .required_height()
        {
            self.settings = self.committed_settings.clone();
            self.error =
                Some("ホームに収まりません。文字サイズ・表示項目を減らしてください。".into());
            return;
        }
        match self.settings.save(&self.path) {
            Ok(()) => {
                self.committed_settings = self.settings.clone();
                self.favorite_ids = self.settings.favorites.iter().cloned().collect();
                self.searched_query = None;
                self.search = Box::new(ReadingSearch::with_readings(
                    self.settings.search_readings.clone(),
                ));
            }
            Err(error) => {
                self.settings = self.committed_settings.clone();
                self.error = Some(format!("保存できません: {error:#}"));
            }
        }
    }
    fn action(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.error = Some(format!("{error:#}"));
        }
    }

    fn launch(&mut self, id: &str) {
        if self.settings_read_failed || self.settings.is_hidden(id) {
            return;
        }
        match self.platform.launch(id) {
            Ok(()) => {
                self.reset_after_launch = true;
            }
            Err(error) => {
                self.error = Some(format!("{error:#}"));
            }
        }
    }

    fn refresh(&mut self, weather: bool) {
        self.weather_service.set_config(
            self.settings.show_weather,
            self.settings.weather_location.clone(),
        );
        if weather {
            self.weather_service.manual_refresh();
        }
        if self.refreshing {
            self.pending_refresh = true;
            return;
        }
        self.refreshing = true;
        let _ = self.refresh.send(Refresh {
            force_apps: false,
            last_24h: self.settings.usage_last_24h,
        });
        self.last_refresh = self.environment.clock.monotonic();
    }

    fn receive(&mut self) {
        for id in self.platform.take_cancelled_widgets() {
            self.settings.widget_ids.retain(|item| *item != id);
            self.settings.widget_names.remove(&id);
            self.widgets.retain(|w| w.id != id);
            self.save();
            if !self.settings.widget_ids.contains(&id) {
                let result = self.platform.remove_widget(id);
                self.action(result);
            }
            self.go_home();
        }
        while let Ok(update) = self.rx.try_recv() {
            match update {
                Update::Done => self.refreshing = false,
                Update::Apps(Ok(apps)) => {
                    self.replace_shared_apps(apps);
                }
                Update::Apps(Err(error)) => {
                    self.error = Some(format!("アプリ一覧: {error:#}"));
                }
                Update::Device(range, Ok(device)) => {
                    if range == self.settings.usage_last_24h {
                        self.device = device;
                    }
                }
                Update::Device(_, Err(error)) => {
                    self.error = Some(format!("端末情報: {error:#}"));
                }
                Update::Providers(result) => {
                    self.refreshing = false;
                    match result {
                        Ok(providers) => self.providers = providers,
                        Err(error) => self.error = Some(format!("ウィジェット一覧: {error:#}")),
                    }
                }
            }
        }
        if self.pending_refresh && !self.refreshing {
            self.pending_refresh = false;
            self.refresh(false);
        }
        if let Some(error) = self.platform.take_error() {
            self.error = Some(error);
        }
        if let Some((key, value)) = self.platform.take_text() {
            match key.as_str() {
                "search" if self.drawer_open && !self.settings_open => self.query = value,
                "search_blurred" => {
                    self.search_field = None;
                }
                "home" => self.go_home(),
                "location" => self.location_text = value,
                key if key.starts_with("reading:") => {
                    let id = key.trim_start_matches("reading:").to_owned();
                    if self.pending_reading.as_ref() != Some(&id) {
                        return;
                    }
                    self.pending_reading = None;
                    if value.trim().is_empty() {
                        self.settings.search_readings.remove(&id);
                    } else {
                        self.settings.search_readings.insert(id, value);
                    }
                    self.save();
                }
                _ => {}
            }
        }
    }

    fn input(&mut self, ui: &mut egui::Ui, key: &str, title: &str, value: &mut String) -> bool {
        if self.platform.is_android() {
            let label = if value.is_empty() {
                format!("{title}…")
            } else {
                value.clone()
            };
            let size = Vec2::new(ui.available_width(), TOUCH_TARGET);
            if mobile::text_button(ui, &label, size, 18.0).clicked() {
                self.action(self.platform.request_text(key, title, value));
            }
            false
        } else {
            ui.add_sized(
                [ui.available_width(), 40.0],
                egui::TextEdit::singleline(value).hint_text(title),
            )
            .changed()
        }
    }

    fn home_line(ui: &mut egui::Ui, text: &str, size: f32, height: f32) {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            egui::Sense::hover(),
        );
        mobile::paint_button(ui, rect, &response, text, size);
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let scale = self.settings.font_percent as f32 / 100.0;
        let now = self.environment.clock.wall_time();
        ui.add_space(8.0 * scale);
        Self::home_line(
            ui,
            &now.format("%H:%M").to_string(),
            (68.0 * scale).min(ui.available_width() / 3.5),
            80.0 * scale,
        );
        let weekday = ["月", "火", "水", "木", "金", "土", "日"]
            [now.weekday().num_days_from_monday() as usize];
        let date = format!(
            "{}年{}月{}日（{}）",
            now.year(),
            now.month(),
            now.day(),
            weekday
        );
        Self::home_line(
            ui,
            &date,
            (16.0 * scale).min(ui.available_width() / date.chars().count() as f32),
            24.0 * scale,
        );
        Self::home_line(
            ui,
            &self
                .device
                .battery_percent
                .map(|p| format!("電池 {p}%"))
                .unwrap_or_else(|| "電池 —".into()),
            12.0 * scale,
            16.0 * scale,
        );
        ui.add_space(16.0 * scale);
        if self.settings_read_failed {
            ui.label("設定を読み取れません。設定から復元してください。");
            return;
        }
        if !self.home_layout_for(&self.settings).fits() {
            ui.label("ホームに収まりません。設定で文字サイズ・表示項目を減らしてください。");
            return;
        }
        if let Some(text) = self.weather_service.display() {
            Self::home_line(ui, &text, 16.0 * scale, 24.0 * scale);
        }
        if self.settings.show_usage {
            let text = self
                .device
                .screen_seconds
                .map(|seconds| {
                    format!(
                        "{} {}時間{:02}分",
                        if self.settings.usage_last_24h {
                            "直近24時間"
                        } else {
                            "今日"
                        },
                        seconds / 3600,
                        seconds / 60 % 60
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "{} —",
                        if self.settings.usage_last_24h {
                            "直近24時間"
                        } else {
                            "今日"
                        }
                    )
                });
            Self::home_line(ui, &text, 16.0 * scale, 24.0 * scale);
        }
        if self.settings.show_media {
            self.media_ui(ui);
        }
        for widget in &self.widgets {
            if !self.settings.widget_ids.contains(&widget.id) {
                continue;
            }
            Self::home_line(ui, &widget.title, 12.0 * scale, 24.0 * scale);
            Self::home_line(ui, &widget.lines.join(" · "), 16.0 * scale, 24.0 * scale);
        }
        let favorites: Vec<String> = self
            .settings
            .visible_favorites()
            .iter()
            .filter(|id| self.app_indices.contains_key(**id))
            .map(|id| (*id).to_owned())
            .collect();
        if favorites.is_empty() {
            if mobile::text_button(
                ui,
                "＋ アプリを追加",
                Vec2::new(ui.available_width(), 48.0),
                18.0 * scale,
            )
            .clicked()
            {
                self.open_drawer();
            }
        } else {
            for id in favorites {
                let label = self.apps[self.app_indices[&id]].label.clone();
                ui.push_id(&id, |ui| {
                    if mobile::text_button(
                        ui,
                        &label,
                        Vec2::new(ui.available_width(), 48.0f32.max(32.0 * scale)),
                        27.0 * scale,
                    )
                    .clicked()
                    {
                        self.launch(&id);
                    }
                    ui.add_space(8.0);
                });
            }
        }
    }

    fn drawer(&mut self, ui: &mut egui::Ui, native_ready: bool) {
        ui.horizontal(|ui| {
            let width =
                (ui.available_width() - TOUCH_TARGET - ui.spacing().item_spacing.x).max(24.0);
            if self.platform.is_android() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(width, TOUCH_TARGET), egui::Sense::click());
                if response.clicked() {
                    self.search_focus = true;
                }
                if self.drawer_open && native_ready && self.app_menu.is_none() {
                    let scale = ui.ctx().pixels_per_point();
                    let field = SearchField {
                        rect: [rect.left(), rect.top(), rect.width(), rect.height()]
                            .map(|x| (x * scale).round() as i32),
                        text: self.query.clone(),
                        dark: self.settings.dark,
                        focus: std::mem::take(&mut self.search_focus),
                        font_percent: self.settings.font_percent,
                    };
                    if self.search_field.as_ref().is_none_or(|previous| {
                        previous.rect != field.rect
                            || previous.dark != field.dark
                            || previous.font_percent != field.font_percent
                            || field.focus
                    }) {
                        self.platform.set_search_field(Some(field.clone()));
                        self.search_field = Some(field);
                    }
                }
            } else {
                let response = ui.add_sized(
                    [width, TOUCH_TARGET],
                    egui::TextEdit::singleline(&mut self.query)
                        .id(egui::Id::new("drawer-input"))
                        .frame(false)
                        .font(FontId::proportional(
                            18.0 * self.settings.font_percent as f32 / 100.0,
                        )),
                );
                if native_ready && std::mem::take(&mut self.search_focus) {
                    response.request_focus();
                }
            }
            let menu = ui
                .scope(|ui| {
                    ui.spacing_mut().interact_size = Vec2::splat(TOUCH_TARGET);
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    ui.menu_button("   ", |ui| {
                        if ui.button("設定").clicked() {
                            self.settings_open = true;
                            self.platform.dismiss_keyboard();
                            ui.close_menu();
                        }
                    })
                })
                .inner;
            let center = menu.response.rect.center();
            for y in [-6.0, 0.0, 6.0] {
                ui.painter().circle_filled(
                    center + Vec2::new(0.0, y),
                    1.6,
                    ui.visuals().text_color(),
                );
            }
            menu.response.on_hover_text("メニュー");
        });
        ui.add_space(8.0);
        let changed = self.searched_query.as_ref() != Some(&self.query);
        if changed {
            self.results = self
                .search
                .search(&self.query, &self.apps)
                .iter()
                .filter(|app| !self.settings_read_failed && !self.settings.is_hidden(&app.id))
                .filter_map(|app| self.app_indices.get(&app.id).copied())
                .collect();
            self.searched_query = Some(self.query.clone());
        }
        let mut scroll = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt("drawer-list")
            .drag_to_scroll(true);
        if changed || std::mem::take(&mut self.reset_drawer_scroll) {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        let output = scroll.show(ui, |ui| {
            if self.results.is_empty() {
                ui.label(if self.refreshing {
                    "アプリを読み込み中…"
                } else {
                    "一致するアプリはありません"
                });
            }
            let (list, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), self.results.len() as f32 * ROW_HEIGHT),
                egui::Sense::hover(),
            );
            self.rendered_rows = 0;
            for row in mobile::visible_rows(list, ui.clip_rect(), self.results.len()) {
                let app = &self.apps[self.results[row]];
                let rect = egui::Rect::from_min_size(
                    list.min + Vec2::new(0.0, row as f32 * ROW_HEIGHT),
                    Vec2::new(list.width(), ROW_HEIGHT),
                );
                let identity = (self.app_name_counts.get(&app.label).copied().unwrap_or(0) > 1)
                    .then_some(app.id.as_str());
                let (launch, favorite, menu) = mobile::app_row_with_identity(
                    ui,
                    rect,
                    &app.id,
                    &app.label,
                    self.favorite_ids.contains(&app.id),
                    identity,
                );
                self.rendered_rows += 1;
                if launch {
                    let id = app.id.clone();
                    self.launch(&id);
                } else if menu {
                    self.app_menu = Some(app.id.clone());
                    self.platform.dismiss_keyboard();
                } else if favorite {
                    let id = app.id.clone();
                    self.settings.toggle_favorite(&id);
                    self.save();
                }
            }
        });
        self.scrolling = output.state.velocity().y.abs() > 1.0;
        self.scroll_offset = output.state.offset.y;
        // Keep the inline field mounted while scrolling; only dismiss its IME.
        if ui.input(|input| input.pointer.primary_down() && input.pointer.delta().y < -2.0)
            && output.inner_rect.contains(
                ui.input(|input| input.pointer.interact_pos())
                    .unwrap_or_default(),
            )
        {
            self.platform.dismiss_keyboard();
        }
    }

    fn media_ui(&mut self, ui: &mut egui::Ui) {
        crate::media_ui::panel(
            ui,
            &mut self.media,
            self.settings.font_percent as f32 / 100.,
        );
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 6.0;
        if self.settings_read_failed {
            ui.label("設定を読み取れません。元のファイルは保持しています。");
            if ui.button("バックアップから復元").clicked() {
                match Settings::recover(&self.path) {
                    Ok(settings) => {
                        self.settings = settings.clone();
                        self.committed_settings = settings;
                        self.settings_read_failed = false;
                        self.error = None;
                        self.searched_query = None;
                        self.favorite_ids = self.settings.favorites.iter().cloned().collect();
                        self.search = Box::new(ReadingSearch::with_readings(
                            self.settings.search_readings.clone(),
                        ));
                    }
                    Err(error) => self.error = Some(format!("復元できません: {error:#}")),
                }
            }
            if !self.reset_confirmation {
                if ui.button("設定を初期化").clicked() {
                    self.reset_confirmation = true;
                }
            } else {
                ui.label("保存された設定を初期化しますか？ 元のファイルは退避します。");
                if ui.button("キャンセル").clicked() {
                    self.reset_confirmation = false;
                }
                if ui.button("初期化する").clicked() {
                    match crate::recovery::reset(&self.path) {
                        Ok(settings) => {
                            self.settings = settings.clone();
                            self.committed_settings = settings;
                            self.settings_read_failed = false;
                            self.reset_confirmation = false;
                            self.error = None;
                            self.favorite_ids.clear();
                            self.searched_query = None;
                            self.search = Box::new(ReadingSearch::with_readings(
                                self.settings.search_readings.clone(),
                            ));
                        }
                        Err(error) => self.error = Some(format!("初期化できません: {error:#}")),
                    }
                }
            }
            return;
        }
        ui.label("文字サイズ");
        if ui
            .add(
                egui::Slider::new(&mut self.settings.font_percent, 80..=160)
                    .step_by(10.0)
                    .suffix("%"),
            )
            .changed()
        {
            self.save();
        }
        ui.horizontal(|ui| {
            ui.label("画面時間");
            let a = ui
                .selectable_value(&mut self.settings.usage_last_24h, false, "今日")
                .changed();
            let b = ui
                .selectable_value(&mut self.settings.usage_last_24h, true, "直近24時間")
                .changed();
            if a || b {
                self.save();
                self.device.screen_seconds = None;
                self.refresh(false);
            }
        });
        ui.heading("表示と連携");
        if ui.button("通知アクセス").clicked() {
            self.reset_after_launch = true;
            self.action(self.platform.media_access_settings());
        }
        if self.settings.show_media {
            ui.label(if self.media.unavailable {
                "メディアを取得できません。通知アクセスを確認してください"
            } else if self.media.selected().is_none() {
                "操作可能なメディアはありません"
            } else {
                "メディア接続中"
            });
            if let Some(session) = self.media.selected().cloned() {
                ui.collapsing("メディアの詳細", |ui| {
                    ui.label(&session.provider);
                    if let Some(title) = &session.title {
                        ui.label(title);
                    }
                    ui.label(match session.state {
                        Playback::Playing => "再生中",
                        Playback::Paused => "一時停止中",
                        _ => "接続待機中",
                    });
                });
            }
            if self.media.command_failed {
                ui.label("メディア操作を完了できませんでした");
            }
        }
        ui.add_space(12.0);
        if ui.button("標準のホームアプリに設定").clicked() {
            self.reset_after_launch = true;
            self.action(self.platform.settings(SystemSettings::Home));
        }
        let changed = ui.checkbox(&mut self.settings.dark, "暗い背景").changed()
            | ui.checkbox(&mut self.settings.show_weather, "天気")
                .changed()
            | ui.checkbox(&mut self.settings.show_usage, "スクリーンタイム")
                .changed()
            | ui.checkbox(&mut self.settings.show_media, "メディアコントロール")
                .changed();
        if changed {
            self.save();
        }
        if ui.button("利用状況へのアクセス設定").clicked() {
            self.reset_after_launch = true;
            self.action(self.platform.settings(SystemSettings::Usage));
        }
        ui.add_space(20.0);
        ui.heading("天気の地域");
        ui.label("緯度,経度（例: 35.68,139.76）");
        let mut location = self.location_text.clone();
        self.input(ui, "location", "緯度,経度", &mut location);
        self.location_text = location;
        ui.horizontal_wrapped(|ui| {
            if ui.button("地域を保存").clicked() {
                match Location::parse(&self.location_text) {
                    Ok(location) => {
                        self.settings.weather_location = Some(location);
                        self.save();
                        self.refresh(true);
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
            if ui.button("地域を解除").clicked() {
                self.settings.weather_location = None;
                self.location_text.clear();
                self.save();
            }
        });
        ui.label(
            RichText::new("指定した座標を Open-Meteo に送信します。")
                .small()
                .weak(),
        );
        if let Some(error) = self.weather_service.error() {
            ui.collapsing("天気の取得エラー", |ui| {
                ui.label(&error);
            });
        }
        ui.hyperlink_to("Weather data by Open-Meteo.com", "https://open-meteo.com/");
        ui.add_space(20.0);
        ui.heading("ホームの並び順");
        let visible: Vec<String> = self
            .settings
            .visible_favorites()
            .iter()
            .filter(|id| self.app_indices.contains_key(**id))
            .map(|id| (*id).to_owned())
            .collect();
        for (index, id) in visible.iter().enumerate() {
            let label = self
                .app_indices
                .get(id)
                .map(|&i| self.apps[i].label.clone())
                .unwrap_or_else(|| format!("未インストール: {id}"));
            ui.horizontal_wrapped(|ui| {
                ui.label(label);
                if ui.add_enabled(index > 0, egui::Button::new("↑")).clicked() {
                    self.settings.move_visible_favorite(id, -1);
                    self.save();
                }
                if ui
                    .add_enabled(index + 1 < visible.len(), egui::Button::new("↓"))
                    .clicked()
                {
                    self.settings.move_visible_favorite(id, 1);
                    self.save();
                }
                if ui.small_button("外す").clicked() {
                    self.settings.favorites.retain(|item| item != id);
                    self.save();
                }
            });
        }
        let missing: Vec<String> = self
            .settings
            .visible_favorites()
            .into_iter()
            .filter(|id| !self.app_indices.contains_key(*id))
            .map(str::to_owned)
            .collect();
        if !missing.is_empty() {
            ui.add_space(8.0);
            for (index, id) in missing.into_iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    if index == 0 {
                        ui.heading("未インストールの登録");
                    }
                    ui.label(&id);
                    if ui.button("外す").clicked() {
                        self.settings.favorites.retain(|item| item != &id);
                        self.save();
                    }
                });
            }
        }
        ui.add_space(20.0);
        ui.heading("非表示アプリ");
        for package in self.settings.hidden_packages.clone() {
            let name = self
                .apps
                .iter()
                .find(|app| app.id.split('/').next() == Some(package.as_str()))
                .map(|app| app.label.clone())
                .unwrap_or_else(|| package.clone());
            ui.horizontal_wrapped(|ui| {
                ui.label(name);
                if !self
                    .apps
                    .iter()
                    .any(|app| app.id.split('/').next() == Some(package.as_str()))
                {
                    ui.label("未インストール");
                }
                if ui.button("再表示").clicked() {
                    self.settings.set_hidden(&package, false);
                    self.save();
                }
            });
        }
        ui.add_space(20.0);
        ui.heading("他のアプリのウィジェット");
        ui.collapsing("対応する内容", |ui| {
            ui.label("文字と説明文を表示します。画像・独自描画・ボタン操作は未対応です。");
        });
        for id in self.settings.widget_ids.clone() {
            ui.vertical(|ui| {
                let widgets = &self.widgets;
                let label = widgets
                    .iter()
                    .find(|widget| widget.id == id)
                    .map(|widget| widget.title.clone())
                    .unwrap_or_else(|| widget_title(id, &self.settings.widget_names, None));
                ui.label(label);
                if ui
                    .add_sized(
                        [ui.available_width(), TOUCH_TARGET],
                        egui::Button::new("削除"),
                    )
                    .clicked()
                {
                    match self.platform.remove_widget(id) {
                        Ok(()) => {
                            self.settings.widget_ids.retain(|item| *item != id);
                            self.settings.widget_names.remove(&id);
                            self.widgets.retain(|widget| widget.id != id);
                            self.save();
                        }
                        Err(error) => self.error = Some(format!("{error:#}")),
                    }
                }
            });
        }
        ui.collapsing("ウィジェットの全文", |ui| {
            for widget in &self.widgets {
                ui.label(&widget.title);
                for line in &widget.lines {
                    ui.label(line);
                }
            }
        });
        ui.collapsing("＋ ウィジェットを選ぶ", |ui| {
            if self.providers.is_empty() {
                ui.label("利用できるウィジェットがありません");
            }
            for provider in self.providers.clone() {
                if ui
                    .button(&provider.label)
                    .on_hover_text(&provider.component)
                    .clicked()
                {
                    let candidate = HomeLayout {
                        widgets: self.settings.widget_ids.len() + 1,
                        ..self.home_layout_for(&self.settings)
                    };
                    if !candidate.fits() {
                        self.error =
                            Some("ホームに収まりません。表示項目を減らしてください。".into());
                        continue;
                    }
                    match self.platform.add_widget(&provider.component) {
                        Ok(id) => {
                            self.reset_after_launch = true;
                            self.settings.widget_ids.push(id);
                            self.settings
                                .widget_names
                                .insert(id, provider.label.clone());
                            self.save();
                            self.last_widgets = self
                                .environment
                                .clock
                                .monotonic()
                                .saturating_sub(Duration::from_secs(3));
                        }
                        Err(error) => self.error = Some(format!("{error:#}")),
                    }
                }
            }
        });
        ui.add_space(24.0);
        if ui
            .add_enabled(!self.refreshing, egui::Button::new("端末情報と天気を更新"))
            .clicked()
        {
            self.refresh(true);
        }
        ui.add_space(40.0);
    }
}

impl Launcher {
    pub fn render(&mut self, ctx: &egui::Context) {
        self.receive();
        self.weather_service.set_config(
            self.settings.show_weather,
            self.settings.weather_location.clone(),
        );
        let size = ctx.screen_rect().size();
        if self
            .last_screen_size
            .is_some_and(|old| (old.x - size.x).abs() > 1.0 && (old.x > old.y) != (size.x > size.y))
        {
            self.go_home();
        }
        self.last_screen_size = Some(size);
        let menu_was_open = self.app_menu.is_some();
        self.navigation_gesture(ctx);
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        if focused && !self.was_focused {
            self.refresh(false);
            self.last_widgets = self
                .environment
                .clock
                .monotonic()
                .saturating_sub(Duration::from_secs(3));
            if self.reset_after_launch {
                self.go_home();
                self.reset_after_launch = false;
            }
        }
        if focused {
            self.weather_service.tick();
        }
        if focused || !self.settings.show_media {
            self.media.tick(
                self.environment.clock.monotonic(),
                self.settings.show_media,
                focused && !self.was_focused,
            );
        }
        self.was_focused = focused;
        let touching =
            ctx.input(|input| input.pointer.any_down() || input.raw_scroll_delta != Vec2::ZERO);
        if touching || self.scrolling {
            self.last_interaction = self.environment.clock.monotonic();
        }
        let idle = self
            .environment
            .clock
            .monotonic()
            .saturating_sub(self.last_interaction)
            >= Duration::from_millis(350);
        if touching || self.scrolling {
            ctx.request_repaint();
        }
        if focused && idle {
            if self
                .environment
                .clock
                .monotonic()
                .saturating_sub(self.last_refresh)
                >= Duration::from_secs(30)
            {
                self.refresh(false);
            }
            // Consume completed main-thread extraction on the next frame. The two-second
            // interval throttles requests, not delivery of already available content.
            if let Some(widgets) = self.platform.try_widgets() {
                let mut names_changed = false;
                for widget in &widgets {
                    if !widget.pending
                        && !widget.title.trim().is_empty()
                        && widget.title != format!("未連携 ({})", widget.id)
                        && self.settings.widget_names.get(&widget.id) != Some(&widget.title)
                    {
                        self.settings
                            .widget_names
                            .insert(widget.id, widget.title.clone());
                        names_changed = true;
                    }
                }
                self.widgets = widgets;
                if names_changed {
                    self.save();
                }
            }
            if self
                .environment
                .clock
                .monotonic()
                .saturating_sub(self.last_widgets)
                >= Duration::from_secs(2)
            {
                self.platform
                    .poll_widgets(&self.settings.widget_ids, &self.settings.widget_names);
                self.last_widgets = self.environment.clock.monotonic();
            }
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.go_home();
        }
        if self.theme != Some(self.settings.dark) {
            self.platform.set_dark_theme(self.settings.dark);
            ctx.set_theme(if self.settings.dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            let mut visuals = if self.settings.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            visuals.panel_fill = if self.settings.dark {
                Color32::from_rgb(24, 26, 23)
            } else {
                Color32::from_rgb(244, 241, 233)
            };
            visuals.override_text_color = Some(if self.settings.dark {
                Color32::from_rgb(231, 230, 219)
            } else {
                Color32::from_rgb(34, 36, 31)
            });
            ctx.set_visuals(visuals);
            self.theme = Some(self.settings.dark);
        }
        if self.applied_font != self.settings.font_percent {
            let scale = self.settings.font_percent as f32 / 100.0;
            let mut style = (*ctx.style()).clone();
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(16.0 * scale));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(16.0 * scale));
            ctx.set_style(style);
            self.applied_font = self.settings.font_percent;
        }
        let insets = self
            .platform
            .window_insets()
            .map(|pixels| (pixels / ctx.pixels_per_point()).ceil().clamp(0.0, 120.0));
        let side = (ctx.screen_rect().width() * 0.06).clamp(12.0, 28.0) as i8;
        let margin = egui::Margin {
            left: (side as f32 + insets[0]).min(127.0) as i8,
            right: (side as f32 + insets[2]).min(127.0) as i8,
            top: 8,
            bottom: 8,
        };
        if let Some(error) = self.error.clone() {
            egui::TopBottomPanel::bottom("error")
                .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                    left: margin.left,
                    right: margin.right,
                    top: 8,
                    bottom: (8.0 + insets[3]).min(127.0) as i8,
                }))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(if error.starts_with("ホームに収まりません") {
                            "ホームに収まりません"
                        } else {
                            "操作を完了できませんでした"
                        });
                        if ui.small_button("閉じる").clicked() {
                            self.error = None;
                        }
                    });
                    ui.collapsing("詳細", |ui| {
                        ui.label(error);
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin {
                        top: (16.0 + insets[1]).min(127.0) as i8,
                        bottom: (8.0 + insets[3]).min(127.0) as i8,
                        ..margin
                    }),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
                ui.spacing_mut().interact_size.y = TOUCH_TARGET;
                if self.settings_open {
                    egui::ScrollArea::vertical()
                        .id_salt("settings")
                        .show(ui, |ui| {
                            if ui.button("←").on_hover_text("戻る").clicked() {
                                self.go_home();
                            }
                            self.settings_ui(ui);
                        });
                } else {
                    let progress = ctx.animate_bool_with_time_and_easing(
                        egui::Id::new("drawer-transition"),
                        self.drawer_open,
                        0.22,
                        egui::emath::easing::cubic_out,
                    );
                    let mut viewport = ui.available_rect_before_wrap();
                    if self.drawer_open {
                        let keyboard = self.platform.keyboard_inset() / ctx.pixels_per_point();
                        viewport.max.y = (viewport.max.y - (keyboard - insets[3]).max(0.0))
                            .max(viewport.min.y + TOUCH_TARGET + 32.0);
                    }
                    if progress < 1.0 {
                        let blank = ui.interact(
                            viewport,
                            ui.id().with("home-background"),
                            egui::Sense::click(),
                        );
                        if !self.drawer_open
                            && (blank.secondary_clicked()
                                || (blank.hovered() && mobile::take_long_press(ctx, viewport)))
                        {
                            self.settings_open = true;
                        }
                        ui.scope_builder(egui::UiBuilder::new().max_rect(viewport), |ui| {
                            if self.drawer_open {
                                ui.disable();
                            }
                            self.home_height = viewport.height();
                            self.home(ui);
                        });
                    }
                    if progress > 0.0 {
                        let drawer_rect = viewport
                            .translate(Vec2::new(0.0, viewport.height() * (1.0 - progress)));
                        ui.painter().rect_filled(
                            viewport,
                            0.0,
                            Color32::from_black_alpha((progress * 40.0) as u8),
                        );
                        ui.painter()
                            .rect_filled(drawer_rect, 0.0, ui.visuals().panel_fill);
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .id_salt("drawer")
                                .max_rect(drawer_rect),
                            |ui| {
                                ui.set_clip_rect(viewport);
                                self.drawer(ui, progress >= 1.0);
                            },
                        );
                    } else {
                        self.scroll_offset = 0.0;
                        self.scrolling = false;
                    }
                }
                if (!self.drawer_open || self.settings_open || self.app_menu.is_some())
                    && self.search_field.take().is_some()
                {
                    self.platform.set_search_field(None);
                }
                if self.drawer_open && !self.settings_open && self.app_menu.is_none() {
                    ctx.request_repaint_after(Duration::from_millis(50));
                    if self
                        .environment
                        .clock
                        .monotonic()
                        .saturating_sub(self.last_search_poll)
                        >= Duration::from_millis(50)
                    {
                        self.platform.poll_search_field();
                        self.last_search_poll = self.environment.clock.monotonic();
                    }
                }
            });
        if let Some(id) = self.app_menu.clone() {
            let label = self
                .app_indices
                .get(&id)
                .map(|&i| self.apps[i].label.clone())
                .unwrap_or_default();
            let mut open = true;
            let menu_window = egui::Window::new(label)
                .id(egui::Id::new("app-actions"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .open(&mut open)
                .show(ctx, |ui| {
                    if ui
                        .button(if self.favorite_ids.contains(&id) {
                            "ホームから外す"
                        } else {
                            "ホームに追加"
                        })
                        .clicked()
                    {
                        self.settings.toggle_favorite(&id);
                        self.save();
                        self.app_menu = None;
                    }
                    if ui.button("アプリを非表示").clicked() {
                        self.settings.set_hidden(&id, true);
                        self.save();
                        self.app_menu = None;
                    }
                    if ui.button("読みを設定").clicked() {
                        self.pending_reading = Some(id.clone());
                        self.action(
                            self.platform.request_text(
                                &format!("reading:{id}"),
                                "検索の読み",
                                self.settings
                                    .search_readings
                                    .get(&id)
                                    .map(String::as_str)
                                    .unwrap_or_default(),
                            ),
                        );
                        self.app_menu = None;
                    }
                });
            let outside = menu_was_open
                && ctx.input(|i| {
                    i.pointer.primary_clicked()
                        && i.pointer.interact_pos().is_some_and(|p| {
                            menu_window
                                .as_ref()
                                .is_some_and(|w| !w.response.rect.contains(p))
                        })
                });
            if !menu_was_open {
                ctx.request_discard("new app menu layout");
            }
            if !open || outside {
                self.app_menu = None;
            }
        }
    }
}

impl eframe::App for Launcher {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        let color = if self.settings.dark {
            Color32::from_rgb(24, 26, 23)
        } else {
            Color32::from_rgb(244, 241, 233)
        };
        color.to_normalized_gamma_f32()
    }
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.touch_input.prepare(ctx, input);
    }
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.render(ctx);
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_launcher(ctx: &egui::Context, apps: Vec<AppEntry>) -> Launcher {
        let mut launcher = Launcher::from_context(
            ctx,
            Arc::new(crate::platform::desktop::Desktop),
            std::env::temp_dir().join("quiet-headless-unused/settings.json"),
        );
        // Keep asynchronous Android refreshes out of deterministic input tests.
        launcher.rx = mpsc::channel().1;
        launcher.refreshing = false;
        launcher.replace_apps(apps);
        launcher
    }

    fn phone_frame(
        launcher: &mut Launcher,
        ctx: &egui::Context,
        size: Vec2,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        launcher.touch_input.prepare(ctx, &mut input);
        ctx.run(input, |ctx| launcher.render(ctx))
    }

    fn many_apps(count: usize) -> Vec<AppEntry> {
        (0..count)
            .map(|index| AppEntry {
                id: format!("test/{index}"),
                label: format!("長い名前のアプリケーションを画面幅に合わせて表示する {index:04}"),
            })
            .collect()
    }

    fn frame(launcher: &mut Launcher, ctx: &egui::Context) -> egui::FullOutput {
        phone_frame(launcher, ctx, Vec2::new(420.0, 860.0), vec![])
    }

    fn rendered_text(output: &egui::FullOutput) -> Vec<(String, f32)> {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.text().to_owned(), text.pos.y)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn drawer_has_inline_search_and_no_navigation_headings() {
        let ctx = egui::Context::default();
        let mut launcher = Launcher::from_context(
            &ctx,
            Arc::new(crate::platform::desktop::Desktop),
            std::env::temp_dir().join("quiet-headless-unused/settings.json"),
        );
        launcher.replace_apps(launcher.platform.apps().unwrap());
        let first = frame(&mut launcher, &ctx);
        assert!(rendered_text(&first)
            .iter()
            .any(|(text, _)| text.contains("年") && text.contains("月")));
        launcher.open_drawer();
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        let drawer = frame(&mut launcher, &ctx);
        let text = rendered_text(&drawer);
        assert!(
            text.iter().any(|(text, y)| text.is_empty() && *y < 100.0),
            "{text:?}"
        );
        assert!(!text
            .iter()
            .any(|(text, _)| matches!(text.as_str(), "ホーム" | "アプリ" | "↑ アプリ")));
        assert!(text.iter().any(|(text, y)| text == "電話" && *y < 200.0));
        launcher.query = "does not exist".into();
        launcher.refreshing = false;
        let empty = frame(&mut launcher, &ctx);
        assert!(rendered_text(&empty)
            .iter()
            .any(|(text, _)| text == "一致するアプリはありません"));
    }

    fn swipe(launcher: &mut Launcher, ctx: &egui::Context, start: egui::Pos2, end: egui::Pos2) {
        let _ = phone_frame(
            launcher,
            ctx,
            Vec2::new(420.0, 860.0),
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for step in 1..=8 {
            let pos = start.lerp(end, step as f32 / 8.0);
            let _ = phone_frame(
                launcher,
                ctx,
                Vec2::new(420.0, 860.0),
                vec![egui::Event::PointerMoved(pos)],
            );
        }
        let _ = phone_frame(
            launcher,
            ctx,
            Vec2::new(420.0, 860.0),
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }

    #[test]
    fn swipe_opens_a_separate_drawer_and_pull_at_top_returns_home() {
        let ctx = egui::Context::default();
        let mut launcher = test_launcher(&ctx, many_apps(200));
        let _ = frame(&mut launcher, &ctx);
        swipe(
            &mut launcher,
            &ctx,
            egui::pos2(180.0, 650.0),
            egui::pos2(180.0, 470.0),
        );
        assert!(launcher.drawer_open);
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        let output = frame(&mut launcher, &ctx);
        assert!(!rendered_text(&output)
            .iter()
            .any(|(text, _)| text.contains("年") && text.contains("月")));
        assert!(
            launcher.error.is_none(),
            "opening swipe must not launch an app"
        );
        launcher.query = "no matching apps".into();
        for _ in 0..4 {
            let _ = frame(&mut launcher, &ctx);
        }
        swipe(
            &mut launcher,
            &ctx,
            egui::pos2(180.0, 250.0),
            egui::pos2(180.0, 450.0),
        );
        assert!(!launcher.drawer_open);
        assert!(launcher.query.is_empty());
    }

    #[test]
    fn pulling_a_scrolled_list_does_not_dismiss_the_drawer() {
        let ctx = egui::Context::default();
        let mut launcher = test_launcher(&ctx, many_apps(200));
        launcher.open_drawer();
        let _ = frame(&mut launcher, &ctx);
        let _ = phone_frame(
            &mut launcher,
            &ctx,
            Vec2::new(420.0, 860.0),
            vec![
                egui::Event::PointerMoved(egui::pos2(180.0, 500.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -1000.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        assert!(launcher.scroll_offset > 100.0);
        swipe(
            &mut launcher,
            &ctx,
            egui::pos2(180.0, 250.0),
            egui::pos2(180.0, 450.0),
        );
        assert!(
            launcher.drawer_open,
            "scrolling toward the start must keep the drawer open"
        );
        assert!(launcher.error.is_none());
    }

    #[test]
    fn scrolling_does_not_repeat_search_or_layout_thousands_of_rows() {
        struct CountingSearch(Arc<AtomicUsize>);
        impl AppSearch for CountingSearch {
            fn search<'a>(&self, query: &str, apps: &'a [AppEntry]) -> Vec<&'a AppEntry> {
                self.0.fetch_add(1, Ordering::Relaxed);
                ExactSearch.search(query, apps)
            }
        }
        let ctx = egui::Context::default();
        let mut launcher = test_launcher(&ctx, many_apps(10_000));
        let searches = Arc::new(AtomicUsize::new(0));
        launcher.set_search_engine(Box::new(CountingSearch(searches.clone())));
        launcher.open_drawer();
        for _ in 0..60 {
            let _ = frame(&mut launcher, &ctx);
            assert!(launcher.rendered_rows <= 18);
        }
        assert_eq!(searches.load(Ordering::Relaxed), 1);
        launcher.query = "not installed".into();
        let _ = frame(&mut launcher, &ctx);
        assert_eq!(searches.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn filtering_after_scrolling_reveals_the_matching_row_below_search() {
        let ctx = egui::Context::default();
        let mut launcher = test_launcher(&ctx, many_apps(200));
        launcher.open_drawer();
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        let _ = phone_frame(
            &mut launcher,
            &ctx,
            Vec2::new(420.0, 860.0),
            vec![
                egui::Event::PointerMoved(egui::pos2(160.0, 500.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -4000.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        assert!(launcher.scroll_offset > 1000.0);
        launcher.query = launcher.apps[0].label.clone();
        for _ in 0..4 {
            let _ = frame(&mut launcher, &ctx);
        }
        let output = frame(&mut launcher, &ctx);
        assert_eq!(launcher.results.len(), 1);
        let body = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text
                        .galley
                        .job
                        .sections
                        .iter()
                        .any(|section| section.format.font_id.size == 22.0) =>
                {
                    Some((shape.clip_rect, text))
                }
                _ => None,
            })
            .expect("matching app must be painted below the pinned search header");
        assert!(
            body.1.pos.y >= body.0.top(),
            "matching row must not be covered by search"
        );
    }

    #[test]
    fn phone_widths_keep_home_drawer_and_settings_text_inside_screen() {
        for size in [
            Vec2::new(240.0, 640.0),
            Vec2::new(320.0, 640.0),
            Vec2::new(360.0, 800.0),
            Vec2::new(640.0, 360.0),
        ] {
            let ctx = egui::Context::default();
            let apps = many_apps(200);
            let favorite = apps[0].id.clone();
            let mut launcher = test_launcher(&ctx, apps);
            launcher.settings.favorites = vec![favorite];
            launcher.widgets = vec![TextWidget {
                id: 7,
                title: "予定表".into(),
                lines: vec!["長い情報を表示するための文字列です。".repeat(8)],
                pending: false,
            }];
            for mode in 0..3 {
                launcher.settings_open = mode == 2;
                launcher.drawer_open = mode == 1;
                for _ in 0..25 {
                    let _ = phone_frame(&mut launcher, &ctx, size, vec![]);
                }
                let output = phone_frame(&mut launcher, &ctx, size, vec![]);
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape {
                        let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                        if bounds.intersects(shape.clip_rect) {
                            assert!(
                                bounds.left() >= -1.0 && bounds.right() <= size.x + 1.0,
                                "{size:?} mode {mode}: {:?} {bounds:?}",
                                text.galley.text()
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn touch_drag_moves_list_and_keeps_inertia_without_launching_an_app() {
        let ctx = egui::Context::default();
        let mut launcher = test_launcher(&ctx, many_apps(200));
        launcher.open_drawer();
        for _ in 0..30 {
            let _ = frame(&mut launcher, &ctx);
        }
        let before = launcher.scroll_offset;
        let size = Vec2::new(420.0, 860.0);
        let mut pos = egui::pos2(160.0, 600.0);
        let touch = |phase, pos| egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(1),
            phase,
            pos,
            force: None,
        };
        let _ = phone_frame(
            &mut launcher,
            &ctx,
            size,
            vec![
                touch(egui::TouchPhase::Start, pos),
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..8 {
            pos.y -= 24.0;
            let _ = phone_frame(
                &mut launcher,
                &ctx,
                size,
                vec![
                    touch(egui::TouchPhase::Move, pos),
                    egui::Event::PointerMoved(pos),
                ],
            );
        }
        let _ = phone_frame(
            &mut launcher,
            &ctx,
            size,
            vec![
                touch(egui::TouchPhase::End, pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerGone,
            ],
        );
        let released = launcher.scroll_offset;
        assert!(released > before + 100.0, "{before} -> {released}");
        for _ in 0..8 {
            let _ = phone_frame(&mut launcher, &ctx, size, vec![]);
        }
        assert!(
            launcher.scroll_offset > released,
            "fling must continue after releasing the finger"
        );
        assert!(
            launcher.error.is_none(),
            "dragging must not invoke Desktop::launch"
        );
        let output = phone_frame(&mut launcher, &ctx, size, vec![]);
        assert!(
            rendered_text(&output)
                .iter()
                .any(|(text, y)| text.is_empty() && *y < 100.0),
            "search stays visible while the list scrolls"
        );
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "/system/fonts/NotoSansCJK-Regular.ttc",
        "/system/fonts/NotoSansCJK-VF.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/mnt/c/Windows/Fonts/msgothic.ttc",
        "C:/Windows/Fonts/msgothic.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    ];
    let data = candidates
        .into_iter()
        .find_map(|path| std::fs::read(path).ok());
    if let Some(data) = data {
        fonts.font_data.insert(
            "japanese".into(),
            Arc::new(egui::FontData::from_owned(data)),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .push("japanese".into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("japanese".into());
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(16.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(16.0));
    style.interaction.selectable_labels = false;
    style.wrap_mode = Some(egui::TextWrapMode::Wrap);
    style.spacing.interact_size.y = TOUCH_TARGET;
    ctx.set_style(style);
}
