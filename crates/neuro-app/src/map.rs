use crate::{Selection, theme::*};
use bevy_egui::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use neuro_domain::{Catalog, Relation};

#[derive(Default)]
pub struct MapState {
    pub zoom: f32,
    pub pan: Vec2,
}
impl MapState {
    pub fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }
}
fn position(id: &str) -> Vec2 {
    match id {
        "caffeine" => egui::vec2(-205.0, 0.0),
        "adenosine" => egui::vec2(205.0, 0.0),
        "a1" => egui::vec2(0.0, -155.0),
        _ => egui::vec2(0.0, 155.0),
    }
}
fn node_title(id: &str) -> &'static str {
    match id {
        "caffeine" => "Кофеин",
        "adenosine" => "Аденозин",
        "a1" => "A1",
        _ => "A2A",
    }
}
pub fn show(ui: &mut egui::Ui, catalog: &Catalog, state: &mut MapState, selection: &mut Selection) {
    if state.zoom == 0.0 {
        state.reset();
    }
    ui.horizontal(|ui| {
        tag(ui, "Опубликованные данные", TEAL);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("По центру").clicked() {
                state.reset();
            }
            if ui.button("+").clicked() {
                state.zoom = (state.zoom * 1.15).min(1.8);
            }
            if ui.button("−").clicked() {
                state.zoom = (state.zoom / 1.15).max(0.55);
            }
            ui.label(format!("{:.0}%", state.zoom * 100.0));
        });
    });
    ui.add_space(10.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            (ui.available_height() - 58.0).max(220.0),
        ),
        Sense::click_and_drag(),
    );
    let base = (rect.width() / 650.0)
        .min(rect.height() / 465.0)
        .clamp(0.55, 1.25);
    if response.dragged() {
        state.pan += ui.input(|i| i.pointer.delta());
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        let old = state.zoom;
        state.zoom = (state.zoom * (scroll * 0.002).exp()).clamp(0.55, 1.8);
        if let Some(pointer) = response.hover_pos() {
            state.pan = pointer
                - rect.center()
                - (pointer - rect.center() - state.pan) * (state.zoom / old);
        }
    }
    let scale = state.zoom * base;
    let center = rect.center() + state.pan;
    let at = |id: &str| center + position(id) * scale;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 10, BACKGROUND);
    for x in (0..(rect.width() as usize)).step_by(28) {
        for y in (0..(rect.height() as usize)).step_by(28) {
            painter.circle_filled(
                rect.min + egui::vec2(x as f32 + 14.0, y as f32 + 14.0),
                0.8,
                BORDER.gamma_multiply(0.55),
            );
        }
    }
    let half = egui::vec2(74.0, 37.0) * scale;
    for claim in &catalog.claims {
        let start = at(&claim.subject);
        let end = at(&claim.object);
        let vector = end - start;
        let direction = vector.normalized();
        let trim = |d: Vec2| (half.x / d.x.abs()).min(half.y / d.y.abs());
        let start = start + direction * trim(direction);
        let end = end - direction * (trim(direction) + 4.0);
        let color = if claim.relation == Relation::Antagonist {
            PURPLE
        } else {
            TEAL
        };
        let selected = matches!(selection, Selection::Claim(id) if id == &claim.id);
        painter.line_segment(
            [start, end],
            Stroke::new(
                if selected { 3.0_f32 } else { 1.5_f32 },
                if selected {
                    color
                } else {
                    color.gamma_multiply(0.6)
                },
            ),
        );
        let perpendicular = egui::vec2(-direction.y, direction.x);
        match claim.relation {
            Relation::Antagonist => {
                painter.line_segment(
                    [end - perpendicular * 8.0, end + perpendicular * 8.0],
                    Stroke::new(2.4_f32, color),
                );
            }
            Relation::Agonist => {
                painter.line_segment(
                    [end - direction * 10.0 - perpendicular * 5.0, end],
                    Stroke::new(2.0_f32, color),
                );
                painter.line_segment(
                    [end - direction * 10.0 + perpendicular * 5.0, end],
                    Stroke::new(2.0_f32, color),
                );
            }
        }
        let mid = start + (end - start) * 0.5;
        let label = Rect::from_center_size(mid, egui::vec2(100.0, 26.0) * scale.sqrt());
        painter.rect_filled(label, 5, BACKGROUND);
        painter.text(
            mid,
            Align2::CENTER_CENTER,
            claim.relation.label(),
            FontId::proportional(12.0 * scale.sqrt()),
            color,
        );
        if response.clicked()
            && let Some(pointer) = response.interact_pointer_pos()
            && (label.expand(6.0).contains(pointer) || segment_distance(pointer, start, end) < 10.0)
        {
            *selection = Selection::Claim(claim.id.clone());
        }
    }
    for entity in &catalog.entities {
        let node = Rect::from_center_size(at(&entity.id), half * 2.0);
        let selected = matches!(selection, Selection::Entity(id) if id == &entity.id);
        let hover = response.hover_pos().is_some_and(|p| node.contains(p));
        let color = if entity.id == "caffeine" {
            PURPLE
        } else {
            TEAL
        };
        painter.rect_filled(
            node.expand(4.0),
            12,
            color.gamma_multiply(if selected { 0.12 } else { 0.03 }),
        );
        painter.rect_filled(node, 8, RAISED);
        painter.rect_stroke(
            node,
            8,
            Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected || hover { color } else { BORDER },
            ),
            egui::StrokeKind::Inside,
        );
        let pos = node.center();
        painter.text(
            pos - egui::vec2(0.0, 9.0 * scale),
            Align2::CENTER_CENTER,
            node_title(&entity.id),
            FontId::proportional(19.0 * scale.sqrt()),
            TEXT,
        );
        painter.text(
            pos + egui::vec2(0.0, 17.0 * scale),
            Align2::CENTER_CENTER,
            &entity.kind,
            FontId::proportional(11.0 * scale.sqrt()),
            MUTED,
        );
        if response.clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| node.contains(p))
        {
            *selection = Selection::Entity(entity.id.clone());
        }
        if hover {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(PURPLE, "⊣  Антагонист");
        ui.colored_label(TEAL, "→  Агонист");
        muted(ui, "Выберите узел или связь");
    });
    ui.label(
        egui::RichText::new(
            "Перетаскивание — перемещение; прокрутка — масштаб. Положение узлов схематическое.",
        )
        .size(12.0)
        .color(MUTED),
    );
}
fn segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let v = end - start;
    let t = ((point - start).dot(v) / v.length_sq()).clamp(0.0, 1.0);
    point.distance(start + v * t)
}
