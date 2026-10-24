use super::{CaffeineLab, MapSelection, kind_color};
use crate::theme::*;
use bevy_egui::egui::{self, Align2, FontId, Pos2, Rect, Stroke};
fn position(id: &str) -> egui::Vec2 {
    let (x, y) = match id {
        "dose" => (85.0, 65.0),
        "gut" => (270.0, 65.0),
        "blood" => (455.0, 65.0),
        "brain" => (650.0, 65.0),
        "a1" => (650.0, 205.0),
        "a2a" => (840.0, 205.0),
        "adenosine" => (455.0, 345.0),
        "liver" => (270.0, 205.0),
        "removed" => (270.0, 345.0),
        "metabolites" => (85.0, 345.0),
        _ => (840.0, 345.0),
    };
    egui::vec2(x, y)
}
fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let f = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * f)
}
pub fn show(ui: &mut egui::Ui, c: &mut CaffeineLab, height: f32) {
    ui.horizontal(|ui| {
        if ui.small_button("По центру").clicked() {
            c.map_pan = egui::Vec2::ZERO;
            c.map_zoom = 1.0;
        }
        if ui.small_button("−").clicked() {
            c.map_zoom = (c.map_zoom / 1.15).max(0.5);
        }
        if ui.small_button("+").clicked() {
            c.map_zoom = (c.map_zoom * 1.15).min(2.5);
        }
        muted(ui, "Выбор узла или связи · Перемещение и масштаб");
    });
    let (canvas, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), (height - 29.0).max(60.0)),
        egui::Sense::click_and_drag(),
    );
    if response.dragged() {
        c.map_pan += ui.input(|i| i.pointer.delta());
    }
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("caffeine-pathway-canvas"), canvas));
    let fit = (canvas.width() / 950.0)
        .min(canvas.height() / 425.0)
        .clamp(0.65, 1.25);
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let old = c.map_zoom;
            c.map_zoom = (c.map_zoom * (scroll * 0.0015).exp()).clamp(0.5, 2.5);
            if let Some(p) = response.hover_pos() {
                let center = canvas.center() + c.map_pan;
                c.map_pan += (p - center) * (1.0 - c.map_zoom / old);
            }
        }
    }
    let scale = fit * c.map_zoom;
    let center = canvas.center() + c.map_pan;
    let at = |id: &str| center + (position(id) - egui::vec2(475.0, 210.0)) * scale;
    let painter = ui.painter_at(canvas);
    let node_size = egui::vec2(165.0, 70.0) * scale;
    for (i, e) in c.knowledge.links.iter().enumerate() {
        let a = at(&e.from);
        let b = at(&e.to);
        let d = (b - a).normalized();
        let half = node_size * 0.5;
        let trim = |v: egui::Vec2| (half.x / v.x.abs()).min(half.y / v.y.abs());
        let start = a + d * trim(d);
        let end = b - d * (trim(d) + 3.0);
        let selected = c.selection == Some(MapSelection::Link(i));
        let color = kind_color(&e.kind).gamma_multiply(if selected { 1.0 } else { 0.55 });
        let stroke = Stroke::new(if selected { 2.7_f32 } else { 1.4_f32 }, color);
        if e.kind == "hypothesis" {
            let length = start.distance(end);
            let n = (length / 10.0).ceil() as usize;
            for j in 0..n {
                let t = j as f32 / n as f32;
                let t2 = ((j as f32 + 0.55) / n as f32).min(1.0);
                painter.line_segment([start.lerp(end, t), start.lerp(end, t2)], stroke);
            }
        } else {
            painter.line_segment([start, end], stroke);
        }
        let n = egui::vec2(-d.y, d.x);
        painter.add(egui::Shape::convex_polygon(
            vec![end, end - d * 8.0 + n * 3.0, end - d * 8.0 - n * 3.0],
            color,
            Stroke::NONE,
        ));
    }
    for (i, n) in c.knowledge.nodes.iter().enumerate() {
        let rect = Rect::from_center_size(at(&n.id), node_size);
        let selected = c.selection == Some(MapSelection::Node(i));
        let color = kind_color(&n.kind);
        painter.rect_filled(rect, 7.0, if selected { RAISED } else { SURFACE });
        painter.rect_stroke(
            rect,
            7.0,
            Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected { color } else { BORDER },
            ),
            egui::StrokeKind::Inside,
        );
        let text_size = (14.0 * scale).clamp(10.0, 17.0);
        let node_painter = painter.with_clip_rect(rect.shrink(3.0));
        node_painter.text(
            rect.center() - egui::vec2(0.0, 11.0 * scale),
            Align2::CENTER_CENTER,
            &n.title,
            FontId::proportional(text_size),
            TEXT,
        );
        let galley = node_painter.layout(
            c.node_value(&n.id),
            FontId::proportional((11.0 * scale).clamp(8.5, 13.0)),
            color,
            (rect.width() - 6.0).max(20.0),
        );
        let pos = Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y + 3.0 * scale,
        );
        node_painter.galley(pos, galley, color);
    }
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        if let Some(i) = c
            .knowledge
            .nodes
            .iter()
            .position(|n| Rect::from_center_size(at(&n.id), node_size).contains(p))
        {
            c.selection = Some(MapSelection::Node(i));
        } else if let Some((i, _)) = c
            .knowledge
            .links
            .iter()
            .enumerate()
            .map(|(i, e)| (i, segment_distance(p, at(&e.from), at(&e.to))))
            .filter(|(_, d)| *d < 8.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            c.selection = Some(MapSelection::Link(i));
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(TEAL, "Публикации / PET");
        ui.colored_label(egui::Color32::from_rgb(125, 177, 224), "PK-модель");
        ui.colored_label(PURPLE, "Пунктир — гипотеза");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_frames_select_nodes_links_and_pan_the_pathway() {
        let atlas = neuro_domain::BrainAtlas::bundled().unwrap();
        let mut c = CaffeineLab::new(&atlas).unwrap();
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        // Capture the actual canvas at layout time rather than assuming a window offset.
        let mut canvas = Rect::NOTHING;
        let mut frame = 0;
        let mut run = |events: Vec<egui::Event>, c: &mut CaffeineLab| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1100.0, 750.0))),
                    time: Some(frame as f64 / 30.0),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        show(ui, c, 500.0);
                        canvas = ctx
                            .data(|d| d.get_temp::<Rect>(egui::Id::new("caffeine-pathway-canvas")))
                            .unwrap();
                    });
                },
            );
            frame += 1;
            canvas
        };
        let rect = run(vec![], &mut c);
        let scale = (rect.width() / 950.0).min(rect.height() / 425.0);
        let at = |id: &str| rect.center() + (position(id) - egui::vec2(475.0, 210.0)) * scale;
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let p = at("a1");
        run(vec![egui::Event::PointerMoved(p), button(p, true)], &mut c);
        run(vec![button(p, false)], &mut c);
        assert_eq!(c.selection, Some(MapSelection::Node(4)));
        let p = at("dose").lerp(at("gut"), 0.5);
        run(vec![egui::Event::PointerMoved(p), button(p, true)], &mut c);
        run(vec![button(p, false)], &mut c);
        assert_eq!(c.selection, Some(MapSelection::Link(0)));
        let p = rect.left_bottom() + egui::vec2(15.0, -15.0);
        run(vec![egui::Event::PointerMoved(p), button(p, true)], &mut c);
        run(
            vec![egui::Event::PointerMoved(p + egui::vec2(30.0, -20.0))],
            &mut c,
        );
        run(vec![button(p + egui::vec2(30.0, -20.0), false)], &mut c);
        assert_eq!(c.map_pan, egui::vec2(30.0, -20.0));
    }
}
