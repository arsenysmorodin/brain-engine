use super::{BrainLab, BrainViewport, camera_transform, render::egui_color};
use crate::theme::*;
use bevy::prelude::{UVec2, Vec2, Vec3};
use bevy_egui::egui::{self, RichText};
use neuro_domain::{
    BrainCamera, BrainColorMode, BrainGroup, BrainLayers, BrainScene, HemisphereView,
};

pub fn sidebar(ui: &mut egui::Ui, lab: &mut BrainLab) {
    if lab.caffeine.mode {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut lab.caffeine.show_regions, false, "Настройки");
            ui.selectable_value(&mut lab.caffeine.show_regions, true, "Области");
        });
        ui.add_space(8.0);
        if !lab.caffeine.show_regions {
            super::caffeine::sidebar(ui, lab);
            return;
        }
    }
    ui.label(RichText::new("Области мозга").size(18.0).strong());
    ui.add_space(5.0);
    ui.add(
        egui::TextEdit::singleline(&mut lab.search)
            .hint_text("Найти область или FMA ID")
            .desired_width(ui.available_width()),
    );
    egui::ComboBox::from_id_salt("brain-group")
        .selected_text(lab.group.map_or("Все разделы", BrainGroup::label))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut lab.group, None, "Все разделы");
            for group in BrainGroup::ALL {
                ui.selectable_value(&mut lab.group, Some(group), group.label());
            }
        });
    let before = lab.scene.clone();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut lab.scene.hemisphere, HemisphereView::Both, "Оба");
        ui.selectable_value(&mut lab.scene.hemisphere, HemisphereView::Left, "Левое");
        ui.selectable_value(&mut lab.scene.hemisphere, HemisphereView::Right, "Правое");
    });
    let search = lab.search.trim().to_lowercase();
    let matches: Vec<_> = lab
        .atlas
        .regions
        .iter()
        .filter(|r| {
            lab.group.is_none_or(|g| g == r.group)
                && (search.is_empty()
                    || format!("{} {} {}", r.name, r.name_en, r.id)
                        .to_lowercase()
                        .contains(&search))
        })
        .cloned()
        .collect();
    let height = (ui.available_height() - 200.0).max(100.0);
    egui::ScrollArea::vertical()
        .id_salt("brain-regions")
        .max_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if matches.is_empty() {
                muted(
                    ui,
                    "Области не найдены. Попробуйте другое название или сбросьте раздел.",
                );
            }
            for r in matches {
                let visible = lab.scene.visible(&r);
                let selected = lab.scene.selected_region.as_ref() == Some(&r.id);
                let text =
                    RichText::new(&r.name)
                        .size(12.0)
                        .color(if visible { TEXT } else { MUTED });
                if ui
                    .add(
                        egui::Button::new(text)
                            .selected(selected)
                            .wrap()
                            .min_size(egui::vec2(ui.available_width(), 32.0)),
                    )
                    .on_hover_text(format!(
                        "{} · {}\n{}",
                        r.id,
                        r.name_en,
                        if visible {
                            "Видима в сцене"
                        } else {
                            "Скрыта текущими слоями"
                        }
                    ))
                    .clicked()
                {
                    lab.scene.selected_region = Some(r.id);
                    lab.caffeine.selection = None;
                    lab.caffeine.selected_vessel = None;
                    lab.caffeine.revision += 1;
                    lab.reveal_selected();
                }
            }
        });
    ui.separator();
    ui.strong("Слои");
    ui.columns(2, |cols| {
        cols[0].checkbox(&mut lab.scene.layers.cortex, "Кора");
        cols[0].checkbox(&mut lab.scene.layers.deep, "Глубокие");
        cols[0]
            .checkbox(&mut lab.scene.layers.white_matter, "Белое")
            .on_hover_text("Белое вещество");
        cols[1].checkbox(&mut lab.scene.layers.cerebellum, "Мозжечок");
        cols[1].checkbox(&mut lab.scene.layers.brainstem, "Ствол");
        cols[1].checkbox(&mut lab.scene.layers.cavities, "Желудочки");
    });
    if ui.button("Сбросить слои").clicked() {
        lab.scene.layers = BrainLayers::default();
        lab.scene.hidden_regions.clear();
        lab.scene.hemisphere = HemisphereView::Both;
        lab.scene.isolate = false;
    }
    ui.add_space(5.0);
    muted(
        ui,
        if lab.caffeine.mode {
            "Цвет: выбранная оценка кофеина. Серый — значение не задано."
        } else if lab.simulation.mode {
            "Цвет: рассчитанная активность E."
        } else {
            "Цвет обозначает анатомический раздел."
        },
    );
    lab.dirty |= lab.scene != before;
}
pub fn viewport(ui: &mut egui::Ui, lab: &mut BrainLab, viewport: &BrainViewport) {
    let before = lab.scene.clone();
    let mut mode = if lab.caffeine.mode {
        2
    } else if lab.simulation.mode {
        1
    } else {
        0
    };
    ui.horizontal_wrapped(|ui| {
        ui.heading("Мозг человека");
        ui.add_space(10.0);
        ui.selectable_value(&mut mode, 0, "Анатомия");
        ui.selectable_value(&mut mode, 1, "Симуляция");
        ui.selectable_value(&mut mode, 2, "Кофеин");
    });
    lab.simulation.mode = mode == 1;
    lab.caffeine.mode = mode == 2;
    if mode != 1 {
        lab.simulation.playing = false;
    }
    if mode != 2 {
        lab.caffeine.playing = false;
    }
    if lab.caffeine.mode {
        super::caffeine::viewport(ui, lab, viewport);
        lab.dirty |= lab.scene != before;
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (label, yaw, pitch) in [
            ("Слева", -std::f32::consts::FRAC_PI_2, 0.0),
            ("Спереди", std::f32::consts::PI, 0.0),
            ("Сверху", std::f32::consts::PI, 1.45),
        ] {
            if ui.button(label).clicked() {
                lab.scene.camera.yaw = yaw;
                lab.scene.camera.pitch = pitch;
            }
        }
        if ui.button("По центру").clicked() {
            lab.scene.camera = BrainCamera::default();
        }
        if ui.small_button("−").on_hover_text("Отдалить").clicked() {
            zoom(&mut lab.scene, -120.0);
        }
        if ui.small_button("+").on_hover_text("Приблизить").clicked() {
            zoom(&mut lab.scene, 120.0);
        }
        ui.separator();
        if lab.simulation.mode {
            tag(ui, "Активность E", TEAL);
        } else {
            egui::ComboBox::from_id_salt("brain-colors")
                .selected_text(match lab.scene.color_mode {
                    BrainColorMode::Groups => "Цвета областей",
                    BrainColorMode::Anatomy => "Нейтральная анатомия",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut lab.scene.color_mode,
                        BrainColorMode::Groups,
                        "Цвета областей",
                    );
                    ui.selectable_value(
                        &mut lab.scene.color_mode,
                        BrainColorMode::Anatomy,
                        "Нейтральная анатомия",
                    );
                });
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Раздвижение полушарий");
        ui.add(egui::Slider::new(&mut lab.scene.separation, 0.0..=1.0).show_value(false));
        if ui.button("Внутренние структуры").clicked() {
            lab.scene.layers.cortex = false;
            lab.scene.layers.white_matter = false;
            lab.scene.layers.deep = true;
            lab.scene.layers.brainstem = true;
            lab.scene.layers.cerebellum = false;
            lab.scene.isolate = false;
        }
    });
    if lab.simulation.mode {
        super::simulation::transport(ui, lab);
    }
    ui.add_space(4.0);
    let size = egui::vec2(
        ui.available_width(),
        (ui.available_height()
            - if lab.simulation.mode {
                if lab.simulation.cursor.is_some() {
                    240.0
                } else {
                    210.0
                }
            } else {
                80.0
            })
        .max(160.0),
    );
    // Bound the offscreen target, including Retina, without allocating it every frame.
    let ppp = ui.ctx().pixels_per_point();
    let scale = (1600.0 / (size.x * ppp).max(size.y * ppp)).min(1.0);
    lab.target_size = UVec2::new(
        (size.x * ppp * scale).round().max(64.0) as u32,
        (size.y * ppp * scale).round().max(64.0) as u32,
    );
    let response = ui.add(
        egui::Image::new((viewport.texture, size))
            .sense(egui::Sense::click_and_drag())
            .corner_radius(12),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::Grab);
    if lab.simulation.mode {
        super::simulation::overlay(ui, response.rect, lab);
    }
    interact(ui, &response, lab);
    let visible = lab
        .atlas
        .regions
        .iter()
        .filter(|r| lab.scene.visible(r))
        .count();
    if !lab.simulation.mode {
        ui.horizontal_wrapped(|ui| {
            muted(
                ui,
                format!("{visible} из {} структур включены", lab.atlas.regions.len()),
            );
            if let Some(r) = lab.selected() {
                ui.colored_label(TEAL, &r.name);
            }
        });
        ui.label(RichText::new("Вращение — перетаскивание · Масштаб — прокрутка · Выбор — щелчок · Панорама — правая кнопка").size(11.0).color(MUTED));
    }
    if lab.simulation.mode {
        super::simulation::legend(ui);
        super::simulation::timeline(ui, lab);
    }
    ui.label(
        RichText::new(if lab.simulation.mode {
            "BodyParts3D © DBCLS · CC BY 4.0 · Связи — схема, параметры — допущения"
        } else {
            "BodyParts3D © DBCLS · CC BY 4.0 · Геометрия не задаёт динамику"
        })
        .size(11.0)
        .color(MUTED),
    );
    lab.dirty |= lab.scene != before;
}
pub(super) fn interact(ui: &egui::Ui, response: &egui::Response, lab: &mut BrainLab) {
    if response.dragged_by(egui::PointerButton::Primary) {
        let delta = ui.input(|i| i.pointer.delta());
        orbit(&mut lab.scene, Vec2::new(delta.x, delta.y));
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if response.dragged_by(egui::PointerButton::Secondary) {
        let delta = ui.input(|i| i.pointer.delta());
        let c = lab.scene.camera;
        let rotation = camera_transform(c).rotation;
        let target = Vec3::from_array(c.target)
            + rotation * Vec3::new(-delta.x, delta.y, 0.0) * c.distance * 0.0015;
        lab.scene.camera.target = target.clamp(Vec3::splat(-8.0), Vec3::splat(8.0)).to_array();
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            zoom(&mut lab.scene, scroll);
        }
    }
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = (pointer - response.rect.min) / response.rect.size();
        let ndc = Vec2::new(local.x * 2.0 - 1.0, 1.0 - local.y * 2.0);
        let aspect = response.rect.width() / response.rect.height();
        if let Some(v) = lab.pick_vessel(ndc, aspect) {
            lab.caffeine.selected_vessel = Some(v);
            lab.caffeine.selection = None;
            lab.caffeine.revision += 1;
            return;
        }
        lab.caffeine.selected_vessel = None;
        lab.caffeine.revision += 1;
        lab.scene.selected_region = lab.pick(
            Vec2::new(local.x * 2.0 - 1.0, 1.0 - local.y * 2.0),
            response.rect.width() / response.rect.height(),
        );
        if lab.scene.selected_region.is_none() {
            lab.scene.isolate = false;
        }
    }
    if response.double_clicked() {
        lab.focus_selected();
    }
}
pub(super) fn orbit(scene: &mut BrainScene, delta: Vec2) {
    scene.camera.yaw = (scene.camera.yaw - delta.x * 0.007 + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    scene.camera.pitch = (scene.camera.pitch + delta.y * 0.007).clamp(-1.45, 1.45);
}
pub(super) fn zoom(scene: &mut BrainScene, scroll: f32) {
    scene.camera.distance = (scene.camera.distance * (-scroll * 0.0018).exp()).clamp(0.2, 15.0);
}
pub fn inspector(ui: &mut egui::Ui, lab: &mut BrainLab) {
    if lab.caffeine.mode {
        let before = lab.scene.clone();
        super::caffeine::inspector(ui, lab);
        lab.dirty |= lab.scene != before;
        return;
    }
    let before = lab.scene.clone();
    if lab.simulation.mode {
        super::simulation::inspector(ui, lab);
        ui.add_space(10.0);
        ui.separator();
    }
    if let Some(r) = lab.selected().cloned() {
        tag(ui, r.kind.label(), egui_color(r.group));
        ui.add_space(12.0);
        ui.heading(&r.name);
        muted(ui, &r.name_en);
        ui.add_space(10.0);
        ui.strong(r.group.label());
        muted(ui, r.hemisphere.label());
        ui.horizontal(|ui| {
            tag(ui, &r.id, TEAL);
        });
        ui.add_space(12.0);
        if ui
            .add_sized(
                [ui.available_width(), 34.0],
                egui::Button::new("Приблизить структуру"),
            )
            .clicked()
        {
            lab.focus_selected();
        }
        if ui
            .add_sized(
                [ui.available_width(), 34.0],
                egui::Button::new(if lab.scene.isolate {
                    "Вернуть остальные структуры"
                } else {
                    "Оставить только выбранную"
                }),
            )
            .clicked()
        {
            lab.scene.isolate = !lab.scene.isolate;
            lab.reveal_selected();
            if lab.scene.isolate {
                lab.focus_selected();
            }
        }
        if !lab.scene.visible(&r) {
            ui.colored_label(PURPLE, "Выбранная структура сейчас скрыта.");
            if ui.button("Показать структуру").clicked() {
                lab.reveal_selected();
            }
        } else if ui.button("Скрыть структуру").clicked() {
            lab.scene.hidden_regions.push(r.id.clone());
        }
        if ui.button("Снять выделение").clicked() {
            lab.scene.selected_region = None;
            lab.scene.isolate = false;
        }
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);
        ui.strong("Данные области");
        muted(ui, format!("Объектов геометрии: {}", r.mesh_ids.len()));
        if !lab.simulation.mode {
            ui.label("Динамика доступна на вкладке «Симуляция». Области без настроенной популяции остаются неизвестными.");
        }
        ui.add_space(10.0);
        muted(
            ui,
            "FMA ID связывает анатомическую область с параметрами и результатами сети. Форма меша не задаёт динамику.",
        );
        ui.collapsing("Исходные объекты", |ui| {
            for id in &r.mesh_ids {
                muted(ui, id);
            }
        });
    } else if !lab.simulation.mode {
        tag(ui, "Анатомия человека", TEAL);
        ui.add_space(12.0);
        ui.heading("Основа симуляции");
        ui.add_space(10.0);
        ui.label("Выберите область на мозге или в списке. Внутренние структуры доступны через слои и раздвижение полушарий.");
        ui.add_space(14.0);
        ui.label(RichText::new(format!("{} структур", lab.atlas.regions.len())).size(25.0));
        muted(
            ui,
            format!(
                "{} меша · {} треугольников",
                lab.atlas.meshes.len(),
                lab.atlas.total_triangles
            ),
        );
        ui.add_space(12.0);
        ui.strong("Слои модели");
        for group in BrainGroup::ALL {
            ui.colored_label(egui_color(group), group.label());
        }
    } else {
        ui.add_space(12.0);
        muted(
            ui,
            "Выберите область на мозге или в списке: здесь появятся её E/I-популяции и направленные связи.",
        );
    }
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(10.0);
    ui.strong("Происхождение");
    ui.hyperlink_to(&lab.atlas.source.title, &lab.atlas.source.url);
    muted(ui, &lab.atlas.species);
    ui.hyperlink_to("Лицензия CC BY 4.0", &lab.atlas.source.license_url);
    ui.collapsing("Источник и границы", |ui| {
        muted(ui, &lab.atlas.source.attribution);
        muted(ui, &lab.atlas.source.changes);
        muted(ui, &lab.atlas.limitations);
        muted(
            ui,
            format!(
                "Проверены данные и лицензия: {}",
                lab.atlas.source.checked_on
            ),
        );
    });
    lab.dirty |= lab.scene != before;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_frames_rotate_pan_and_zoom_the_brain() {
        let ctx = egui::Context::default();
        let mut lab = BrainLab::new().unwrap();
        let original = lab.scene.camera;
        let start = egui::pos2(200.0, 180.0);
        let end = start + egui::vec2(50.0, 30.0);
        let button = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        use egui::PointerButton::{Primary, Secondary};
        for (frame, events) in [
            vec![],
            vec![
                egui::Event::PointerMoved(start),
                button(start, Primary, true),
            ],
            vec![egui::Event::PointerMoved(end)],
            vec![button(end, Primary, false)],
            vec![button(end, Secondary, true)],
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, Secondary, false)],
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100.0),
                modifiers: egui::Modifiers::NONE,
            }],
        ]
        .into_iter()
        .enumerate()
        {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 600.0),
                    )),
                    time: Some(frame as f64 / 30.0),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (_, response) = ui.allocate_exact_size(
                            egui::vec2(600.0, 500.0),
                            egui::Sense::click_and_drag(),
                        );
                        interact(ui, &response, &mut lab);
                    });
                },
            );
        }
        assert!((lab.scene.camera.yaw - original.yaw + 0.35).abs() < 1e-5);
        assert!((lab.scene.camera.pitch - original.pitch - 0.21).abs() < 1e-5);
        assert_ne!(lab.scene.camera.target, original.target);
        assert!(lab.scene.camera.distance < original.distance);
        lab.scene.validate(&lab.atlas).unwrap();
    }
}
