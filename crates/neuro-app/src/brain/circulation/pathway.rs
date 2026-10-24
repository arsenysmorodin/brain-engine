//! Common interactive transport/mechanism map for every supported compound.
use super::*;
use bevy_egui::egui::{Align2, FontId, Pos2, Rect, Stroke};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MapSelection {
    Node(usize),
    Link(usize),
}
const TITLES: [&str; 12] = [
    "Приём вещества",
    "ЖКТ",
    "Общий кровоток",
    "Артерии",
    "Капилляры",
    "Вены",
    "Возврат в кровь",
    "Ткань мозга",
    "Мишени",
    "Учебный вход E/I",
    "Устранено",
    "PK-взаимодействия",
];
const POSITIONS: [(f32, f32); 12] = [
    (85., 55.),
    (285., 55.),
    (485., 55.),
    (685., 55.),
    (685., 225.),
    (485., 225.),
    (285., 225.),
    (685., 395.),
    (485., 395.),
    (285., 395.),
    (85., 225.),
    (85., 395.),
];
const EDGES: [(usize, usize); 13] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (4, 5),
    (5, 6),
    (6, 2),
    (4, 7),
    (7, 4),
    (7, 8),
    (7, 9),
    (2, 10),
    (11, 2),
];
fn color(node: usize) -> Color32 {
    match node {
        8 => DRUG_COLORS[0],
        9 | 11 => PURPLE,
        _ => TEAL,
    }
}
fn bed(c: &CirculationLab, id: Option<&str>) -> Option<usize> {
    id.map_or(Some(0), |id| c.bed_index(id))
}
fn value(c: &CirculationLab, node: usize, region: Option<&str>) -> String {
    let Some(f) = c.frame() else {
        return "Пересчитайте протокол".into();
    };
    let d = c.selected.index();
    let s = f.systemic[d];
    let r = bed(c, region).map(|i| f.regions[i][d]);
    match node {
        0 => format!("{:.1} мг принято", s.administered_mg),
        1 => format!("{:.2} мг", s.gut_mg),
        2 | 3 => format!("Cp {:.3} мг/л", s.central_mg_l),
        4 => r.map_or("Нет модели области".into(), |r| {
            format!("Ccap {:.3} мг/л", r.capillary_mg_l)
        }),
        5 => r.map_or("Нет модели области".into(), |r| {
            format!("Cv {:.3} мг/л", r.venous_mg_l)
        }),
        6 => format!("{:.4} мг/мин", s.return_mg_min),
        7 => r.map_or("Нет модели области".into(), |r| {
            format!("Ct {:.3} мг/л", r.tissue_mg_l)
        }),
        8 => {
            if c.selected == Purine::Caffeine {
                "A1 / A2A: PET отдельно".into()
            } else {
                "A2A: только in vitro".into()
            }
        }
        9 => {
            if c.scenario
                .neural_hypothesis
                .as_ref()
                .is_some_and(|h| h.enabled)
            {
                "Включена гипотеза".into()
            } else {
                "Гипотеза выключена".into()
            }
        }
        10 => format!("{:.2} мг исходного", s.eliminated_mg),
        _ => {
            if c.interactions_applied() {
                "Парные профили".into()
            } else {
                "Профиль не применён".into()
            }
        }
    }
}
pub fn show(ui: &mut egui::Ui, lab: &mut BrainLab, height: f32) {
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("По центру").clicked() {
            lab.circulation.map_pan = egui::Vec2::ZERO;
            lab.circulation.map_zoom = 1.0;
        }
        for (text, factor) in [("−", 1.0 / 1.15), ("+", 1.15)] {
            if ui.small_button(text).clicked() {
                lab.circulation.map_zoom = (lab.circulation.map_zoom * factor).clamp(0.65, 2.5);
            }
        }
        egui::ComboBox::from_id_salt("purine-map-region")
            .selected_text(
                lab.selected()
                    .map_or("Выбрать область", |r| r.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for r in lab
                    .atlas
                    .regions
                    .iter()
                    .filter(|r| r.kind != neuro_domain::TissueKind::Cavity)
                {
                    ui.selectable_value(
                        &mut lab.scene.selected_region,
                        Some(r.id.clone()),
                        &r.name,
                    );
                }
            });
    });
    let (canvas, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), (height - 35.0).max(80.0)),
        egui::Sense::click_and_drag(),
    );
    let c = &mut lab.circulation;
    if response.dragged() {
        c.map_pan += ui.input(|i| i.pointer.delta());
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            c.map_zoom = (c.map_zoom * (scroll * 0.0015).exp()).clamp(0.65, 2.5);
        }
    }
    let scale = (canvas.width() / 790.0)
        .min(canvas.height() / 465.0)
        .clamp(0.6, 1.3)
        * c.map_zoom;
    let center = canvas.center() + c.map_pan;
    let at = |i: usize| {
        center + (egui::vec2(POSITIONS[i].0, POSITIONS[i].1) - egui::vec2(385.0, 225.0)) * scale
    };
    let size = egui::vec2(176.0, 68.0) * scale;
    let painter = ui.painter_at(canvas);
    let mut links = Vec::new();
    for (i, (from, to)) in EDGES.iter().enumerate() {
        let mut points = vec![at(*from), at(*to)];
        if i == 6 {
            points = vec![
                at(*from),
                at(*from) + egui::vec2(0.0, -85.0) * scale,
                at(*to) + egui::vec2(0.0, 85.0) * scale,
                at(*to),
            ];
        }
        if i == 11 {
            points = vec![
                at(*from),
                at(*from) + egui::vec2(0.0, 85.0) * scale,
                at(*to) - egui::vec2(0.0, 85.0) * scale,
                at(*to),
            ];
        }
        if i == 10 {
            points = vec![
                at(*from),
                at(*from) + egui::vec2(0.0, 55.0) * scale,
                at(*to) + egui::vec2(0.0, 55.0) * scale,
                at(*to),
            ];
        }
        if i == 12 {
            points = vec![
                at(*from),
                at(*from) - egui::vec2(95.0, 0.0) * scale,
                at(*from) - egui::vec2(95.0, 395.0) * scale,
                at(*to) - egui::vec2(0.0, 55.0) * scale,
                at(*to),
            ];
        }
        let first = (points[1] - points[0]).normalized();
        let last = (points[points.len() - 1] - points[points.len() - 2]).normalized();
        let trim = |v: egui::Vec2| (size.x * 0.5 / v.x.abs()).min(size.y * 0.5 / v.y.abs());
        points[0] += first * trim(first);
        let n = points.len();
        points[n - 1] -= last * (trim(last) + 3.0);
        let col = if *to == 9 || *from == 11 {
            PURPLE
        } else {
            TEAL
        }
        .gamma_multiply(if c.map_selection == Some(MapSelection::Link(i)) {
            1.0
        } else {
            0.55
        });
        painter.add(egui::Shape::line(
            points.clone(),
            Stroke::new(
                if c.map_selection == Some(MapSelection::Link(i)) {
                    2.5
                } else {
                    1.3_f32
                },
                col,
            ),
        ));
        let end = points[n - 1];
        let normal = egui::vec2(-last.y, last.x);
        painter.add(egui::Shape::convex_polygon(
            vec![
                end,
                end - last * 8.0 + normal * 3.0,
                end - last * 8.0 - normal * 3.0,
            ],
            col,
            Stroke::NONE,
        ));
        links.push(points);
    }
    for (i, title) in TITLES.iter().enumerate() {
        let rect = Rect::from_center_size(at(i), size);
        let selected = c.map_selection == Some(MapSelection::Node(i));
        painter.rect_filled(rect, 7.0, if selected { RAISED } else { SURFACE });
        painter.rect_stroke(
            rect,
            7.0,
            Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected { color(i) } else { BORDER },
            ),
            egui::StrokeKind::Inside,
        );
        let p = painter.with_clip_rect(rect.shrink(3.0));
        p.text(
            rect.center() - egui::vec2(0.0, 11.0 * scale),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional((14.0 * scale).clamp(10.0, 17.0)),
            TEXT,
        );
        let galley = p.layout(
            value(c, i, lab.scene.selected_region.as_deref()),
            FontId::proportional((11.0 * scale).clamp(9.0, 13.0)),
            color(i),
            rect.width() - 6.0,
        );
        p.galley(
            egui::pos2(
                rect.center().x - galley.size().x * 0.5,
                rect.center().y + 4.0 * scale,
            ),
            galley,
            color(i),
        );
    }
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        c.map_selection = (0..TITLES.len())
            .find(|&i| Rect::from_center_size(at(i), size).contains(pos))
            .map(MapSelection::Node)
            .or_else(|| {
                links
                    .iter()
                    .position(|points| points.windows(2).any(|p| distance(pos, p[0], p[1]) < 7.0))
                    .map(MapSelection::Link)
            });
    }
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::new("purine-pathway-node-rects"),
            (0..TITLES.len())
                .map(|i| Rect::from_center_size(at(i), size))
                .collect::<Vec<_>>(),
        );
        data.insert_temp(egui::Id::new("purine-pathway-canvas"), canvas);
    });
    muted(
        ui,
        "Выберите узел или стрелку: справа появятся механизм и источник. Бирюзовый — транспорт; фиолетовый — гипотеза/перенос профиля.",
    );
}
fn distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let f = ((p - a).dot(d) / d.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + d * f)
}
pub fn inspector(ui: &mut egui::Ui, c: &CirculationLab) {
    let Some(selection) = c.map_selection.filter(|_| c.route) else {
        return;
    };
    let node = match selection {
        MapSelection::Node(i) => i,
        MapSelection::Link(i) => {
            let (a, b) = EDGES[i];
            ui.label(format!("{} → {}", TITLES[a], TITLES[b]));
            b
        }
    };
    ui.label(RichText::new(TITLES[node]).strong());
    let text = match node {
        0 => {
            "Доза добавляется в депо ЖКТ в указанную минуту, затем всасывается. До этого момента вещество не поступает в кровь."
        }
        1 => {
            "Всасывание первого порядка. Полувремя всасывания не равно Tmax; параметры M/T здесь приближённые."
        }
        2 | 3 => {
            "Общий хорошо перемешанный компартмент задаёт входную Cp. Его V/F — кажущийся объём распределения, а не анатомический объём крови. Кровь/плазма=1 — допущение."
        }
        4 => {
            "Конечный капиллярный объём получает вещество из артерий и обменивается с тканью. Ccap может отличаться от Ct и Cv."
        }
        5 | 6 => {
            "Из капилляров вещество поступает в венозный компартмент и с потоком Q·Cv возвращается в общий кровоток. Повторная циркуляция не создаёт дополнительную дозу."
        }
        7 => {
            "Обмен PS(Ccap−Ct/Kp). Kp и скорость переноса через ГЭБ для M/T — условные параметры. Цвет показывает концентрацию, а не активацию нейронов."
        }
        8 => {
            if c.selected == Purine::Caffeine {
                "Для кофеина отдельно доступны опубликованные PET-оценки A1 и A2A. A2A-кривая относится к стриатуму пациентов с болезнью Паркинсона; занятость не равна активности."
            } else {
                "Для M/T есть данные об антагонизме A2A in vitro. Они не определяют занятость рецепторов в человеческом мозге и не задают численную E/I-кривую."
            }
        }
        9 => {
            "ΔP_E=Σg·Ct/(S+Ct): явная учебная гипотеза. Коэффициенты g и масштабы S задаёт исследователь. Сравниваются одинаковые сети с/без добавочного входа при выбранном PK-снимке."
        }
        10 => {
            "Клиренс удаляет исходное вещество из общего компартмента. Это не расчёт массы метаболитов или мочи. Для всех трёх веществ сохраняется баланс массы."
        }
        _ => {
            "Метиллиберин может менять PK кофеина, кофеин — экспозицию теакрина. В модели применяются парные отношения CL/F и V/F; тройное сочетание и иные дозы остаются экстраполяцией."
        }
    };
    ui.label(text);
    if matches!(node, 8 | 11) {
        let ids: &[&str] = if node == 11 {
            &["mondal2022", "he2017", "wang2020"]
        } else if c.selected != Purine::Caffeine {
            &["santillo2026"]
        } else {
            &[]
        };
        for s in c
            .knowledge
            .sources
            .iter()
            .filter(|s| ids.contains(&s.id.as_str()))
        {
            ui.hyperlink_to(&s.title, &s.url);
            muted(ui, &s.verification);
        }
        if node == 8 && c.selected == Purine::Caffeine {
            ui.hyperlink_to(
                "A1 PET — Elmenhorst 2012",
                "https://jnm.snmjournals.org/content/53/11/1723",
            );
            ui.hyperlink_to(
                "A2A PET — Ishibashi 2022",
                "https://pmc.ncbi.nlm.nih.gov/articles/PMC9306703/",
            );
        }
    }
    ui.separator();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_map_selects_targets_and_transport_links_for_methylliberine() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let mut lab = BrainLab::new().unwrap();
        lab.circulation.selected = Purine::Methylliberine;
        let mut node_center = Pos2::ZERO;
        let mut link_center = Pos2::ZERO;
        for frame in 0..7 {
            let pos = if frame <= 3 { node_center } else { link_center };
            let events = if [2, 3, 5, 6].contains(&frame) {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2 || frame == 5,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            };
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| show(ui, &mut lab, 550.0));
                },
            );
            if frame == 1 {
                let rects = ctx
                    .data(|d| d.get_temp::<Vec<Rect>>(egui::Id::new("purine-pathway-node-rects")))
                    .unwrap();
                node_center = rects[8].center();
                link_center = rects[0].center().lerp(rects[1].center(), 0.5);
            }
            if frame == 3 {
                assert_eq!(lab.circulation.map_selection, Some(MapSelection::Node(8)));
            }
        }
        assert_eq!(lab.circulation.map_selection, Some(MapSelection::Link(0)));
    }
}
