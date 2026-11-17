//! Bounded touch controls and row virtualization shared by phone screens.
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::ops::Range;

pub const ROW_HEIGHT: f32 = 56.0;
pub const TOUCH_TARGET: f32 = 48.0;

#[derive(Default)]
pub struct TouchInput {
    clear_pointer_next_frame: bool,
    fingers: std::collections::HashSet<u64>,
    origin: Option<(Pos2, f64)>,
    blocked: bool,
    long_fired: bool,
}

impl TouchInput {
    /// egui-winit 0.31 emits release and PointerGone together. In egui 0.31,
    /// PointerGone clears position history before ScrollArea samples release
    /// velocity. Delay only the disappearance by one frame so native touch
    /// flings retain momentum. The release event itself is never delayed.
    pub fn prepare(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        ctx.data_mut(|d| d.remove::<Pos2>(egui::Id::new("touch-long-press")));
        let now = input.time.unwrap_or_else(|| ctx.input(|i| i.time));
        let mut ended = false;
        for event in &input.events {
            if let egui::Event::Touch { id, pos, phase, .. } = event {
                match phase {
                    egui::TouchPhase::Start => {
                        if self.fingers.is_empty() {
                            self.origin = Some((*pos, now));
                            self.blocked = false;
                            self.long_fired = false;
                        }
                        self.fingers.insert(id.0);
                        if self.fingers.len() > 1 {
                            self.blocked = true;
                        }
                    }
                    egui::TouchPhase::Move => {
                        if self
                            .origin
                            .is_some_and(|(start, _)| start.distance(*pos) >= 10.)
                        {
                            self.blocked = true;
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        if *phase == egui::TouchPhase::Cancel {
                            self.blocked = true;
                        }
                        self.fingers.remove(&id.0);
                        ended = true;
                    }
                }
            }
        }
        if self.fingers.len() == 1 && !self.blocked && !self.long_fired {
            if let Some((pos, start)) = self.origin {
                if now - start + 1e-9 >= 0.5 {
                    self.long_fired = true;
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("touch-long-press"), pos));
                }
            }
        }
        let suppress = (self.blocked || self.long_fired) && (!self.fingers.is_empty() || ended);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("touch-suppress-click"), suppress));
        if std::mem::take(&mut self.clear_pointer_next_frame) {
            input.events.insert(0, egui::Event::PointerGone);
        }
        let touch_ended = input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Touch {
                    phase: egui::TouchPhase::End,
                    ..
                }
            )
        });
        let cancelled = input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                }
            )
        });
        if touch_ended
            && !cancelled
            && input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerGone))
        {
            input
                .events
                .retain(|event| !matches!(event, egui::Event::PointerGone));
            self.clear_pointer_next_frame = true;
            ctx.request_repaint();
        }
    }
}

pub fn visible_rows(list: Rect, clip: Rect, count: usize) -> Range<usize> {
    if count == 0 || !list.intersects(clip) {
        return 0..0;
    }
    let first = ((clip.top() - list.top()).max(0.0) / ROW_HEIGHT).floor() as usize;
    let last = ((clip.bottom() - list.top()).max(0.0) / ROW_HEIGHT).ceil() as usize;
    first.saturating_sub(1).min(count)..last.saturating_add(1).min(count)
}

/// Unlike Button::min_size, this control cannot expand when a label is long.
/// Click-only sensing leaves vertical dragging to the parent ScrollArea.
pub fn text_button(ui: &mut Ui, label: &str, size: Vec2, font_size: f32) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    suppress_touch_click(&mut response);
    paint_button(ui, rect, &response, label, font_size);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response
}

pub fn paint_button(ui: &Ui, rect: Rect, response: &Response, label: &str, font_size: f32) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let visuals = ui.style().interact(response);
    if response.is_pointer_button_down_on() || response.has_focus() {
        ui.painter().rect_filled(rect, 6.0, visuals.weak_bg_fill);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.to_owned(),
        FontId::proportional(font_size),
        ui.visuals().text_color(),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: (rect.width() - 8.0).max(0.0),
        max_rows: 1,
        break_anywhere: true,
        ..Default::default()
    };
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let pos = Align2::LEFT_CENTER
        .align_size_within_rect(galley.size(), rect.shrink2(Vec2::new(4.0, 0.0)))
        .min;
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(pos, galley, ui.visuals().text_color());
}

pub fn app_row(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    label: &str,
    favorite: bool,
) -> (bool, bool, bool) {
    app_row_with_identity(ui, rect, id, label, favorite, None)
}

pub fn app_row_with_identity(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    label: &str,
    favorite: bool,
    identity: Option<&str>,
) -> (bool, bool, bool) {
    let action = Rect::from_min_max(
        Pos2::new((rect.right() - TOUCH_TARGET).max(rect.left()), rect.top()),
        rect.max,
    );
    let name = Rect::from_min_max(
        rect.min,
        Pos2::new((action.left() - 8.0).max(rect.left()), rect.bottom()),
    );
    let mut response = ui.interact(name, ui.id().with((id, "launch")), Sense::click());
    suppress_touch_click(&mut response);
    let long_press = take_long_press(ui.ctx(), name);
    let font_size = 22.0 * ui.style().text_styles[&egui::TextStyle::Body].size / 16.0;
    let label_rect = if identity.is_some() {
        Rect::from_min_max(name.min, Pos2::new(name.right(), name.bottom() - 16.))
    } else {
        name
    };
    paint_button(ui, label_rect, &response, label, font_size);
    if let Some(identity) = identity {
        let meta = Rect::from_min_max(Pos2::new(name.left(), name.bottom() - 16.), name.max);
        paint_button(ui, meta, &response, identity, 10.);
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let mut toggle = ui.interact(action, ui.id().with((id, "favorite")), Sense::click());
    suppress_touch_click(&mut toggle);
    let symbol = if favorite { "−" } else { "+" };
    if toggle.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(action, 6.0, ui.visuals().widgets.active.weak_bg_fill);
    }
    ui.painter().text(
        action.center(),
        Align2::CENTER_CENTER,
        symbol,
        FontId::proportional(22.0),
        ui.visuals().weak_text_color(),
    );
    toggle.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            if favorite {
                "ホームから外す"
            } else {
                "ホームに追加"
            },
        )
    });
    (
        response.clicked(),
        toggle.clicked(),
        response.secondary_clicked() || long_press,
    )
}

fn suppress_touch_click(response: &mut Response) {
    if response.ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new("touch-suppress-click"))
            .unwrap_or(false)
    }) {
        response
            .flags
            .remove(egui::response::Flags::CLICKED | egui::response::Flags::FAKE_PRIMARY_CLICKED);
    }
}
pub fn take_long_press(ctx: &egui::Context, rect: Rect) -> bool {
    let key = egui::Id::new("touch-long-press");
    if ctx
        .data(|d| d.get_temp::<Pos2>(key))
        .is_some_and(|p| rect.contains(p))
    {
        ctx.data_mut(|d| d.remove::<Pos2>(key));
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thousands_of_apps_draw_only_the_viewport() {
        let list = Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 10_000.0 * ROW_HEIGHT));
        let clip = Rect::from_min_size(Pos2::new(0.0, 140_000.0), Vec2::new(320.0, 640.0));
        let visible = visible_rows(list, clip, 10_000);
        assert!(visible.len() <= 15);
        assert!(visible.contains(&2500));
        assert!(visible_rows(list, clip.translate(Vec2::new(400.0, 0.0)), 10_000).is_empty());
    }
}
