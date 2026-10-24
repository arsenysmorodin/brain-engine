use super::{BrainLab, camera_transform};
use crate::theme::*;
use bevy::prelude::{Vec2, Vec3};
use bevy_egui::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke};
use neuro_domain::{
    BrainAtlas, Error, NETWORK_SOURCE_URL, NetworkScenario, PopulationConnection, PopulationState,
};
use neuro_engine::NetworkSimulation;
use std::path::PathBuf;

pub struct SimulationLab {
    pub mode: bool,
    pub scenario: NetworkScenario,
    pub engine: Option<NetworkSimulation>,
    pub playing: bool,
    pub speed: f64,
    pub cursor: Option<usize>,
    pub revision: u64,
    pub show_links: bool,
    pub error: Option<String>,
    pub dirty: bool,
    pub last_path: Option<PathBuf>,
    pub source_path: Option<PathBuf>,
    pub add_target: usize,
    credit_ms: f64,
}
impl SimulationLab {
    pub fn new(atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::from_scenario(NetworkScenario::bundled(atlas)?, atlas)
    }
    pub fn from_scenario(scenario: NetworkScenario, atlas: &BrainAtlas) -> Result<Self, Error> {
        let engine = NetworkSimulation::new(scenario.clone(), atlas)?;
        Ok(Self {
            mode: false,
            scenario,
            engine: Some(engine),
            playing: false,
            speed: 100.0,
            cursor: None,
            revision: 0,
            show_links: true,
            error: None,
            dirty: false,
            last_path: None,
            source_path: None,
            add_target: 0,
            credit_ms: 0.0,
        })
    }
    pub fn reset(&mut self, atlas: &BrainAtlas) {
        self.playing = false;
        self.credit_ms = 0.0;
        self.cursor = None;
        self.revision = self.revision.wrapping_add(1);
        match NetworkSimulation::new(self.scenario.clone(), atlas) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.error = None;
            }
            Err(e) => {
                self.engine = None;
                self.error = Some(e.to_string());
            }
        }
    }
    pub fn advance(&mut self, steps: usize) {
        if let Some(engine) = &mut self.engine {
            if let Err(e) = engine.advance_steps(steps) {
                self.error = Some(e.to_string());
                self.playing = false;
            }
            if engine.finished() {
                self.playing = false;
                self.credit_ms = 0.0;
            }
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn tick(&mut self, wall_seconds: f64) {
        if !self.mode || !self.playing || self.error.is_some() {
            return;
        }
        self.credit_ms += wall_seconds.clamp(0.0, 0.1) * self.speed;
        let steps = (self.credit_ms / self.scenario.dt_ms).floor() as usize;
        if steps > 0 {
            let steps = steps.min(400);
            self.credit_ms -= steps as f64 * self.scenario.dt_ms;
            self.advance(steps);
        }
    }
    pub fn display_time(&self) -> f64 {
        self.engine.as_ref().map_or(0.0, |e| {
            self.cursor
                .and_then(|i| e.samples().get(i))
                .map_or(e.time_ms(), |s| s.time_ms)
        })
    }
    pub fn state(&self, id: &str) -> Option<PopulationState> {
        if self.error.is_some() {
            return None;
        }
        let index = self
            .scenario
            .populations
            .iter()
            .position(|p| p.region_id == id)?;
        let engine = self.engine.as_ref()?;
        Some(
            self.cursor
                .and_then(|i| engine.samples().get(i))
                .map_or(engine.states(), |s| &s.states)[index],
        )
    }
    pub fn stimulate(&mut self, id: &str, atlas: &BrainAtlas) {
        if self.scenario.populations.iter().any(|p| p.region_id == id) {
            self.scenario.stimuli = vec![neuro_domain::PopulationStimulus {
                region_id: id.into(),
                start_ms: 0.0,
                duration_ms: ((80.0 / self.scenario.dt_ms).round() * self.scenario.dt_ms)
                    .min(self.scenario.duration_ms),
                amplitude: 6.0,
            }];
            self.dirty = true;
            self.reset(atlas);
            self.playing = self.error.is_none();
        }
    }
}
pub fn heat_rgb(value: f64) -> [u8; 3] {
    let stops = [
        (0.0, [32.0, 48.0, 68.0]),
        (0.15, [40.0, 113.0, 159.0]),
        (0.4, [84.0, 217.0, 192.0]),
        (0.7, [255.0, 178.0, 82.0]),
        (1.0, [255.0, 91.0, 79.0]),
    ];
    let value = value.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        if value <= pair[1].0 {
            let f = (value - pair[0].0) / (pair[1].0 - pair[0].0);
            return std::array::from_fn(|a| {
                (pair[0].1[a] + f * (pair[1].1[a] - pair[0].1[a])).round() as u8
            });
        }
    }
    [255, 91, 79]
}
pub fn transport(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let sim = &mut lab.simulation;
    ui.horizontal_wrapped(|ui| {
        let ready = sim.engine.is_some() && sim.error.is_none();
        if ui
            .add_enabled(
                ready,
                egui::Button::new(if sim.playing {
                    "Пауза"
                } else {
                    "Запуск"
                }),
            )
            .clicked()
        {
            if sim.engine.as_ref().is_some_and(|e| e.finished()) {
                sim.reset(&lab.atlas);
            }
            sim.playing = !sim.playing;
            sim.cursor = None;
            sim.credit_ms = 0.0;
            sim.revision = sim.revision.wrapping_add(1);
        }
        if ui
            .add_enabled(
                ready && !sim.playing,
                egui::Button::new(format!("Шаг {} мс", sim.scenario.sample_ms)),
            )
            .clicked()
        {
            sim.cursor = None;
            sim.advance((sim.scenario.sample_ms / sim.scenario.dt_ms).round() as usize);
        }
        if ui.button("Сброс").clicked() {
            sim.reset(&lab.atlas);
        }
        ui.label(format!(
            "{:.0} / {:.0} мс",
            sim.display_time(),
            sim.scenario.duration_ms
        ));
        egui::ComboBox::from_id_salt("simulation-speed")
            .selected_text(format!("{} мс/с", sim.speed))
            .show_ui(ui, |ui| {
                for speed in [25.0, 100.0, 250.0] {
                    ui.selectable_value(&mut sim.speed, speed, format!("{speed} мс/с"));
                }
            });
        ui.checkbox(&mut sim.show_links, "Связи");
    });
    if let Some(error) = &sim.error {
        ui.colored_label(ERROR, error);
    }
    if sim.playing {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}
pub fn legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("E · активность модели").small());
        for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let [r, g, b] = heat_rgb(v);
            ui.colored_label(Color32::from_rgb(r, g, b), format!("{v:.2}"));
        }
        ui.label(RichText::new("Серый — без модели").small().color(MUTED));
    });
}
pub fn timeline(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let sim = &mut lab.simulation;
    let Some(engine) = &sim.engine else {
        return;
    };
    let id = lab.scene.selected_region.as_deref().unwrap_or(
        sim.scenario
            .stimuli
            .first()
            .map_or("", |s| s.region_id.as_str()),
    );
    let index = sim
        .scenario
        .populations
        .iter()
        .position(|p| p.region_id == id);
    let name = lab
        .atlas
        .region(id)
        .map_or("Выберите область с моделью", |r| {
            r.name.as_str()
        });
    ui.label(
        RichText::new(format!("{name} · E — оранжевый, I — фиолетовый"))
            .small()
            .color(MUTED),
    );
    let (outer, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 88.0), egui::Sense::click());
    let rect = Rect::from_min_max(
        outer.min + egui::vec2(30.0, 5.0),
        outer.max - egui::vec2(12.0, 20.0),
    );
    let painter = ui.painter_at(outer);
    let at = |t: f64, y: f64| {
        Pos2::new(
            rect.left() + rect.width() * (t / sim.scenario.duration_ms) as f32,
            rect.bottom() - rect.height() * y as f32,
        )
    };
    for y in [0.0, 0.5, 1.0] {
        painter.line_segment(
            [at(0.0, y), at(sim.scenario.duration_ms, y)],
            Stroke::new(1.0_f32, BORDER),
        );
        painter.text(
            at(0.0, y) - egui::vec2(6.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{y}"),
            FontId::proportional(10.0),
            MUTED,
        );
    }
    for t in [
        0.0,
        sim.scenario.duration_ms * 0.5,
        sim.scenario.duration_ms,
    ] {
        painter.text(
            at(t, 0.0) + egui::vec2(0.0, 6.0),
            if t == sim.scenario.duration_ms {
                Align2::RIGHT_TOP
            } else if t == 0.0 {
                Align2::LEFT_TOP
            } else {
                Align2::CENTER_TOP
            },
            format!("{t:.0} мс"),
            FontId::proportional(10.0),
            MUTED,
        );
    }
    if let Some(n) = index {
        for (inhibitory, color) in [(false, Color32::from_rgb(255, 178, 82)), (true, PURPLE)] {
            let points = engine
                .samples()
                .iter()
                .map(|s| {
                    at(
                        s.time_ms,
                        if inhibitory {
                            s.states[n].i
                        } else {
                            s.states[n].e
                        },
                    )
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(1.7_f32, color)));
        }
    }
    let x = at(sim.display_time(), 0.0).x;
    painter.line_segment(
        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
        Stroke::new(1.0_f32, TEXT),
    );
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64
            * sim.scenario.duration_ms;
        sim.cursor = Some(
            engine
                .samples()
                .partition_point(|s| s.time_ms <= t)
                .saturating_sub(1),
        );
        sim.playing = false;
        sim.revision = sim.revision.wrapping_add(1);
    }
    if sim.cursor.is_some() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Просмотр записанной истории")
                    .small()
                    .color(TEAL),
            );
            if ui.small_button("К текущему расчёту").clicked() {
                sim.cursor = None;
                sim.revision = sim.revision.wrapping_add(1);
            }
        });
    }
}

pub fn inspector(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let selected = lab.scene.selected_region.clone();
    let sim = &mut lab.simulation;
    tag(ui, "Демонстрационная сеть", TEAL);
    ui.label(format!(
        "{} областей · {} E/I-популяций · {} связей",
        sim.scenario.populations.len(),
        sim.scenario.populations.len() * 2,
        sim.scenario
            .connections
            .iter()
            .filter(|c| c.enabled)
            .count()
    ));
    muted(
        ui,
        "Веса и задержки заданы как допущения. Это не измеренный коннектом.",
    );
    let mut changed = false;
    if let Some(id) = &selected {
        ui.separator();
        if let Some(state) = sim.state(id) {
            ui.label(RichText::new(format!("E  {:.3}    I  {:.3}", state.e, state.i)).size(21.0));
            if ui
                .button("Импульс в выбранную")
                .on_hover_text("Сбросить время и запустить протокол с импульсом в этой области")
                .clicked()
            {
                sim.stimulate(id, &lab.atlas);
            }
        } else {
            muted(ui, "Модель области не настроена; активность неизвестна.");
        }
        ui.collapsing("Параметры популяций", |ui| {
            if let Some(p) = sim
                .scenario
                .populations
                .iter_mut()
                .find(|p| &p.region_id == id)
            {
                egui::Grid::new("population-parameters")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for (label, v, min, max) in [
                            ("τ E, мс", &mut p.tau_e_ms, 2.0, 200.0),
                            ("τ I, мс", &mut p.tau_i_ms, 2.0, 200.0),
                            ("E → E", &mut p.w_ee, 0.0, 20.0),
                            ("I → E", &mut p.w_ei, 0.0, 20.0),
                            ("E → I", &mut p.w_ie, 0.0, 20.0),
                            ("I → I", &mut p.w_ii, 0.0, 20.0),
                            ("Порог E", &mut p.threshold_e, 0.0, 20.0),
                            ("Порог I", &mut p.threshold_i, 0.0, 20.0),
                        ] {
                            ui.label(label);
                            changed |= ui
                                .add(egui::DragValue::new(v).speed(0.1).range(min..=max))
                                .changed();
                            ui.end_row();
                        }
                    });
            }
        });
        ui.collapsing("Связи области", |ui| {
            let mut remove = None;
            for (n, c) in sim
                .scenario
                .connections
                .iter_mut()
                .enumerate()
                .filter(|(_, c)| &c.from == id || &c.to == id)
            {
                ui.push_id(n, |ui| {
                    let other = if &c.from == id { &c.to } else { &c.from };
                    ui.label(format!(
                        "{} {}",
                        if &c.from == id { "→" } else { "←" },
                        lab.atlas
                            .region(other)
                            .map_or(other.as_str(), |r| r.name.as_str())
                    ));
                    ui.horizontal_wrapped(|ui| {
                        changed |= ui.checkbox(&mut c.enabled, "Вкл.").changed();
                        ui.label("w");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut c.weight)
                                    .speed(0.1)
                                    .range(-20.0..=20.0),
                            )
                            .changed();
                        ui.label("мс");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut c.delay_ms)
                                    .speed(sim.scenario.dt_ms)
                                    .range(sim.scenario.dt_ms..=200.0),
                            )
                            .changed();
                        if ui
                            .small_button("×")
                            .on_hover_text("Удалить связь")
                            .clicked()
                        {
                            remove = Some(n);
                        }
                    });
                });
            }
            if let Some(n) = remove {
                sim.scenario.connections.remove(n);
                changed = true;
            }
            if sim.scenario.populations.iter().any(|p| &p.region_id == id) {
                ui.separator();
                ui.label("Новая исходящая связь");
                sim.add_target = sim.add_target.min(sim.scenario.populations.len() - 1);
                let target = &sim.scenario.populations[sim.add_target].region_id;
                egui::ComboBox::from_id_salt("edge-target")
                    .width(ui.available_width())
                    .selected_text(
                        lab.atlas
                            .region(target)
                            .map_or(target.as_str(), |r| r.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for (n, p) in sim.scenario.populations.iter().enumerate() {
                            if &p.region_id != id {
                                ui.selectable_value(
                                    &mut sim.add_target,
                                    n,
                                    &lab.atlas.region(&p.region_id).unwrap().name,
                                );
                            }
                        }
                    });
                let target = sim.scenario.populations[sim.add_target].region_id.clone();
                let valid = target != *id
                    && !sim
                        .scenario
                        .connections
                        .iter()
                        .any(|c| c.from == *id && c.to == target);
                if ui
                    .add_enabled(valid, egui::Button::new("Добавить связь"))
                    .clicked()
                {
                    sim.scenario.connections.push(PopulationConnection {
                        from: id.clone(),
                        to: target,
                        weight: 4.0,
                        delay_ms: 24.0,
                        enabled: true,
                    });
                    changed = true;
                }
            }
        });
    }
    ui.collapsing("Стимул и протокол", |ui| {
        for s in &mut sim.scenario.stimuli {
            ui.label(
                lab.atlas
                    .region(&s.region_id)
                    .map_or(s.region_id.as_str(), |r| r.name.as_str()),
            );
            egui::Grid::new("stimulus-grid")
                .num_columns(2)
                .show(ui, |ui| {
                    for (label, v, min, max) in [
                        ("Амплитуда", &mut s.amplitude, -20.0, 20.0),
                        ("Начало, мс", &mut s.start_ms, 0.0, sim.scenario.duration_ms),
                        (
                            "Длительность, мс",
                            &mut s.duration_ms,
                            sim.scenario.dt_ms,
                            sim.scenario.duration_ms,
                        ),
                    ] {
                        ui.label(label);
                        changed |= ui
                            .add(
                                egui::DragValue::new(v)
                                    .speed(sim.scenario.dt_ms)
                                    .range(min..=max),
                            )
                            .changed();
                        ui.end_row();
                    }
                });
        }
        ui.label(format!(
            "dt = {} мс · запись {} мс · Heun",
            sim.scenario.dt_ms, sim.scenario.sample_ms
        ));
        muted(
            ui,
            "Изменение параметров сбрасывает расчёт и ставит его на паузу.",
        );
    });
    ui.collapsing("Модель и источник",|ui|{
        ui.hyperlink_to("Wilson–Cowan · Heitmann & Ermentrout, 2020",NETWORK_SOURCE_URL);
        muted(ui,"DOI 10.1371/journal.pcbi.1008164 · PMID 32877405. Уравнения (1), (2), (8) проверены в полном тексте 24.10.2026.");
        muted(ui,"Нормированная E/I-активность 0–1; не Гц и не BOLD. Проверены численный метод и причинность задержек; физиологическая валидация не выполнена.");
        muted(ui,&sim.scenario.assumptions);
    });
    if changed {
        sim.dirty = true;
        sim.reset(&lab.atlas);
    }
    if let Some(error) = &sim.error {
        ui.colored_label(ERROR, error);
    }
}

pub fn overlay(ui: &egui::Ui, rect: Rect, lab: &BrainLab) {
    if !lab.simulation.show_links {
        return;
    }
    let camera = camera_transform(lab.scene.camera);
    let project = |p: Vec3| -> Option<Pos2> {
        let p = camera.rotation.inverse() * (p - camera.translation);
        if p.z >= -0.01 {
            return None;
        }
        let tan = (super::FIELD_OF_VIEW * 0.5).tan();
        let ndc = Vec2::new(p.x / (-p.z * tan * rect.aspect_ratio()), p.y / (-p.z * tan));
        Some(
            rect.min
                + egui::vec2(
                    (ndc.x + 1.0) * 0.5 * rect.width(),
                    (1.0 - ndc.y) * 0.5 * rect.height(),
                ),
        )
    };
    let center = |id: &str| -> Option<Vec3> {
        let r = lab.atlas.region(id)?;
        if !lab.scene.visible(r) {
            return None;
        }
        let n = lab.atlas.regions.iter().position(|r| r.id == id)?;
        Some(lab.centers[n] + lab.offset(r))
    };
    let painter = ui.painter_at(rect);
    let selected = lab.scene.selected_region.as_deref();
    for c in &lab.simulation.scenario.connections {
        if !c.enabled || selected.is_some_and(|id| c.from != id && c.to != id) {
            continue;
        }
        let (Some(a), Some(b)) = (center(&c.from), center(&c.to)) else {
            continue;
        };
        let control = (a + b) * 0.5 + Vec3::Y * (0.12 + a.distance(b) * 0.16);
        let points: Option<Vec<_>> = (0..=16)
            .map(|n| {
                let t = n as f32 / 16.0;
                project((1.0 - t).powi(2) * a + 2.0 * t * (1.0 - t) * control + t * t * b)
            })
            .collect();
        if let Some(points) = points {
            let color = if c.weight < 0.0 { PURPLE } else { TEAL }
                .gamma_multiply(if selected.is_some() { 0.85 } else { 0.28 });
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(if selected.is_some() { 1.8_f32 } else { 1.0_f32 }, color),
            ));
            let end = points[15];
            let dir = (points[15] - points[13]).normalized();
            let side = egui::vec2(-dir.y, dir.x);
            painter.line_segment(
                [end - dir * 6.0 + side * 3.0, end],
                Stroke::new(1.4_f32, color),
            );
            painter.line_segment(
                [end - dir * 6.0 - side * 3.0, end],
                Stroke::new(1.4_f32, color),
            );
        }
    }
    if let Some(id) = selected
        && let Some(p) = center(id).and_then(project)
    {
        painter.circle_stroke(p, 6.0, Stroke::new(1.5_f32, TEXT));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_reset_and_reconfiguration_do_not_leak_old_activity() {
        let atlas = BrainAtlas::bundled().unwrap();
        let mut sim = SimulationLab::new(&atlas).unwrap();
        sim.mode = true;
        sim.tick(0.1);
        assert_eq!(sim.display_time(), 0.0);
        sim.playing = true;
        sim.tick(0.05);
        assert_eq!(sim.display_time(), 5.0);
        sim.playing = false;
        sim.tick(0.1);
        assert_eq!(sim.display_time(), 5.0);
        sim.scenario.populations[0].tau_e_ms = f64::NAN;
        sim.reset(&atlas);
        assert!(sim.engine.is_none());
        assert!(sim.state("FMA72976").is_none());
        sim.scenario = NetworkScenario::bundled(&atlas).unwrap();
        sim.reset(&atlas);
        assert_eq!(sim.display_time(), 0.0);
        assert!(!sim.playing);
    }
}
