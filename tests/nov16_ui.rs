#![cfg(feature = "ui")]
use eframe::egui::{self, Pos2, Vec2};
use quiet_launcher::{
    catalog::AppCatalog, config::Settings, model::*, platform::Platform, ui::Launcher,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
struct Phone {
    enumerations: AtomicUsize,
}
impl Platform for Phone {
    fn apps(&self) -> anyhow::Result<Vec<AppEntry>> {
        self.enumerations.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("unchanged catalog must not be enumerated")
    }
    fn catalog_revision(&self) -> anyhow::Result<Option<String>> {
        Ok(Some("known".into()))
    }
    fn snapshot(&self) -> anyhow::Result<DeviceSnapshot> {
        Ok(DeviceSnapshot::default())
    }
    fn launch(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("unexpected launch")
    }
    fn settings(&self, _: SystemSettings) -> anyhow::Result<()> {
        Ok(())
    }
    fn media(&self, _: MediaCommand) -> anyhow::Result<()> {
        Ok(())
    }
    fn widget_providers(&self) -> anyhow::Result<Vec<WidgetProvider>> {
        Ok(vec![])
    }
    fn add_widget(&self, _: &str) -> anyhow::Result<i32> {
        Ok(7)
    }
    fn remove_widget(&self, _: i32) -> anyhow::Result<()> {
        Ok(())
    }
    fn poll_widgets(&self, _: &[i32], _: &WidgetNames) {}
    fn widgets(&self) -> Vec<TextWidget> {
        vec![]
    }
    fn request_text(&self, _: &str, _: &str, _: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn take_text(&self) -> Option<(String, String)> {
        None
    }
}
fn setup(
    mut settings: Settings,
    apps: Vec<AppEntry>,
) -> (Launcher, egui::Context, Arc<Phone>, PathBuf) {
    static N: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir()
        .join(format!(
            "quiet-ui-contract-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ))
        .join("settings.json");
    settings.show_weather = false;
    settings.show_usage = false;
    settings.show_media = false;
    settings.save(&path).unwrap();
    let mut catalog = AppCatalog::default();
    catalog.refresh("known", || Ok(apps)).unwrap();
    catalog.save(&path.with_extension("apps.json")).unwrap();
    let phone = Arc::new(Phone {
        enumerations: AtomicUsize::new(0),
    });
    let ctx = egui::Context::default();
    let launcher = Launcher::from_context(&ctx, phone.clone(), path.clone());
    (launcher, ctx, phone, path)
}
fn frame(app: &mut Launcher, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(360.0, 640.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.render(ctx),
    )
}
fn texts(out: &egui::FullOutput) -> Vec<String> {
    out.shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(t) => Some(t.galley.text().into()),
            _ => None,
        })
        .collect()
}
fn swipe(app: &mut Launcher, ctx: &egui::Context) {
    let start = Pos2::new(160.0, 530.0);
    frame(
        app,
        ctx,
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
    for i in 1..=8 {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerMoved(
                start - Vec2::new(0.0, 25.0 * i as f32),
            )],
        );
    }
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: start - Vec2::new(0.0, 200.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    for _ in 0..40 {
        frame(app, ctx, vec![]);
    }
}
#[test]
fn cached_favorite_is_visible_in_first_frame_without_waiting_for_android_enumeration() {
    let mut s = Settings::default();
    s.favorites.push("a/Main".into());
    let (mut app, ctx, phone, _) = setup(
        s,
        vec![AppEntry {
            id: "a/Main".into(),
            label: "ALPHA".into(),
        }],
    );
    assert!(texts(&frame(&mut app, &ctx, vec![]))
        .iter()
        .any(|t| t == "ALPHA"));
    std::thread::sleep(Duration::from_millis(80));
    frame(&mut app, &ctx, vec![]);
    assert_eq!(phone.enumerations.load(Ordering::SeqCst), 0);
}
#[test]
fn configured_font_size_is_used_for_home_app_names() {
    let mut s = Settings::default();
    s.font_percent = 140;
    s.favorites.push("a/Main".into());
    let (mut app, ctx, _, _) = setup(
        s,
        vec![AppEntry {
            id: "a/Main".into(),
            label: "ALPHA".into(),
        }],
    );
    let out = frame(&mut app, &ctx, vec![]);
    let size = out
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "ALPHA" => {
                Some(t.galley.job.sections[0].format.font_id.size)
            }
            _ => None,
        })
        .unwrap();
    assert!((size - 37.8).abs() < 0.1);
}
#[test]
fn overflow_is_an_error_without_deleting_or_silently_displaying_a_subset() {
    let apps: Vec<AppEntry> = (0..20)
        .map(|n| AppEntry {
            id: format!("a/{n}"),
            label: format!("FAVORITE{n}"),
        })
        .collect();
    let mut s = Settings::default();
    s.favorites = apps.iter().map(|a| a.id.clone()).collect();
    let (mut app, ctx, _, path) = setup(s, apps);
    let out = frame(&mut app, &ctx, vec![]);
    let labels = texts(&out);
    assert!(
        labels.iter().any(|t| t.contains("収まりません")),
        "{labels:?}"
    );
    assert!(!labels.iter().any(|t| t.starts_with("FAVORITE")));
    assert_eq!(Settings::load(&path).unwrap().favorites.len(), 20);
}
#[test]
fn default_drawer_search_accepts_romaji_and_escape_always_returns_home() {
    let apps = vec![
        AppEntry {
            id: "a/Main".into(),
            label: "カメラ".into(),
        },
        AppEntry {
            id: "b/Main".into(),
            label: "OTHER".into(),
        },
    ];
    let (mut app, ctx, _, _) = setup(Settings::default(), apps);
    frame(&mut app, &ctx, vec![]);
    swipe(&mut app, &ctx);
    frame(&mut app, &ctx, vec![egui::Event::Text("kamera".into())]);
    let labels = texts(&frame(&mut app, &ctx, vec![]));
    assert!(labels.iter().any(|t| t == "カメラ"));
    assert!(!labels.iter().any(|t| t == "OTHER"), "{labels:?}");
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    for _ in 0..40 {
        frame(&mut app, &ctx, vec![]);
    }
    let labels = texts(&frame(&mut app, &ctx, vec![]));
    assert!(!labels.iter().any(|t| t == "kamera" || t == "OTHER"));
    swipe(&mut app, &ctx);
    let labels = texts(&frame(&mut app, &ctx, vec![]));
    assert!(
        labels.iter().any(|t| t == "OTHER"),
        "query must be cleared: {labels:?}"
    );
}
