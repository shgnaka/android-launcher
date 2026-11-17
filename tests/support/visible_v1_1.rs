use eframe::egui;
pub fn fully_visible(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn collect(shape: &egui::Shape, clip: egui::Rect, v: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => {
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                if clip.contains_rect(rect) {
                    v.push((t.galley.text().into(), rect));
                }
            }
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    collect(s, clip, v)
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    for s in &output.shapes {
        collect(&s.shape, s.clip_rect, &mut out);
    }
    out
}
