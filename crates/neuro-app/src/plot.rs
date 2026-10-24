//! Normalized fractions on a zero-inclusive ln(1+x) axis.
use crate::{experiment::number, theme::*};
use bevy_egui::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke};

pub struct Series {
    pub points: Vec<(f64, f64)>,
    pub color: egui::Color32,
    pub dashed: bool,
}
pub fn show(
    ui: &mut egui::Ui,
    max: f64,
    labels: (&str, &str),
    series: &[Series],
    current: (f64, f64),
    sample: impl Fn(f64) -> Option<(f64, String)>,
) {
    let (outer, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 210.0), Sense::hover());
    let rect = Rect::from_min_max(
        outer.min + egui::vec2(44.0, 24.0),
        outer.max - egui::vec2(16.0, 45.0),
    );
    let painter = ui.painter_at(outer);
    let end = max.ln_1p();
    let at = |x: f64, y: f64| {
        Pos2::new(
            rect.left() + (x.ln_1p() / end) as f32 * rect.width(),
            rect.bottom() - y as f32 * rect.height(),
        )
    };
    painter.text(
        egui::pos2(rect.left(), outer.top() + 2.0),
        Align2::LEFT_TOP,
        labels.1,
        FontId::proportional(12.0),
        MUTED,
    );
    for i in 0..=4 {
        let fraction = i as f64 / 4.0;
        let y = at(0.0, fraction).y;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0_f32, BORDER),
        );
        painter.text(
            egui::pos2(rect.left() - 9.0, y),
            Align2::RIGHT_CENTER,
            format!("{fraction:.2}"),
            FontId::proportional(11.0),
            MUTED,
        );
        let x = if i == 4 {
            max
        } else {
            (end * fraction).exp_m1().min(max)
        };
        let px = at(x, 0.0).x;
        painter.line_segment(
            [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
            Stroke::new(1.0_f32, BORDER.gamma_multiply(0.5)),
        );
        painter.text(
            egui::pos2(px, rect.bottom() + 10.0),
            Align2::CENTER_TOP,
            number(x),
            FontId::proportional(11.0),
            MUTED,
        );
    }
    painter.text(
        egui::pos2(rect.center().x, outer.bottom() - 2.0),
        Align2::CENTER_BOTTOM,
        labels.0,
        FontId::proportional(11.0),
        MUTED,
    );
    for s in series {
        if s.dashed {
            for (i, pair) in s.points.windows(2).enumerate() {
                if i % 4 < 2 {
                    painter.line_segment(
                        [at(pair[0].0, pair[0].1), at(pair[1].0, pair[1].1)],
                        Stroke::new(1.6_f32, s.color),
                    );
                }
            }
        } else {
            painter.add(egui::Shape::line(
                s.points.iter().map(|&(x, y)| at(x, y)).collect(),
                Stroke::new(2.5_f32, s.color),
            ));
        }
    }
    if current.0 <= max {
        let point = at(current.0, current.1);
        painter.line_segment(
            [egui::pos2(point.x, rect.bottom()), point],
            Stroke::new(1.0_f32, MUTED.gamma_multiply(0.6)),
        );
        painter.circle_filled(point, 5.5, TEXT);
        painter.circle_filled(point, 3.0, TEAL);
    } else {
        painter.text(
            rect.right_top() + egui::vec2(-8.0, 10.0),
            Align2::RIGHT_TOP,
            "Текущая точка правее диапазона",
            FontId::proportional(11.0),
            ERROR,
        );
    }
    if response.hovered()
        && let Some(pointer) = response.hover_pos()
        && rect.contains(pointer)
    {
        let x = (f64::from((pointer.x - rect.left()) / rect.width()) * end)
            .exp_m1()
            .min(max);
        if let Some((y, text)) = sample(x) {
            painter.circle_filled(at(x, y), 4.0, TEAL);
            response.on_hover_text(text);
        }
    }
}
