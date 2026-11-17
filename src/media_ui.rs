//! Bounded media panel. Icons are vector geometry; labels are accessibility metadata only.
use crate::{
    media::{supports, MediaController},
    model::{MediaAction, Playback},
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub fn panel(ui: &mut egui::Ui, media: &mut MediaController, scale: f32) {
    let Some(session) = media.selected().cloned() else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 84.), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let info_height = 36.;
    let header = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + info_height));
    ui.scope_builder(egui::UiBuilder::new().max_rect(header), |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let state = match session.state {
            Playback::Playing => "再生中",
            Playback::Paused => "一時停止中",
            _ => "接続待機中",
        };
        let label = if scale > 1. {
            format!(
                "{} · {} · {}",
                session.provider,
                session.title.as_deref().unwrap_or(""),
                state
            )
        } else {
            session.provider.clone()
        };
        info_line(
            ui,
            &label,
            Vec2::new(header.width(), if scale > 1. { 36. } else { 18. }),
            12. * scale,
        );
        if scale <= 1. {
            info_line(
                ui,
                &format!("{} · {}", session.title.as_deref().unwrap_or(""), state),
                Vec2::new(header.width(), 18.),
                12.,
            );
        }
    });
    let (index, count) = media.position();
    let indicator = ui.interact(
        Rect::from_min_size(rect.min, Vec2::new(1., 1.)),
        ui.id().with("media-position"),
        Sense::hover(),
    );
    indicator.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            true,
            format!("メディア {} / {}", index + 1, count),
        )
    });
    if count > 1 {
        let dots = (0..count)
            .map(|i| if i == index { "●" } else { "○" })
            .collect::<String>();
        let pos = Pos2::new(rect.right() - 4., rect.top() + 28.);
        painter.text(
            pos,
            egui::Align2::RIGHT_CENTER,
            dots,
            egui::FontId::proportional(10.),
            ui.visuals().text_color(),
        );
    }
    let gesture = ui.id().with("media-swipe-origin");
    let (pressed, released, position, cancelled) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_released(),
            i.pointer.interact_pos(),
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Touch {
                        phase: egui::TouchPhase::Cancel,
                        ..
                    }
                )
            }),
        )
    });
    if cancelled {
        ui.data_mut(|d| d.remove::<Pos2>(gesture));
    }
    if pressed {
        if let Some(p) = position.filter(|p| rect.contains(*p)) {
            ui.data_mut(|d| d.insert_temp(gesture, p));
        }
    }
    let delta = ui
        .data(|d| d.get_temp::<Pos2>(gesture))
        .zip(position)
        .map(|(start, end)| (start, end - start));
    let swiping = delta.is_some_and(|(_, d)| d.x.abs() >= 10.);
    if released {
        ui.data_mut(|d| d.remove::<Pos2>(gesture));
        if let Some((_, d)) = delta {
            if !cancelled && d.x.abs() >= 48. && d.x.abs() >= d.y.abs() * 1.5 {
                media.switch(d.x < 0.);
            }
        }
    }
    let action = match session.state {
        Playback::Playing => Some(MediaAction::Pause),
        Playback::Paused => Some(MediaAction::Play),
        _ => None,
    };
    for (i, action) in [Some(MediaAction::Previous), action, Some(MediaAction::Next)]
        .into_iter()
        .enumerate()
    {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + i as f32 * 48., rect.top() + 36.),
            Vec2::splat(48.),
        );
        let Some(action) = action else {
            painter.circle_stroke(
                r.center(),
                8.,
                Stroke::new(2_f32, ui.visuals().weak_text_color()),
            );
            continue;
        };
        let enabled = supports(&session, action);
        let response = ui.interact(
            r,
            ui.id().with(("media", &session.id, i)),
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let label = match action {
            MediaAction::Previous => "前の曲",
            MediaAction::Play => "再生",
            MediaAction::Pause => "一時停止",
            MediaAction::Next => "次の曲",
        };
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        if response.has_focus() {
            painter.rect_stroke(
                r.shrink(2.),
                4.,
                Stroke::new(1_f32, ui.visuals().text_color()),
                egui::StrokeKind::Inside,
            );
        }
        icon(
            &painter,
            r,
            action,
            if enabled {
                ui.visuals().text_color()
            } else {
                ui.visuals().weak_text_color()
            },
        );
        if enabled && response.clicked() && !swiping {
            media.control(&session.id, action);
        }
    }
}
fn icon(p: &egui::Painter, r: Rect, action: MediaAction, color: Color32) {
    let c = r.center();
    if action == MediaAction::Pause {
        for x in [-5., 3.] {
            p.rect_filled(
                Rect::from_min_size(c + Vec2::new(x, -8.), Vec2::new(3., 16.)),
                0.,
                color,
            );
        }
    } else {
        let direction = if action == MediaAction::Previous {
            -1.
        } else {
            1.
        };
        let points = vec![
            c + Vec2::new(-6. * direction, -8.),
            c + Vec2::new(-6. * direction, 8.),
            c + Vec2::new(7. * direction, 0.),
        ];
        p.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        if action != MediaAction::Play {
            p.line_segment(
                [
                    c + Vec2::new(9. * direction, -8.),
                    c + Vec2::new(9. * direction, 8.),
                ],
                Stroke::new(2_f32, color),
            );
        }
    }
}

fn info_line(ui: &mut egui::Ui, label: &str, size: Vec2, font_size: f32) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    crate::mobile::paint_button(ui, rect, &response, label, font_size);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
}
