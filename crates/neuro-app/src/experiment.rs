use crate::{Lab, theme::*};
use bevy_egui::egui::{self, Align2, FontId, Pos2, Rect, RichText, Sense, Stroke};
use neuro_engine::Evaluation;

pub fn show(ui: &mut egui::Ui, lab: &mut Lab) {
    ui.spacing_mut().item_spacing.y = 5.0;
    tag(ui, "Иллюстративная модель · illustrative", PURPLE);
    ui.add_space(4.0);
    muted(
        ui,
        "Два абстрактных лиганда, один сайт. Это не параметры кофеина.",
    );
    ui.add_space(4.0);
    card().inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().interact_size.y = 22.0;
        let before = lab.scenario.clone();
        ui.columns(2, |columns| {
            parameter(
                &mut columns[0],
                "a",
                "Свободная концентрация A / Kd_A",
                &mut lab.draft_a,
                &mut lab.scenario.parameters.a,
                TEAL,
            );
            parameter(
                &mut columns[1],
                "b",
                "Свободная концентрация B / Kd_B",
                &mut lab.draft_b,
                &mut lab.scenario.parameters.b,
                PURPLE,
            );
        });
        lab.apply_drafts();
        if before != lab.scenario {
            lab.dirty = true;
        }
        if let Some(error) = &lab.parameter_error {
            ui.colored_label(ERROR, error);
            muted(ui, "Исправьте ввод для расчёта, сохранения и экспорта.");
        }
    });
    ui.add_space(5.0);
    if lab.parameter_error.is_some() {
        return;
    }
    if let Some(result) = &lab.evaluation {
        ui.columns(3, |columns| {
            for (index, (label, value, color)) in [
                ("Свободные", result.current.p_free, MUTED),
                ("Заняты A", result.current.p_a, TEAL),
                ("Заняты B", result.current.p_b, PURPLE),
            ]
            .into_iter()
            .enumerate()
            {
                card().inner_margin(10).show(&mut columns[index], |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        RichText::new(format!("{:.2}%", value * 100.0))
                            .size(26.0)
                            .color(color),
                    );
                    ui.label(label);
                    ui.label(
                        RichText::new(format!("p = {value:.6e}"))
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new("Занятость рецептора не равна активации. Эффекты на мозг и поведение не вычисляются.").color(MUTED).size(12.0));
        ui.add_space(5.0);
    }
    card().inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Связывание лиганда A").size(18.0));
            ui.label("Максимум a:");
            if ui
                .add(
                    egui::DragValue::new(&mut lab.scenario.plot.max_a)
                        .speed(1.0)
                        .custom_formatter(|value, _| number(value))
                        .range(0.001..=f64::MAX),
                )
                .changed()
            {
                lab.refresh();
                lab.dirty = true;
            }
            if ui.button("Показать текущую точку").clicked() {
                lab.scenario.plot.max_a = lab.scenario.parameters.a.max(10.0);
                lab.refresh();
                lab.dirty = true;
            }
        });
        if let Some(result) = &lab.evaluation {
            plot(ui, result);
        }
    });
}
pub(crate) fn parameter(
    ui: &mut egui::Ui,
    name: &str,
    label: &str,
    draft: &mut String,
    value: &mut f64,
    color: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).size(23.0).color(color));
        ui.add(
            egui::TextEdit::singleline(draft)
                .desired_width(125.0)
                .hint_text("0 … 1.797e308"),
        )
        .on_hover_text("Точный ввод: например, 1e6. Допустимы точка и запятая.");
    });
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    let mut slider_value = value.min(100.0);
    if ui
        .add(
            egui::Slider::new(&mut slider_value, 0.0..=100.0)
                .logarithmic(true)
                .smallest_positive(0.001)
                .show_value(false),
        )
        .changed()
    {
        *draft = slider_value.to_string();
    }
}
fn plot(ui: &mut egui::Ui, result: &Evaluation) {
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(
            TEAL,
            format!("При b = {}", number(result.scenario.parameters.b)),
        );
        ui.colored_label(PURPLE, "Пунктир: b = 0");
        ui.label("●  Текущая точка");
    });
    let (outer, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 220.0), Sense::hover());
    let rect = Rect::from_min_max(
        outer.min + egui::vec2(44.0, 24.0),
        outer.max - egui::vec2(16.0, 45.0),
    );
    let painter = ui.painter_at(outer);
    let max = result.scenario.plot.max_a;
    let end = max.ln_1p();
    let at = |a: f64, p: f64| {
        Pos2::new(
            rect.left() + (a.ln_1p() / end) as f32 * rect.width(),
            rect.bottom() - p as f32 * rect.height(),
        )
    };
    painter.text(
        egui::pos2(rect.left(), outer.top() + 2.0),
        Align2::LEFT_TOP,
        "p_A · доля рецепторов",
        FontId::proportional(12.0),
        MUTED,
    );
    for index in 0..=4 {
        let p = index as f64 / 4.0;
        let y = at(0.0, p).y;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0_f32, BORDER),
        );
        painter.text(
            egui::pos2(rect.left() - 9.0, y),
            Align2::RIGHT_CENTER,
            format!("{p:.2}"),
            FontId::proportional(11.0),
            MUTED,
        );
        let a = if index == 4 {
            max
        } else {
            (end * p).exp_m1().min(max)
        };
        let x = at(a, 0.0).x;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0_f32, BORDER.gamma_multiply(0.5)),
        );
        painter.text(
            egui::pos2(x, rect.bottom() + 10.0),
            Align2::CENTER_TOP,
            number(a),
            FontId::proportional(11.0),
            MUTED,
        );
    }
    painter.text(
        egui::pos2(rect.center().x, outer.bottom() - 2.0),
        Align2::CENTER_BOTTOM,
        "a = [A]своб / Kd_A · шкала ln(1 + a), включая 0",
        FontId::proportional(11.0),
        MUTED,
    );
    painter.add(egui::Shape::line(
        result
            .curve
            .iter()
            .map(|p| at(p.a, p.occupancy.p_a))
            .collect(),
        Stroke::new(2.5_f32, TEAL),
    ));
    for (index, pair) in result.curve.windows(2).enumerate() {
        if index % 4 < 2 {
            painter.line_segment(
                [
                    at(pair[0].a, pair[0].p_a_without_b),
                    at(pair[1].a, pair[1].p_a_without_b),
                ],
                Stroke::new(1.6_f32, PURPLE),
            );
        }
    }
    let a = result.scenario.parameters.a;
    if a <= max {
        let point = at(a, result.current.p_a);
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
        let ratio = f64::from((pointer.x - rect.left()) / rect.width());
        let a = (ratio * end).exp_m1().min(max);
        if let Ok(p) = neuro_engine::calculate(neuro_domain::Parameters {
            a,
            b: result.scenario.parameters.b,
        }) {
            painter.circle_filled(at(a, p.p_a), 4.0, TEAL);
            response.on_hover_text(format!(
                "a = {a:.6e}\np_A = {:.8}\nb = {}",
                p.p_a, result.scenario.parameters.b
            ));
        }
    }
}
pub(crate) fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else if !(0.01..10000.0).contains(&value) {
        format!("{value:.1e}")
    } else {
        format!("{value:.2}")
    }
}
