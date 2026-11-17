#[derive(Clone, Copy)]
pub struct HomeLayout {
    pub height: f32,
    pub font_percent: u16,
    pub favorites: usize,
    pub widgets: usize,
    pub weather: bool,
    pub usage: bool,
    pub media: bool,
}
impl HomeLayout {
    pub fn required_height(&self) -> f32 {
        let scale = self.font_percent as f32 / 100.0;
        144.0 * scale
            + (self.weather as u8 as f32 + self.usage as u8 as f32) * 24.0 * scale
            + if self.media { 84.0 } else { 0.0 }
            + self.widgets as f32 * 48.0 * scale
            + if self.favorites == 0 {
                48.0
            } else {
                self.favorites as f32 * (48.0f32.max(32.0 * scale) + 8.0)
            }
    }
    pub fn fits(&self) -> bool {
        (80..=160).contains(&self.font_percent) && self.height >= self.required_height()
    }
}
