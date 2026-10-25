mod pathway;
use super::{BrainLab, BrainViewport};
use crate::theme::*;
use bevy::prelude::UVec2;
use bevy_egui::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke};
use neuro_domain::{
    BrainAtlas, BrainCamera, BrainRegion, CAFFEINE_SAMPLE_MIN, CaffeineDose, CaffeineKnowledge,
    CaffeineParameters, CaffeineScenario, Error, caffeine_a1_scope, caffeine_a2a_scope,
};
use neuro_engine::{
    CaffeineNetworkComparison, CaffeineSample, CaffeineTrace, compare_caffeine_network,
    simulate_caffeine,
};
use std::path::PathBuf;

pub(super) type PlotSeries = (Color32, Vec<(f64, f64)>);

pub const BLOOD: Color32 = Color32::from_rgb(255, 178, 82);
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CaffeineLayer {
    A1,
    A2a,
    BrainProxy,
    DeltaE,
    Tissue,
    Cbf,
    Delivery,
    Territories,
}
impl CaffeineLayer {
    pub fn label(self) -> &'static str {
        match self {
            Self::A1 => "Занятость A1 · PET",
            Self::A2a => "A2A · PET в PD",
            Self::BrainProxy => "Условный Ce · мг/л",
            Self::DeltaE => "ΔE · гипотеза",
            Self::Tissue => "Кофеин в ткани · мг/л",
            Self::Cbf => "Кровоток · мл/100 г/мин",
            Self::Delivery => "Доставка · мкг/100 г/мин",
            Self::Territories => "Артериальные бассейны",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CaffeinePlot {
    Concentration,
    Occupancy,
    Mass,
    Neural,
    RegionalConcentration,
    RegionalFlow,
    RegionalFlux,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MapSelection {
    Node(usize),
    Link(usize),
}
pub struct CaffeineLab {
    pub mode: bool,
    pub scenario: CaffeineScenario,
    pub knowledge: CaffeineKnowledge,
    pub trace: Option<CaffeineTrace>,
    pub comparison: Option<CaffeineNetworkComparison>,
    pub time_min: f64,
    pub neural_time_ms: f64,
    pub playing: bool,
    pub speed_min_s: f64,
    pub layer: CaffeineLayer,
    pub plot: CaffeinePlot,
    pub route: bool,
    pub show_plot: bool,
    pub show_regions: bool,
    pub vascular_panel: bool,
    pub show_vessels: bool,
    pub tissue_opacity: f32,
    pub selected_vessel: Option<usize>,
    pub vessel_search: String,
    pub perfusion_knowledge: neuro_domain::PerfusionKnowledge,
    pub selection: Option<MapSelection>,
    pub revision: u64,
    pub error: Option<String>,
    pub neural_error: Option<String>,
    pub dirty: bool,
    pub last_path: Option<PathBuf>,
    pub source_path: Option<PathBuf>,
    pub map_pan: egui::Vec2,
    pub map_zoom: f32,
    clock_min: f64,
}
impl CaffeineLab {
    pub fn new(atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::from_scenario(CaffeineScenario::reference(atlas)?, atlas)
    }
    pub fn from_scenario(scenario: CaffeineScenario, atlas: &BrainAtlas) -> Result<Self, Error> {
        let trace = simulate_caffeine(scenario.clone(), atlas)?;
        Ok(Self {
            mode: false,
            scenario: scenario.clone(),
            knowledge: CaffeineKnowledge::bundled()?,
            trace: Some(trace),
            comparison: None,
            time_min: 0.0,
            neural_time_ms: 180.0,
            playing: false,
            speed_min_s: 30.0,
            layer: if scenario.perfusion.is_some() {
                CaffeineLayer::Tissue
            } else {
                CaffeineLayer::A1
            },
            plot: if scenario.perfusion.is_some() {
                CaffeinePlot::RegionalConcentration
            } else {
                CaffeinePlot::Concentration
            },
            route: false,
            show_plot: false,
            show_regions: false,
            vascular_panel: true,
            show_vessels: true,
            tissue_opacity: 0.22,
            selected_vessel: None,
            vessel_search: String::new(),
            perfusion_knowledge: neuro_domain::PerfusionKnowledge::bundled()?,
            selection: Some(MapSelection::Node(2)),
            revision: 0,
            error: None,
            neural_error: None,
            dirty: false,
            last_path: None,
            source_path: None,
            map_pan: egui::Vec2::ZERO,
            map_zoom: 1.0,
            clock_min: 0.0,
        })
    }
    pub fn time_grid(&self, t: f64) -> f64 {
        if self.scenario.perfusion.is_some() && t <= 5.0 {
            0.05
        } else {
            CAFFEINE_SAMPLE_MIN
        }
    }
    pub fn regional(
        &self,
        id: &str,
    ) -> Option<(
        &neuro_domain::RegionalPerfusion,
        neuro_engine::RegionalPerfusionSample,
    )> {
        let config = self.scenario.perfusion.as_ref()?;
        let trace = self.trace.as_ref()?.perfusion.as_ref()?;
        let i = config.regions.iter().position(|r| r.region_id == id)?;
        Some((&config.regions[i], trace.at(self.time_min).regions[i]))
    }
    pub fn refresh(&mut self, atlas: &BrainAtlas) {
        self.playing = false;
        self.comparison = None;
        self.neural_error = None;
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
        match simulate_caffeine(self.scenario.clone(), atlas) {
            Ok(trace) => {
                self.time_min = trace
                    .at(self.time_min.min(self.scenario.duration_min))
                    .time_min;
                self.clock_min = self.time_min;
                self.trace = Some(trace);
                self.error = None;
            }
            Err(error) => {
                self.trace = None;
                self.error = Some(error.to_string());
            }
        }
    }
    pub fn sample(&self) -> Option<CaffeineSample> {
        self.trace.as_ref().map(|t| *t.at(self.time_min))
    }
    pub fn set_time(&mut self, time_min: f64) {
        if !time_min.is_finite() {
            return;
        }
        let grid = self.time_grid(time_min);
        let t = (time_min.clamp(0.0, self.scenario.duration_min) / grid).round() * grid;
        if t != self.time_min {
            self.time_min = t;
            self.comparison = None;
            self.neural_error = None;
            self.revision = self.revision.wrapping_add(1);
        }
        self.clock_min = t;
    }
    pub fn tick(&mut self, seconds: f64) {
        if !self.mode || !self.playing || self.trace.is_none() {
            return;
        }
        self.clock_min = (self.clock_min + seconds.clamp(0.0, 0.1) * self.speed_min_s)
            .min(self.scenario.duration_min);
        let grid = self.time_grid(self.clock_min);
        let t = (self.clock_min / grid).floor() * grid;
        if t != self.time_min {
            self.time_min = t;
            self.comparison = None;
            self.neural_error = None;
            self.revision = self.revision.wrapping_add(1);
        }
        if self.clock_min >= self.scenario.duration_min {
            self.playing = false;
        }
    }
    pub fn compute_neural(&mut self, atlas: &BrainAtlas) {
        self.playing = false;
        self.comparison = None;
        self.neural_error = None;
        if let Some(trace) = &self.trace {
            match compare_caffeine_network(trace, self.time_min, atlas) {
                Ok(c) => {
                    self.neural_time_ms = 180.0_f64.min(c.baseline.elapsed_ms);
                    self.comparison = Some(c);
                    self.layer = CaffeineLayer::DeltaE;
                    self.route = false;
                    self.plot = CaffeinePlot::Neural;
                    self.show_plot = true;
                    self.selection = None;
                }
                Err(e) => self.neural_error = Some(e.to_string()),
            }
        }
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn neural_values(
        &self,
        id: &str,
    ) -> Option<(neuro_domain::PopulationState, neuro_domain::PopulationState)> {
        let c = self.comparison.as_ref()?;
        if c.pk_time_min != self.time_min {
            return None;
        }
        let n = c
            .baseline
            .scenario
            .populations
            .iter()
            .position(|p| p.region_id == id)?;
        let i = ((self.neural_time_ms / c.baseline.scenario.sample_ms).round() as usize)
            .min(c.baseline.samples.len() - 1);
        Some((
            c.baseline.samples[i].states[n],
            c.caffeine_hypothesis.samples[i].states[n],
        ))
    }
    pub fn region_value(&self, r: &BrainRegion) -> Option<f64> {
        let s = self.sample()?;
        match self.layer {
            CaffeineLayer::A1 => caffeine_a1_scope(r).then_some(s.a1_occupancy),
            CaffeineLayer::A2a => caffeine_a2a_scope(r).then_some(s.a2a_pd_occupancy),
            CaffeineLayer::BrainProxy => caffeine_a1_scope(r).then_some(s.brain_proxy_mg_l),
            CaffeineLayer::DeltaE => self.neural_values(&r.id).map(|(b, c)| c.e - b.e),
            CaffeineLayer::Tissue => self.regional(&r.id).map(|(_, s)| s.tissue_mg_l),
            CaffeineLayer::Cbf => self.regional(&r.id).map(|(_, s)| s.cbf_ml_100g_min),
            CaffeineLayer::Delivery => self
                .regional(&r.id)
                .map(|(_, s)| s.cbf_ml_100g_min * s.arterial_mg_l),
            CaffeineLayer::Territories => None,
        }
    }
    pub fn region_rgb(&self, r: &BrainRegion) -> [u8; 3] {
        if self.layer == CaffeineLayer::Territories {
            return self.regional(&r.id).map_or([65, 69, 77], |(r, _)| {
                super::vascular::territory_rgb(r.territory)
            });
        }
        self.region_value(r).map_or([65, 69, 77], |v| {
            if self.layer == CaffeineLayer::DeltaE {
                let base = [50.0, 61.0, 76.0];
                let target = if v >= 0.0 {
                    [255.0, 178.0, 82.0]
                } else {
                    [185.0, 161.0, 244.0]
                };
                std::array::from_fn(|i| {
                    (base[i] + v.abs().min(1.0).sqrt() * (target[i] - base[i])) as u8
                })
            } else {
                super::simulation::heat_rgb(match self.layer {
                    CaffeineLayer::BrainProxy | CaffeineLayer::Tissue => v / 10.0,
                    CaffeineLayer::Cbf => v / 100.0,
                    CaffeineLayer::Delivery => v / 300.0,
                    _ => v,
                })
            }
        })
    }
    pub fn node_value(&self, id: &str) -> String {
        let Some(s) = self.sample() else {
            return "нет расчёта".into();
        };
        match id {
            "dose" => format!("{:.0} мг принято", s.administered_mg),
            "gut" => format!("{:.2} мг", s.gut_mg),
            "blood" => format!("{:.3} мг/л", s.plasma_mg_l),
            "brain" => format!("{:.3} мг/л *", s.brain_proxy_mg_l),
            "a1" => format!("{:.1}% · оценка", 100.0 * s.a1_occupancy),
            "a2a" => format!("{:.1}% · PD, n=5", 100.0 * s.a2a_pd_occupancy),
            "adenosine" => "нет концентрации".into(),
            "liver" => format!(
                "{:.3} мг/мин",
                self.scenario.parameters.ke() * s.systemic_mg
            ),
            "metabolites" => "нет концентраций".into(),
            "removed" => format!("{:.2} мг исходного", s.eliminated_parent_mg),
            "network" => {
                if self.comparison.is_some() {
                    format!("снимок {:.0} мин", self.time_min)
                } else {
                    "нет снимка E/I".into()
                }
            }
            _ => String::new(),
        }
    }
}
fn hours(t: f64) -> String {
    format!("{:02}:{:04.1}", (t / 60.0).floor() as u32, t % 60.0)
}
pub(super) fn parameter(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) {
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.add_sized(
            [(width - 90.0).max(60.0), 30.0],
            egui::Label::new(label).wrap(),
        )
        .on_hover_text(match label {
            "Vd, л/кг" => "Кажущийся объём распределения на килограмм массы; не объём крови.",
            "Биодоступность F" => {
                "Доля всасывающейся дозы, достигающая системной камеры: 0–1."
            }
            "t½ всасыв., мин" => "За это время половина массы покидает депо ЖКТ.",
            "t½ элимин., ч" => {
                "Полувремя элиминации исходного кофеина после завершения всасывания."
            }
            "t½ Ce, мин" => {
                "Условное полувремя уравновешивания места эффекта; измерений ткани здесь нет."
            }
            _ => label,
        });
        ui.add_sized(
            [60.0, 28.0],
            egui::DragValue::new(value)
                .range(range)
                .speed(speed)
                .max_decimals(2),
        );
    });
}
pub fn sidebar(ui: &mut egui::Ui, lab: &mut BrainLab) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut lab.caffeine.vascular_panel, true, "Кровоснабжение");
        ui.selectable_value(&mut lab.caffeine.vascular_panel, false, "Протокол");
    });
    if lab.caffeine.vascular_panel {
        super::vascular::sidebar(ui, lab);
        return;
    }
    let c = &mut lab.caffeine;
    let before = c.scenario.clone();
    egui::ScrollArea::vertical().id_salt("caffeine-protocol-scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width());
        ui.heading("Кофеин");
        muted(ui, "Пероральный приём · мг");
        ui.add_space(8.0);
        let mut remove = None;
        egui::Grid::new("caffeine-doses").num_columns(3).spacing([5.0, 5.0]).show(ui, |ui| {
            ui.small("Время, мин");
            ui.small("Кофеин, мг");
            ui.label("");
            ui.end_row();
            for (i, d) in c.scenario.doses.iter_mut().enumerate() {
                ui.add_sized([62.0, 28.0], egui::DragValue::new(&mut d.time_min).range(0.0..=c.scenario.duration_min).speed(0.5).max_decimals(1));
                ui.add_sized([65.0, 28.0], egui::DragValue::new(&mut d.amount_mg).range(0.0..=500.0).speed(1.0).max_decimals(1));
                if ui.small_button("×").on_hover_text("Удалить приём").clicked() { remove = Some(i); }
                ui.end_row();
            }
        });
        if let Some(i) = remove { c.scenario.doses.remove(i); }
        if ui.add_enabled(c.scenario.doses.len() < 32, egui::Button::new("Добавить приём")).clicked() {
            let t = c.scenario.doses.iter().map(|d| d.time_min).fold(0.0, f64::max);
            c.scenario.doses.push(CaffeineDose { time_min: (t + 240.0).min(c.scenario.duration_min), amount_mg: 100.0 });
        }
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("Без кофеина").clicked() { c.scenario.doses.clear(); }
            if ui.small_button("Пример 100 мг").clicked() {
                c.scenario.doses = vec![CaffeineDose { time_min: 0.0, amount_mg: 100.0 }];
                c.scenario.parameters = CaffeineParameters::default();
            }
        });
        ui.add_space(10.0);
        ui.separator();
        ui.strong("Фармакокинетика");
        let p = &mut c.scenario.parameters;
        parameter(ui, "Масса, кг", &mut p.body_mass_kg, 30.0..=200.0, 0.5);
        parameter(ui, "Vd, л/кг", &mut p.distribution_l_kg, 0.4..=1.0, 0.01);
        parameter(ui, "Биодоступность F", &mut p.bioavailability, 0.0..=1.0, 0.01);
        parameter(ui, "t½ всасыв., мин", &mut p.absorption_half_min, 0.5..=60.0, 0.5);
        parameter(ui, "t½ элимин., ч", &mut p.elimination_half_h, 1.0..=20.0, 0.1);
        parameter(ui, "t½ Ce, мин", &mut p.brain_equilibration_half_min, 0.5..=60.0, 0.5);
        muted(ui, format!("Vd = {:.1} л · CL = {:.2} л/ч", p.volume_l(), p.clearance_l_h()));
        muted(ui, "Vd — кажущийся объём, не объём крови. Ce и его задержка — допущение.");
        ui.horizontal(|ui| {
            ui.label("Горизонт");
            egui::ComboBox::from_id_salt("caffeine-horizon").selected_text(format!("{:.0} ч", c.scenario.duration_min / 60.0)).show_ui(ui, |ui| {
                for h in [12.0, 24.0, 48.0] { ui.selectable_value(&mut c.scenario.duration_min, h * 60.0, format!("{h} ч")); }
            });
        });
        ui.add_space(8.0);
        ui.collapsing("Связь с E/I-популяциями", |ui| {
            ui.checkbox(&mut c.scenario.neural_hypothesis.enabled, "Включить гипотезу");
            muted(ui, "Коэффициенты не калиброваны. Это не оценка бодрости или безопасности.");
            parameter(ui, "Вход A1, g1", &mut c.scenario.neural_hypothesis.a1_drive_gain, -5.0..=5.0, 0.05);
            parameter(ui, "A2A стриатум, g2", &mut c.scenario.neural_hypothesis.a2a_striatal_drive_gain, -5.0..=5.0, 0.05);
            if ui.button("Взять текущую сеть E/I").clicked() { c.scenario.neural_hypothesis.network = lab.simulation.scenario.clone(); }
            muted(ui, format!("{} областей · сеть illustrative", c.scenario.neural_hypothesis.network.populations.len()));
        });
        ui.add_space(8.0);
        muted(ui, "Изменения пересчитывают PK и ставят воспроизведение на паузу. Пределы ввода технические.");
    });
    if c.scenario != before {
        c.refresh(&lab.atlas);
    }
}
pub fn transport(ui: &mut egui::Ui, c: &mut CaffeineLab) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                c.trace.is_some(),
                egui::Button::new(if c.playing {
                    "Пауза"
                } else {
                    "Запуск"
                }),
            )
            .clicked()
        {
            if c.time_min >= c.scenario.duration_min {
                c.set_time(0.0);
            }
            c.playing = !c.playing;
        }
        if ui
            .add_enabled(c.trace.is_some(), egui::Button::new("+15 мин"))
            .clicked()
        {
            c.playing = false;
            c.set_time(c.time_min + 15.0);
        }
        if ui.button("К началу").clicked() {
            c.playing = false;
            c.set_time(0.0);
        }
        let mut time = c.time_min;
        if ui
            .add(
                egui::DragValue::new(&mut time)
                    .range(0.0..=c.scenario.duration_min)
                    .speed(0.05)
                    .max_decimals(2)
                    .suffix(" мин"),
            )
            .changed()
        {
            c.playing = false;
            c.set_time(time);
        }
        ui.label(format!(
            "{} / {:.0} ч",
            hours(c.time_min),
            c.scenario.duration_min / 60.0
        ));
        egui::ComboBox::from_id_salt("caffeine-speed")
            .selected_text(format!("{} мин/с", c.speed_min_s))
            .show_ui(ui, |ui| {
                for speed in [0.05, 0.5, 5.0, 15.0, 30.0, 60.0] {
                    ui.selectable_value(&mut c.speed_min_s, speed, format!("{speed} мин/с"));
                }
            });
    });
    let mut t = c.time_min;
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(50.0);
            ui.add(
                egui::Slider::new(&mut t, 0.0..=c.scenario.duration_min)
                    .show_value(false)
                    .step_by(0.05),
            )
        })
        .inner;
    if response
        .on_hover_text("Время сценария; первые 5 мин записаны через 3 с, далее через 30 с")
        .changed()
    {
        c.playing = false;
        c.set_time(t);
    }
    if c.playing {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
    if let Some(e) = &c.error {
        ui.colored_label(ERROR, e);
    }
}
pub fn viewport(ui: &mut egui::Ui, lab: &mut BrainLab, viewport: &BrainViewport) {
    let scene_before = lab.scene.clone();
    transport(ui, &mut lab.caffeine);
    let route_before = lab.caffeine.route;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut lab.caffeine.route, false, "3D-мозг");
        ui.selectable_value(&mut lab.caffeine.route, true, "Путь вещества");
        if !lab.caffeine.route {
            let before = lab.caffeine.layer;
            egui::ComboBox::from_id_salt("caffeine-layer")
                .selected_text(lab.caffeine.layer.label())
                .show_ui(ui, |ui| {
                    for l in [
                        CaffeineLayer::Tissue,
                        CaffeineLayer::Cbf,
                        CaffeineLayer::Delivery,
                        CaffeineLayer::Territories,
                        CaffeineLayer::A1,
                        CaffeineLayer::A2a,
                        CaffeineLayer::BrainProxy,
                        CaffeineLayer::DeltaE,
                    ] {
                        ui.selectable_value(&mut lab.caffeine.layer, l, l.label());
                    }
                });
            if before != lab.caffeine.layer {
                lab.caffeine.revision = lab.caffeine.revision.wrapping_add(1);
            }
        }
        ui.checkbox(&mut lab.caffeine.show_plot, "График");
    });
    if lab.caffeine.route && !route_before {
        lab.caffeine.show_plot = false;
    }
    if !lab.caffeine.route {
        super::vascular::view_controls(ui, lab);
    }
    let height =
        (ui.available_height() - if lab.caffeine.show_plot { 245.0 } else { 80.0 }).max(100.0);
    if lab.caffeine.route {
        pathway::show(ui, &mut lab.caffeine, height);
    } else {
        if !lab.caffeine.show_vessels {
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("Слева").clicked() {
                    lab.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
                    lab.scene.camera.pitch = 0.0;
                }
                if ui.small_button("По центру").clicked() {
                    lab.scene.camera = BrainCamera::default();
                }
                if ui
                    .small_button("Стриатум")
                    .on_hover_text("Показать внутренние структуры, где применена модель A2A")
                    .clicked()
                {
                    lab.scene.layers.cortex = false;
                    lab.scene.layers.deep = true;
                    lab.scene.isolate = false;
                    lab.scene.hidden_regions.clear();
                    lab.scene.camera = BrainCamera::default();
                }
                let mut sep = lab.scene.separation;
                if ui
                    .add_enabled(
                        !lab.caffeine.show_vessels,
                        egui::Slider::new(&mut sep, 0.0..=1.0)
                            .show_value(false)
                            .text("Полушария"),
                    )
                    .changed()
                {
                    lab.scene.separation = sep;
                }
            });
        }
        let size = egui::vec2(
            ui.available_width(),
            (height - if lab.caffeine.show_vessels { 0.0 } else { 30.0 }).max(100.0),
        );
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
        super::controls::interact(ui, &response, lab);
        if response.clicked() {
            lab.caffeine.selection = None;
        }
        if let Some(r) = lab.selected()
            && lab.scene.visible(r)
            && let Some(n) = lab.atlas.regions.iter().position(|p| p.id == r.id)
        {
            let camera = super::camera_transform(lab.scene.camera);
            let local =
                camera.rotation.inverse() * (lab.centers[n] + lab.offset(r) - camera.translation);
            if local.z < -0.01 {
                let tan = (super::FIELD_OF_VIEW * 0.5).tan();
                let aspect = response.rect.width() / response.rect.height();
                let ndc = egui::vec2(
                    local.x / (-local.z * tan * aspect),
                    -local.y / (-local.z * tan),
                );
                let point = response.rect.center() + ndc * response.rect.size() * 0.5;
                if response.rect.contains(point) {
                    let painter = ui.painter_at(response.rect);
                    painter.circle_stroke(point, 7.0, Stroke::new(1.6_f32, TEAL));
                    painter.circle_stroke(point, 3.0, Stroke::new(1.0_f32, TEXT));
                }
            }
        }
        legend(ui, &lab.caffeine);
        if !matches!(
            lab.caffeine.layer,
            CaffeineLayer::Tissue
                | CaffeineLayer::Cbf
                | CaffeineLayer::Delivery
                | CaffeineLayer::Territories
        ) {
            muted(
                ui,
                match lab.caffeine.layer {
                    CaffeineLayer::A1 => "Общая PET-оценка A1, региональные плотности неизвестны.",
                    CaffeineLayer::A2a => "Стриатальная A2A: PET в PD, n=5.",
                    CaffeineLayer::BrainProxy => {
                        "Общий условный Ce; местные Ct показаны в слое кофеина в ткани."
                    }
                    _ => {
                        "ΔE: два протокола с/без входа по местной Ct (или Ce), непроверенная гипотеза."
                    }
                },
            );
        }
    }
    ui.add_space(3.0);
    if lab.caffeine.show_plot {
        plot(ui, lab);
    }
    lab.dirty |= lab.scene != scene_before;
    ui.label(
        RichText::new("BodyParts3D © DBCLS · CC BY 4.0 · Перфузия: модель с допущениями")
            .size(10.5)
            .color(MUTED),
    );
}
fn legend(ui: &mut egui::Ui, c: &CaffeineLab) {
    if super::vascular::legend(ui, c) {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.small(c.layer.label());
        if c.layer == CaffeineLayer::DeltaE {
            ui.colored_label(PURPLE, "−1");
            ui.colored_label(MUTED, "0");
            ui.colored_label(BLOOD, "+1");
        } else {
            for f in [0.0, 0.5, 1.0] {
                let rgb = super::simulation::heat_rgb(f);
                ui.colored_label(
                    Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                    if c.layer == CaffeineLayer::BrainProxy {
                        format!("{:.0}{}", f * 10.0, if f == 1.0 { "+ мг/л" } else { "" })
                    } else {
                        format!("{:.0}%", f * 100.0)
                    },
                );
            }
        }
        ui.label(
            RichText::new("Серый — оценка не задана")
                .small()
                .color(MUTED),
        );
    });
}
fn plot_label(p: CaffeinePlot) -> &'static str {
    match p {
        CaffeinePlot::RegionalConcentration => "Кофеин в ткани",
        CaffeinePlot::RegionalFlow => "Кровоток",
        CaffeinePlot::RegionalFlux => "Приток / отток",
        CaffeinePlot::Concentration => "Плазма и Ce",
        CaffeinePlot::Occupancy => "Занятость A1 / A2A",
        CaffeinePlot::Mass => "Общий баланс массы",
        CaffeinePlot::Neural => "Сравнение E/I",
    }
}
fn plot(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.caffeine;
    ui.horizontal(|ui| {
        ui.label("График");
        egui::ComboBox::from_id_salt("caffeine-plot-kind")
            .selected_text(plot_label(c.plot))
            .show_ui(ui, |ui| {
                for p in [
                    CaffeinePlot::RegionalConcentration,
                    CaffeinePlot::RegionalFlow,
                    CaffeinePlot::RegionalFlux,
                    CaffeinePlot::Concentration,
                    CaffeinePlot::Occupancy,
                    CaffeinePlot::Mass,
                    CaffeinePlot::Neural,
                ] {
                    ui.selectable_value(&mut c.plot, p, plot_label(p));
                }
            });
        if matches!(
            c.plot,
            CaffeinePlot::RegionalConcentration
                | CaffeinePlot::RegionalFlow
                | CaffeinePlot::RegionalFlux
        ) {
            let early = c.time_min <= 5.0;
            if ui
                .small_button(if early {
                    "Полный горизонт"
                } else {
                    "Первые 5 мин"
                })
                .clicked()
            {
                c.playing = false;
                c.set_time(if early { 60.0 } else { 1.0 });
            }
        }
    });
    if matches!(
        c.plot,
        CaffeinePlot::RegionalConcentration
            | CaffeinePlot::RegionalFlow
            | CaffeinePlot::RegionalFlux
    ) {
        super::vascular::plot(ui, c, lab.scene.selected_region.as_deref());
        return;
    }
    if c.plot == CaffeinePlot::Neural {
        neural_plot(ui, c, lab.scene.selected_region.as_deref(), &lab.atlas);
        return;
    }
    let Some(trace) = &c.trace else {
        muted(ui, "Исправьте параметры, чтобы вернуть расчёт.");
        return;
    };
    let (title, ymax, lines): (&str, f64, Vec<PlotSeries>) = match c.plot {
        CaffeinePlot::Concentration => {
            let max = trace
                .samples
                .iter()
                .map(|s| s.plasma_mg_l.max(s.brain_proxy_mg_l))
                .fold(0.0, f64::max)
                .max(0.1);
            (
                "мг/л · плазма — оранжевый, условный Ce — бирюзовый",
                (max * 10.0).ceil() / 10.0,
                vec![
                    (
                        BLOOD,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.plasma_mg_l))
                            .collect(),
                    ),
                    (
                        TEAL,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.brain_proxy_mg_l))
                            .collect(),
                    ),
                ],
            )
        }
        CaffeinePlot::Occupancy => (
            "% занятости · A1 — оранжевый, A2A (PD, n=5) — фиолетовый",
            100.0,
            vec![
                (
                    BLOOD,
                    trace
                        .samples
                        .iter()
                        .map(|s| (s.time_min, 100.0 * s.a1_occupancy))
                        .collect(),
                ),
                (
                    PURPLE,
                    trace
                        .samples
                        .iter()
                        .map(|s| (s.time_min, 100.0 * s.a2a_pd_occupancy))
                        .collect(),
                ),
            ],
        ),
        _ => {
            let max = c
                .scenario
                .doses
                .iter()
                .map(|d| d.amount_mg)
                .sum::<f64>()
                .max(1.0);
            (
                "мг · ЖКТ — фиолетовый, системно — оранжевый, убыль — бирюзовый, неусвоено — серый",
                max,
                vec![
                    (
                        PURPLE,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.gut_mg))
                            .collect(),
                    ),
                    (
                        BLOOD,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.systemic_mg))
                            .collect(),
                    ),
                    (
                        TEAL,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.eliminated_parent_mg))
                            .collect(),
                    ),
                    (
                        MUTED,
                        trace
                            .samples
                            .iter()
                            .map(|s| (s.time_min, s.unavailable_mg))
                            .collect(),
                    ),
                ],
            )
        }
    };
    muted(ui, title);
    let picked = chart(
        ui,
        "caffeine-pk-plot",
        c.scenario.duration_min,
        ymax,
        c.time_min,
        &lines,
        "h",
    );
    if let Some(t) = picked {
        c.playing = false;
        c.set_time(t);
    }
}
pub(super) fn chart(
    ui: &mut egui::Ui,
    id: &str,
    xmax: f64,
    ymax: f64,
    cursor: f64,
    lines: &[PlotSeries],
    axis: &str,
) -> Option<f64> {
    let (outer, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 90.0),
        egui::Sense::click_and_drag(),
    );
    let rect = Rect::from_min_max(
        outer.min + egui::vec2(42.0, 7.0),
        outer.max - egui::vec2(12.0, 23.0),
    );
    let painter = ui.painter_at(outer);
    let at = |x: f64, y: f64| {
        Pos2::new(
            rect.left() + rect.width() * (x / xmax) as f32,
            rect.bottom() - rect.height() * (y / ymax) as f32,
        )
    };
    for y in [0.0, ymax * 0.5, ymax] {
        painter.line_segment([at(0.0, y), at(xmax, y)], Stroke::new(1.0_f32, BORDER));
        painter.text(
            at(0.0, y) - egui::vec2(6.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{y:.1}"),
            FontId::proportional(10.0),
            MUTED,
        );
    }
    for t in [0.0, xmax * 0.25, xmax * 0.5, xmax * 0.75, xmax] {
        painter.text(
            at(t, 0.0) + egui::vec2(0.0, 5.0),
            if t == 0.0 {
                Align2::LEFT_TOP
            } else if t == xmax {
                Align2::RIGHT_TOP
            } else {
                Align2::CENTER_TOP
            },
            match axis {
                "h" => format!("{:.0} ч", t / 60.0),
                "min" => format!("{t:.1} мин"),
                _ => format!("{t:.0} мс"),
            },
            FontId::proportional(10.0),
            MUTED,
        );
    }
    for (color, series) in lines {
        painter.add(egui::Shape::line(
            series.iter().map(|(x, y)| at(*x, *y)).collect(),
            Stroke::new(1.8_f32, *color),
        ));
    }
    if cursor.is_finite() && (0.0..=xmax).contains(&cursor) {
        painter.line_segment(
            [at(cursor, 0.0), at(cursor, ymax)],
            Stroke::new(1.0_f32, TEXT),
        );
    }
    response.clone().on_hover_text(if axis != "ms" {
        "Щелчок или перетаскивание выбирает время PK"
    } else {
        "Выбор миллисекунды нейронного сравнения"
    });
    let picked = if (response.clicked() || response.dragged())
        && let Some(p) = response.interact_pointer_pos()
    {
        Some(((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * xmax)
    } else {
        None
    };
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(id), rect));
    picked
}
fn neural_plot(ui: &mut egui::Ui, c: &mut CaffeineLab, id: Option<&str>, atlas: &BrainAtlas) {
    let Some(comparison) = &c.comparison else {
        muted(
            ui,
            "Включите гипотезу в протоколе и нажмите «Сравнить E/I» справа. Два расчёта используют одинаковый нейронный протокол.",
        );
        return;
    };
    let network = &comparison.baseline.scenario;
    let id = id.unwrap_or_else(|| {
        network
            .populations
            .first()
            .map_or("", |p| p.region_id.as_str())
    });
    let Some(n) = network.populations.iter().position(|p| p.region_id == id) else {
        muted(ui, "Для этой ткани нейронная популяция не задана.");
        return;
    };
    muted(
        ui,
        format!(
            "{} · PK {:.1} мин · E без входа — серый, E с гипотезой — оранжевый, I — фиолетовый",
            atlas.region(id).map_or(id, |r| r.name.as_str()),
            comparison.pk_time_min
        ),
    );
    let lines = vec![
        (
            MUTED,
            comparison
                .baseline
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].e))
                .collect(),
        ),
        (
            BLOOD,
            comparison
                .caffeine_hypothesis
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].e))
                .collect(),
        ),
        (
            PURPLE,
            comparison
                .caffeine_hypothesis
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].i))
                .collect(),
        ),
    ];
    if let Some(t) = chart(
        ui,
        "caffeine-neural-plot",
        network.duration_ms,
        1.0,
        c.neural_time_ms,
        &lines,
        "ms",
    ) {
        c.neural_time_ms = (t / network.sample_ms).round() * network.sample_ms;
        c.revision = c.revision.wrapping_add(1);
    }
}
pub fn inspector(ui: &mut egui::Ui, lab: &mut BrainLab) {
    if super::vascular::inspector(ui, lab) {
        return;
    }
    let c = &mut lab.caffeine;
    tag(ui, "Кофеин · PK и PET-оценки", TEAL);
    ui.add_space(7.0);
    if let Some(s) = c.sample() {
        egui::Grid::new("caffeine-current")
            .num_columns(2)
            .spacing([12.0, 5.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("Время PK", format!("{} ч:мин", hours(s.time_min))),
                    ("Общая плазма", format!("{:.3} мг/л", s.plasma_mg_l)),
                    ("Ce (допущение)", format!("{:.3} мг/л", s.brain_proxy_mg_l)),
                    (
                        "A1 · общая PET-оценка",
                        format!("{:.1}%", 100.0 * s.a1_occupancy),
                    ),
                    (
                        "A2A · PD, n=5",
                        format!("{:.1}%", 100.0 * s.a2a_pd_occupancy),
                    ),
                ] {
                    ui.label(label);
                    ui.strong(value);
                    ui.end_row();
                }
            });
        if s.plasma_mg_l > 9.0 {
            ui.colored_label(PURPLE, "A1: концентрация выше наблюдённой области PET.");
        }
        if s.plasma_mg_l > 0.0 && !(4.36..=8.94).contains(&s.plasma_mg_l) {
            ui.label(
                RichText::new("A2A: экстраполяция за диапазон нагрузок исследования PD.")
                    .small()
                    .color(PURPLE),
            );
        }
        if let Some(t) = &c.trace {
            let peak = t.peak();
            if peak.plasma_mg_l > 0.0 {
                muted(
                    ui,
                    format!(
                        "Пик на сетке: {:.3} мг/л в {:.1} мин",
                        peak.plasma_mg_l, peak.time_min
                    ),
                );
            } else {
                muted(
                    ui,
                    "Плазма равна 0 на всём горизонте; время пика не определено.",
                );
            }
        }
    }
    ui.add_space(7.0);
    if ui
        .checkbox(
            &mut c.scenario.neural_hypothesis.enabled,
            "Гипотеза связи с E/I",
        )
        .changed()
    {
        c.refresh(&lab.atlas);
    }
    if ui
        .add_enabled(
            c.scenario.neural_hypothesis.enabled && c.trace.is_some(),
            egui::Button::new("Сравнить E/I при этом времени"),
        )
        .clicked()
    {
        c.compute_neural(&lab.atlas);
    }
    muted(
        ui,
        "E/I — дополнительная гипотеза; коэффициенты в протоколе.",
    );
    ui.add_space(7.0);
    ui.separator();
    let mut sources = Vec::new();
    if let Some(selection) = c.selection {
        let (title, text, kind, ids) = match selection {
            MapSelection::Node(n) => {
                let item = &c.knowledge.nodes[n];
                (&item.title, &item.explanation, &item.kind, &item.source_ids)
            }
            MapSelection::Link(n) => {
                let item = &c.knowledge.links[n];
                (&item.title, &item.explanation, &item.kind, &item.source_ids)
            }
        };
        ui.heading(title);
        tag(ui, kind_label(kind), kind_color(kind));
        ui.add_space(7.0);
        ui.label(text);
        sources = ids.clone();
    } else if let Some(r) = lab
        .scene
        .selected_region
        .as_deref()
        .and_then(|id| lab.atlas.region(id))
    {
        ui.heading(&r.name);
        muted(ui, &r.id);
        match c.layer {
            CaffeineLayer::A1 => {
                ui.label("Перенос общей PET-оценки A1 на анатомию. Значение не измерено отдельно для этой области; плотность рецепторов неизвестна.");
                sources.push("elmenhorst2012".into());
            }
            CaffeineLayer::A2a => {
                ui.label(if caffeine_a2a_scope(r){"Область отображения стриатальной PET-модели A2A (PD, n=5). Это не отдельное региональное измерение."}else{"Для этой структуры количественная оценка A2A не задана. Серый цвет не означает отсутствие рецептора или эффекта."});
                sources.push("ishibashi2022".into());
            }
            CaffeineLayer::Tissue
            | CaffeineLayer::Cbf
            | CaffeineLayer::Delivery
            | CaffeineLayer::Territories => {
                ui.label("Выберите участок с заданной перфузией.");
            }
            CaffeineLayer::BrainProxy => {
                ui.label("Одинаковый условный Ce во всём отображаемом сером веществе. Региональная доставка кофеина неизвестна.");
            }
            CaffeineLayer::DeltaE => {
                if let Some((b, e)) = c.neural_values(&r.id) {
                    ui.label(format!("Нейронное время {:.0} мс\nE без входа {:.4} → с гипотезой {:.4}\nΔE {:+.4}\nI без входа {:.4} → с гипотезой {:.4}",c.neural_time_ms,b.e,e.e,e.e-b.e,b.i,e.i));
                } else {
                    muted(
                        ui,
                        "Нет актуального сравнения для выбранного времени и области.",
                    );
                }
            }
        }
        if ui.button("Приблизить область").clicked() {
            lab.focus_selected();
            return;
        }
    } else {
        muted(ui, "Выберите область на мозге или узел/связь на карте.");
    }
    for id in sources {
        if let Some(s) = c.knowledge.sources.iter().find(|s| s.id == id) {
            ui.add_space(8.0);
            ui.hyperlink_to(&s.title, &s.url);
            muted(ui, &s.context);
            muted(ui, &s.verification);
        }
    }
    ui.add_space(10.0);
    ui.separator();
    if let Some(e) = &c.neural_error {
        ui.colored_label(ERROR, e);
    }
    if c.comparison.is_some() {
        ui.horizontal(|ui| {
            ui.label("Время сети, мс");
            if ui
                .add(
                    egui::DragValue::new(&mut c.neural_time_ms)
                        .range(0.0..=c.scenario.neural_hypothesis.network.duration_ms)
                        .speed(2.0),
                )
                .changed()
            {
                c.neural_time_ms =
                    (c.neural_time_ms / c.scenario.neural_hypothesis.network.sample_ms).round()
                        * c.scenario.neural_hypothesis.network.sample_ms;
                c.revision = c.revision.wrapping_add(1);
            }
        });
        tag(ui, "Непроверенная гипотеза E/I", PURPLE);
    }
    ui.collapsing("Масса и потоки",|ui|{if let Some(s)=c.sample(){ui.label(format!("Принято {:.3} мг\nЖКТ {:.3} мг\nСистемная масса {:.3} мг\nУбыль исходного {:.3} мг\nНеусвоено {:.3} мг\nВсасывание {:.4} мг/мин\nЭлиминация {:.4} мг/мин\nAUC до кадра {:.3} мг·ч/л",s.administered_mg,s.gut_mg,s.systemic_mg,s.eliminated_parent_mg,s.unavailable_mg,c.scenario.parameters.bioavailability*c.scenario.parameters.ka()*s.gut_mg,c.scenario.parameters.ke()*s.systemic_mg,s.auc_mg_min_l/60.0));}});
    ui.collapsing("Все источники и границы", |ui| {
        muted(ui, &c.knowledge.species);
        for s in &c.knowledge.sources {
            ui.hyperlink_to(&s.title, &s.url);
            muted(ui, &s.verification);
            muted(ui, &s.context);
            muted(ui, format!("DOI {} · {}", s.doi, s.rights));
            ui.add_space(5.0);
        }
        muted(ui, &c.knowledge.rights);
    });
    ui.add_space(9.0);
    muted(
        ui,
        "Занятость не равна активации или бодрости. Индивидуальная кинетика, метаболиты и региональные плотности не заданы.",
    );
}
pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "published" => "Опубликованный механизм",
        "published_fit" => "Опубликованная PET-модель",
        "hypothesis" => "Гипотеза / допущение",
        _ => "Математическая модель",
    }
}
pub fn kind_color(kind: &str) -> Color32 {
    match kind {
        "published" | "published_fit" => TEAL,
        "hypothesis" => PURPLE,
        _ => Color32::from_rgb(125, 177, 224),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regional_layers_use_local_values_and_early_clock_invalidates_neural_results() {
        let a = BrainAtlas::bundled().unwrap();
        let mut c = CaffeineLab::new(&a).unwrap();
        c.mode = true;
        c.set_time(0.15);
        assert!((c.sample().unwrap().time_min - 0.15).abs() < 1e-12);
        c.speed_min_s = 0.5;
        c.playing = true;
        c.tick(0.1);
        assert!((c.time_min - 0.2).abs() < 1e-12);
        let r = a.region("FMA72976").unwrap();
        c.layer = CaffeineLayer::Tissue;
        assert_eq!(
            c.region_value(r),
            Some(c.regional(&r.id).unwrap().1.tissue_mg_l)
        );
        c.layer = CaffeineLayer::Delivery;
        let (_, s) = c.regional(&r.id).unwrap();
        assert_eq!(c.region_value(r), Some(s.cbf_ml_100g_min * s.arterial_mg_l));
        c.scenario
            .perfusion
            .as_mut()
            .unwrap()
            .regions
            .iter_mut()
            .find(|p| p.region_id == r.id)
            .unwrap()
            .flow_fraction = 0.0;
        c.refresh(&a);
        assert!(!c.playing);
        assert_eq!(c.regional(&r.id).unwrap().1.tissue_mg, 0.0);
        c.layer = CaffeineLayer::Cbf;
        assert_eq!(c.region_value(r), Some(0.0));
        c.scenario.perfusion.as_mut().unwrap().regions[0].transit_sec = f64::NAN;
        c.refresh(&a);
        assert!(c.trace.is_none() && c.region_value(r).is_none());
    }
    #[test]
    fn caffeine_pause_invalidation_and_unknown_regions_are_explicit() {
        let a = BrainAtlas::bundled().unwrap();
        let mut c = CaffeineLab::new(&a).unwrap();
        c.mode = true;
        c.playing = true;
        c.tick(0.1);
        assert_eq!(c.time_min, 3.0);
        c.playing = false;
        c.tick(1.0);
        assert_eq!(c.time_min, 3.0);
        c.scenario.neural_hypothesis.enabled = true;
        c.refresh(&a);
        c.set_time(60.0);
        c.compute_neural(&a);
        assert!(c.comparison.is_some());
        c.set_time(61.0);
        assert!(c.comparison.is_none());
        c.layer = CaffeineLayer::A2a;
        let cortex = a
            .regions
            .iter()
            .find(|r| r.kind == neuro_domain::TissueKind::Cortex)
            .unwrap();
        assert_eq!(c.region_value(cortex), None);
        c.scenario.parameters.body_mass_kg = f64::NAN;
        c.refresh(&a);
        assert!(c.trace.is_none() && c.comparison.is_none() && c.region_value(cortex).is_none());
    }
}
