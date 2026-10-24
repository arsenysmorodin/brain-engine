use bevy_egui::egui::{self, Color32, FontId, RichText, Stroke};
pub const BACKGROUND: Color32 = Color32::from_rgb(17, 21, 28);
pub const SURFACE: Color32 = Color32::from_rgb(23, 29, 38);
pub const RAISED: Color32 = Color32::from_rgb(31, 39, 50);
pub const BORDER: Color32 = Color32::from_rgb(49, 61, 75);
pub const TEXT: Color32 = Color32::from_rgb(234, 240, 246);
pub const MUTED: Color32 = Color32::from_rgb(160, 175, 191);
pub const TEAL: Color32 = Color32::from_rgb(101, 218, 201);
pub const PURPLE: Color32 = Color32::from_rgb(185, 161, 244);
pub const ERROR: Color32 = Color32::from_rgb(255, 156, 147);

pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Noto Math".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSansMath.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Noto Symbols".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSansSymbols2.ttf"
        ))),
    );
    fonts.font_data.insert(
        "Noto Sans".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/NotoSans.ttf"
        ))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(
            family,
            vec![
                "Noto Sans".into(),
                "Noto Math".into(),
                "Noto Symbols".into(),
            ],
        );
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = SURFACE;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = BACKGROUND;
    style.visuals.faint_bg_color = RAISED;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.hyperlink_color = TEAL;
    style.visuals.selection.bg_fill = Color32::from_rgb(44, 83, 85);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEAL);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.inactive.weak_bg_fill = RAISED;
    style.visuals.widgets.inactive.bg_fill = RAISED;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(43, 59, 69);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, TEAL);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(24.0));
    ctx.set_style(style);
}
pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(MUTED));
}
pub fn tag(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.12))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(9, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(color));
        });
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(10)
        .inner_margin(18)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_fonts_cover_russian_and_scientific_notation() {
        let ctx = egui::Context::default();
        configure(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.fonts_mut(|fonts| {
                let font = FontId::proportional(16.0);
                for symbol in "Кофеин Аденозин Ёё ⇌ → ⊣ ● −".chars() {
                    assert!(fonts.has_glyph(&font, symbol), "Missing glyph: {symbol}");
                }
            });
        });
    }
}
