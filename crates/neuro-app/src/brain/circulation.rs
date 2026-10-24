pub(super) mod geometry;
use super::{BrainLab, BrainViewport};
use crate::theme::*;
use bevy::prelude::UVec2;
use bevy_egui::egui::{self, Color32, RichText};
pub use geometry::{FlowGeometry, Stage};
use neuro_domain::{
    BrainAtlas, BrainRegion, CirculationKnowledge, CirculationScenario, Error, Purine, PurineDose,
};
use neuro_engine::{CirculationFrame, CirculationTrace, simulate_circulation};
use std::path::PathBuf;
pub const DRUG_COLORS: [Color32; 3] = [Color32::from_rgb(255, 178, 82), TEAL, PURPLE];
#[derive(Clone, Copy, PartialEq)]
pub enum TissueLayer {
    Concentration,
    Flow,
}
pub struct CirculationLab {
    pub mode: bool,
    pub scenario: CirculationScenario,
    pub trace: Option<CirculationTrace>,
    pub knowledge: CirculationKnowledge,
    pub time_min: f64,
    clock_min: f64,
    pub playing: bool,
    pub speed: f64,
    pub selected: Purine,
    pub layer: TissueLayer,
    pub arteries: bool,
    pub capillaries: bool,
    pub veins: bool,
    pub particles: bool,
    pub opacity: f32,
    pub show_regions: bool,
    pub early: bool,
    pub selected_path: Option<usize>,
    pub revision: u64,
    pub dirty: bool,
    pub needs_run: bool,
    pub error: Option<String>,
    pub last_path: Option<PathBuf>,
    pub source_path: Option<PathBuf>,
}
impl CirculationLab {
    pub fn new(atlas: &BrainAtlas) -> Result<Self, Error> {
        Ok(Self {
            mode: false,
            scenario: CirculationScenario::reference(atlas, 7),
            trace: None,
            knowledge: CirculationKnowledge::bundled()?,
            time_min: 0.0,
            clock_min: 0.0,
            playing: false,
            speed: 15.0,
            selected: Purine::Caffeine,
            layer: TissueLayer::Concentration,
            arteries: true,
            capillaries: true,
            veins: true,
            particles: true,
            opacity: 0.13,
            show_regions: false,
            early: true,
            selected_path: None,
            revision: 0,
            dirty: false,
            needs_run: false,
            error: None,
            last_path: None,
            source_path: None,
        })
    }
    pub fn from_scenario(s: CirculationScenario, atlas: &BrainAtlas) -> Result<Self, Error> {
        let tr = simulate_circulation(s.clone(), atlas)?;
        let mut c = Self::new(atlas)?;
        c.scenario = s;
        c.trace = Some(tr);
        Ok(c)
    }
    pub fn refresh(&mut self, atlas: &BrainAtlas) {
        self.playing = false;
        self.time_min = 0.0;
        self.clock_min = 0.0;
        self.revision = self.revision.wrapping_add(1);
        match simulate_circulation(self.scenario.clone(), atlas) {
            Ok(t) => {
                self.trace = Some(t);
                self.error = None;
                self.needs_run = false;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.trace = None;
            }
        }
    }
    pub fn invalidate(&mut self) {
        self.playing = false;
        self.trace = None;
        self.needs_run = true;
        self.error = None;
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn set_time(&mut self, t: f64) {
        self.clock_min = if t.is_finite() {
            t.clamp(0.0, self.scenario.duration_min)
        } else {
            0.0
        };
        if let Some(tr) = &self.trace {
            self.time_min = tr.at(t).time_min;
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn tick(&mut self, dt: f64, focused: bool) {
        if !self.playing || !focused || !dt.is_finite() || dt <= 0.0 || !self.speed.is_finite() {
            return;
        }
        self.clock_min =
            (self.clock_min + dt.min(0.1) * self.speed).min(self.scenario.duration_min);
        if let Some(t) = &self.trace {
            self.time_min = t.at(self.clock_min).time_min;
            self.revision = self.revision.wrapping_add(1);
        }
        if self.clock_min >= self.scenario.duration_min {
            self.playing = false;
        }
    }
    pub fn frame(&self) -> Option<&CirculationFrame> {
        self.trace.as_ref().map(|t| t.at(self.time_min))
    }
    pub fn bed_index(&self, id: &str) -> Option<usize> {
        self.scenario
            .beds
            .regions
            .iter()
            .position(|r| r.region_id == id)
    }
    pub fn region_rgb(&self, r: &BrainRegion) -> [u8; 3] {
        let value = self.frame().and_then(|f| {
            self.bed_index(&r.id).map(|i| {
                if self.layer == TissueLayer::Flow {
                    f.cbf_ml_100g_min[i] / 100.0
                } else {
                    f.regions[i][self.selected.index()].tissue_mg_l / 5.0
                }
            })
        });
        value.map_or([65, 69, 77], |v| self.heat(v))
    }
    fn heat(&self, f: f64) -> [u8; 3] {
        let c = DRUG_COLORS[self.selected.index()];
        let t = f.clamp(0.0, 1.0).sqrt() as f32;
        std::array::from_fn(|i| (32.0 + (c.to_array()[i] as f32 - 32.0) * t) as u8)
    }
    pub fn artery_rgb(&self) -> [u8; 3] {
        self.heat(
            self.frame()
                .map_or(0.0, |f| f.systemic[self.selected.index()].central_mg_l)
                / 5.0,
        )
    }
    pub fn path_visible(&self, lab: &BrainLab, i: usize) -> bool {
        let p = &lab.flow_geometry.paths[i];
        self.mode
            && match p.stage {
                Stage::Feed => self.arteries,
                Stage::Capillary => self.capillaries,
                Stage::Vein | Stage::Sinus => self.veins,
            }
            && p.region
                .is_none_or(|i| lab.scene.visible(&lab.atlas.regions[i]))
    }
    pub fn path_concentration(&self, lab: &BrainLab, i: usize) -> f64 {
        let Some(f) = self.frame() else {
            return 0.0;
        };
        let p = &lab.flow_geometry.paths[i];
        let d = self.selected.index();
        let Some(n) = p
            .region
            .and_then(|i| self.bed_index(&lab.atlas.regions[i].id))
        else {
            let q = f
                .cbf_ml_100g_min
                .iter()
                .zip(&self.scenario.beds.regions)
                .map(|(cbf, r)| cbf * r.tissue_mass_g / 100_000.0)
                .sum::<f64>();
            return if q > 0.0 {
                f.systemic[d].return_mg_min / q
            } else {
                0.0
            };
        };
        let r = f.regions[n][d];
        match p.stage {
            Stage::Feed => f.systemic[d].central_mg_l,
            Stage::Capillary => r.capillary_mg_l,
            Stage::Vein | Stage::Sinus => r.venous_mg_l,
        }
    }
    pub fn path_rgb(&self, lab: &BrainLab, i: usize) -> [u8; 3] {
        if self.selected_path == Some(i) {
            return [240, 245, 255];
        }
        let c = self.path_concentration(lab, i);
        let base = match lab.flow_geometry.paths[i].stage {
            Stage::Feed => [130, 60, 58],
            Stage::Capillary => [98, 78, 91],
            Stage::Vein | Stage::Sinus => [65, 101, 180],
        };
        let color = DRUG_COLORS[self.selected.index()].to_array();
        let t = (c / 5.0).clamp(0.0, 1.0).sqrt();
        std::array::from_fn(|j| (base[j] as f64 * (1.0 - t) + color[j] as f64 * t) as u8)
    }
}
pub fn sidebar(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.circulation;
    egui::ScrollArea::vertical().id_salt("circulation-settings").show(ui,|ui|{
        ui.heading("Три вещества");muted(ui,"Протоколы перорального приёма");
        let mut mask=0;for d in &c.scenario.doses{if d.amount_mg>0.0{mask|=1<<d.substance.index();}}
        let before=mask;egui::ComboBox::from_id_salt("purine-presets").selected_text(preset_label(mask)).width(ui.available_width()).show_ui(ui,|ui|{for m in 1..8 {ui.selectable_value(&mut mask,m,preset_label(m));}});
        if mask!=before {let duration=c.scenario.duration_min;c.scenario=CirculationScenario::reference(&lab.atlas,mask);c.scenario.duration_min=duration;c.invalidate();}
        ui.add_space(7.0);let mut changed=false;let mut remove=None;
        for (i,d) in c.scenario.doses.iter_mut().enumerate(){ui.colored_label(DRUG_COLORS[d.substance.index()],d.substance.label());ui.horizontal(|ui|{changed|=ui.add(egui::DragValue::new(&mut d.amount_mg).range(0.0..=500.0).speed(1.0).suffix(" мг")).changed();changed|=ui.add(egui::DragValue::new(&mut d.time_min).range(0.0..=c.scenario.duration_min).speed(0.5).suffix(" мин")).changed();if ui.small_button("×").clicked(){remove=Some(i);}});}
        if let Some(i)=remove{c.scenario.doses.remove(i);changed=true;}
        ui.menu_button("Добавить приём",|ui|{for p in Purine::ALL{if ui.add_enabled(c.scenario.doses.len()<32,egui::Button::new(p.label())).clicked(){c.scenario.doses.push(PurineDose{substance:p,time_min:0.0,amount_mg:50.0});changed=true;ui.close();}}});
        ui.add_space(6.0);changed|=ui.checkbox(&mut c.scenario.interactions,"Учитывать взаимодействия").changed();muted(ui,"Парные PK-отношения из исследований. Их сочетание в тройной смеси — гипотеза.");
        if c.scenario.interactions&&!c.scenario.interaction_profile_active(){ui.colored_label(PURPLE,"Разнесённые/повторные дозы: профиль взаимодействий не применяется.");}
        changed|=ui.add(egui::DragValue::new(&mut c.scenario.duration_min).range(5.0..=2880.0).speed(30.0).suffix(" мин длительность")).changed();
        ui.collapsing("PK и обмен в мозге",|ui|{let p=&mut c.scenario.parameters[c.selected.index()];muted(ui,"Редактируется выбранное над мозгом вещество.");for (label,value,min,max) in [("V/F, л",&mut p.apparent_volume_l,10.0,1000.0),("CL/F, л/ч",&mut p.apparent_clearance_l_h,0.1,500.0),("t½ всасывания, мин",&mut p.absorption_half_min,0.5,60.0),("Kp ткани/крови",&mut p.tissue_blood_partition,0.1,3.0),("Обмен, мин⁻¹",&mut p.exchange_min_inv,0.0,5.0)]{ui.label(label);changed|=ui.add(egui::DragValue::new(value).speed(0.05).range(min..=max)).changed();}muted(ui,"Kp и обмен — допущения. V/F и CL/F — кажущиеся параметры; F неизвестна.");});
        ui.collapsing("Кровоток и сосудистые объёмы",|ui|{changed|=ui.checkbox(&mut c.scenario.beds.vasoconstriction_enabled,"Сосудистая гипотеза кофеина").changed();muted(ui,"Максимальное снижение CBF и C50 не калиброваны.");changed|=ui.add(egui::Slider::new(&mut c.scenario.beds.max_flow_reduction,0.0..=0.8).text("Снижение")).changed();changed|=ui.add(egui::DragValue::new(&mut c.scenario.beds.flow_half_effect_mg_l).range(0.1..=20.0).suffix(" мг/л C50")).changed();changed|=ui.add(egui::Slider::new(&mut c.scenario.capillary_volume_fraction,0.01..=0.1).text("Vc/Vt")).changed();changed|=ui.add(egui::Slider::new(&mut c.scenario.venous_volume_fraction,0.01..=0.1).text("Vv/Vt")).changed();});
        if changed{c.invalidate();}
        ui.add_space(7.0);if ui.add_sized([ui.available_width(),32.0],egui::Button::new(if c.needs_run{"Пересчитать протокол"}else{"Рассчитать с начала"})).clicked(){c.refresh(&lab.atlas);}
        if c.needs_run{ui.colored_label(PURPLE,"Параметры изменены. Пересчитайте для просмотра и экспорта.");}
        if let Some(e)=&c.error{ui.colored_label(ERROR,e);}
        ui.separator();muted(ui,"Капилляры и вены — учебная схема. Радиусы увеличены; частицы показывают направление, не скорость крови.");
        ui.collapsing("Исследования и ограничения",|ui|{ui.label(&c.knowledge.limitations);for s in &c.knowledge.sources{ui.add_space(8.0);ui.hyperlink_to(&s.title,&s.url);ui.small(&s.context);muted(ui,&s.verification);muted(ui,&s.rights);}ui.hyperlink_to("Источники кофеина","https://doi.org/10.1124/pr.51.1.11");});
    });
}
fn preset_label(mask: u8) -> &'static str {
    match mask {
        1 => "Только кофеин",
        2 => "Только метиллиберин",
        3 => "Кофеин + метиллиберин",
        4 => "Только теакрин",
        5 => "Кофеин + теакрин",
        6 => "Метиллиберин + теакрин",
        7 => "Все три вещества",
        _ => "Пустой протокол",
    }
}
pub fn viewport(ui: &mut egui::Ui, lab: &mut BrainLab, viewport: &BrainViewport) {
    if lab.circulation.trace.is_none()
        && !lab.circulation.needs_run
        && lab.circulation.error.is_none()
    {
        lab.circulation.refresh(&lab.atlas);
    }
    lab.scene.separation = 0.0;
    let before = (
        lab.circulation.selected,
        lab.circulation.arteries,
        lab.circulation.capillaries,
        lab.circulation.veins,
        lab.circulation.opacity,
        lab.circulation.layer,
        lab.circulation.particles,
    );
    ui.horizontal_wrapped(|ui| {
        for p in Purine::ALL {
            ui.selectable_value(
                &mut lab.circulation.selected,
                p,
                RichText::new(p.label()).color(DRUG_COLORS[p.index()]),
            );
        }
        ui.selectable_value(
            &mut lab.circulation.layer,
            TissueLayer::Concentration,
            "Ct ткани",
        );
        ui.selectable_value(&mut lab.circulation.layer, TissueLayer::Flow, "CBF");
    });
    ui.horizontal_wrapped(|ui| {
        let c = &mut lab.circulation;
        ui.checkbox(&mut c.arteries, "Артерии");
        ui.checkbox(&mut c.capillaries, "Капилляры");
        ui.checkbox(&mut c.veins, "Вены");
        ui.checkbox(&mut c.particles, "Направление");
        ui.add(
            egui::Slider::new(&mut c.opacity, 0.0..=1.0)
                .text("Ткань")
                .show_value(false),
        );
    });
    ui.horizontal_wrapped(|ui| {
        let c = &mut lab.circulation;
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
            c.playing = !c.playing;
        }
        if ui.button("В начало").clicked() {
            c.playing = false;
            c.set_time(0.0);
        }
        let mut t = c.time_min;
        if ui
            .add(
                egui::DragValue::new(&mut t)
                    .speed(0.05)
                    .range(0.0..=c.scenario.duration_min)
                    .suffix(" мин"),
            )
            .changed()
        {
            c.playing = false;
            c.set_time(t);
        }
        egui::ComboBox::from_id_salt("loop-speed")
            .selected_text(format!("×{} мин/с", c.speed))
            .show_ui(ui, |ui| {
                for speed in [0.05, 0.5, 5.0, 15.0, 60.0] {
                    ui.selectable_value(&mut c.speed, speed, format!("{speed} мин/с"));
                }
            });
        if ui.small_button("Слева").clicked() {
            lab.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.scene.camera.pitch = 0.0;
        }
        if ui.small_button("Сверху").clicked() {
            lab.scene.camera.pitch = 1.4;
        }
        if ui.small_button("По центру").clicked() {
            lab.scene.camera = Default::default();
        }
    });
    let c = &mut lab.circulation;
    c.tick(ui.input(|i| i.stable_dt) as f64, ui.input(|i| i.focused));
    if c.playing {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
    if before
        != (
            c.selected,
            c.arteries,
            c.capillaries,
            c.veins,
            c.opacity,
            c.layer,
            c.particles,
        )
    {
        c.revision = c.revision.wrapping_add(1);
    }
    let size = egui::vec2(
        ui.available_width(),
        (ui.available_height() - 305.0).max(100.0),
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
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(Color32::from_rgb(180, 90, 80), "Артериальный вход");
        ui.label("→ капилляры ⇄ ткань →");
        ui.colored_label(Color32::from_rgb(120, 160, 240), "Венозный возврат");
    });
    muted(
        ui,
        if lab.circulation.layer == TissueLayer::Flow {
            "Ткань: 0…100+ мл/(100 г·мин). Сосуды: 0…5+ мг/л выбранного вещества."
        } else {
            "Ткань и сосуды: 0…5+ мг/л выбранного вещества. Серый: нет данных. Цвет не обозначает активность нейронов."
        },
    );
    plot(ui, lab);
    ui.label(RichText::new("BodyParts3D © DBCLS · CC BY 4.0. Капилляры и вены: схематическая геометрия, увеличенный калибр.").size(10.0).color(MUTED));
}
fn plot(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.circulation;
    ui.horizontal_wrapped(|ui| {
        ui.label("Cp плазмы, мг/л");
        ui.selectable_value(&mut c.early, true, "Первые 5 мин");
        ui.selectable_value(&mut c.early, false, "Весь протокол");
    });
    let Some(trace) = &c.trace else {
        muted(ui, "Нет вычисленной трассы. Пересчитайте протокол.");
        return;
    };
    let xmax = if c.early {
        5.0
    } else {
        c.scenario.duration_min
    };
    let lines: Vec<_> = Purine::ALL
        .iter()
        .map(|p| {
            (
                DRUG_COLORS[p.index()],
                trace
                    .frames
                    .iter()
                    .filter(|f| f.time_min <= xmax)
                    .map(|f| (f.time_min, f.systemic[p.index()].central_mg_l))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let ymax = lines
        .iter()
        .flat_map(|(_, p)| p.iter().map(|(_, y)| *y))
        .fold(0.1_f64, f64::max)
        * 1.1;
    if let Some(t) = super::caffeine::chart(
        ui,
        "circulation-plot",
        xmax,
        ymax,
        c.time_min.min(xmax),
        &lines,
        if c.early || xmax <= 120.0 { "min" } else { "h" },
    ) {
        c.playing = false;
        c.set_time(t);
    }
    let mut t = c.time_min;
    if ui
        .add(
            egui::Slider::new(&mut t, 0.0..=c.scenario.duration_min)
                .show_value(false)
                .text("Время протокола"),
        )
        .changed()
    {
        c.playing = false;
        c.set_time(t);
    }
    if let Some(f) = c.frame() {
        let s = f.systemic[c.selected.index()];
        muted(
            ui,
            format!(
                "Баланс: ЖКТ {:.2} + организм {:.2} + мозг {:.3} + устранено {:.2} = {:.2} мг; ошибка {:.1e} мг",
                s.gut_mg,
                s.rest_mg,
                s.brain_mg,
                s.eliminated_mg,
                s.administered_mg,
                s.balance_error_mg
            ),
        );
    }
}
pub fn inspector(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.circulation;
    ui.heading(c.selected.label());
    muted(ui, format!("Время PK: {:.2} мин", c.time_min));
    if let Some(i) = c.selected_path {
        ui.label(&lab.flow_geometry.paths[i].name);
        muted(ui, lab.flow_geometry.paths[i].stage.label());
        ui.colored_label(PURPLE, "Учебная геометрия, не ангиограмма");
    }
    if let Some(f) = c.frame() {
        let d = c.selected.index();
        let sys = f.systemic[d];
        ui.label(format!("Cp = {:.4} мг/л", sys.central_mg_l));
        ui.label(format!(
            "Возврат из мозга = {:.4} мг/мин",
            sys.return_mg_min
        ));
        ui.label(format!("AUC = {:.3} мг·мин/л", sys.auc_mg_min_l));
        if let Some(id) = &lab.scene.selected_region
            && let Some(i) = c.bed_index(id)
        {
            let r = &c.scenario.beds.regions[i];
            let x = f.regions[i][d];
            ui.separator();
            ui.label(RichText::new(&lab.atlas.region(id).unwrap().name).strong());
            for line in [
                format!("Ccap = {:.4} мг/л", x.capillary_mg_l),
                format!("Ct = {:.4} мг/л", x.tissue_mg_l),
                format!("Cv = {:.4} мг/л", x.venous_mg_l),
                format!("CBF = {:.2} мл/(100 г·мин)", f.cbf_ml_100g_min[i]),
                format!("Вход = {:.5} мг/мин", x.inflow_mg_min),
                format!("Возврат = {:.5} мг/мин", x.return_mg_min),
                format!(
                    "Масса капилляров/ткани/вен: {:.5} / {:.5} / {:.5} мг",
                    x.capillary_mg, x.tissue_mg, x.venous_mg
                ),
            ] {
                ui.label(line);
            }
            muted(
                ui,
                format!(
                    "Учёт: вход {:.4} − возврат {:.4} = {:.5} мг в области",
                    x.cumulative_in_mg,
                    x.cumulative_return_mg,
                    x.capillary_mg + x.tissue_mg + x.venous_mg
                ),
            );
            ui.label(format!(
                "Ткань {:.1} г; бассейн {}",
                r.tissue_mass_g,
                r.territory.label()
            ));
        }
    }
    let mut changed = false;
    if let Some(id) = &lab.scene.selected_region
        && let Some(i) = c.bed_index(id)
    {
        ui.add_space(7.0);
        ui.label("Региональный множитель Q");
        changed |= ui
            .add(egui::Slider::new(
                &mut c.scenario.beds.regions[i].flow_fraction,
                0.0..=2.0,
            ))
            .changed();
    }
    if changed {
        c.invalidate();
    }
    ui.separator();
    let p = c.scenario.effective_parameters()[c.selected.index()].clone();
    ui.label(format!(
        "V/F = {:.2} л; CL/F = {:.3} л/ч",
        p.apparent_volume_l, p.apparent_clearance_l_h
    ));
    ui.label(format!(
        "Производное t½ = {:.2} ч",
        std::f64::consts::LN_2 * p.apparent_volume_l / p.apparent_clearance_l_h
    ));
    muted(
        ui,
        "Полувремя CL/V — ориентир общего распределения; хвост многокомпартментной трассы может отличаться.",
    );
    if c.scenario.interaction_profile_active() && c.scenario.interactions {
        ui.colored_label(
            PURPLE,
            "Парные отношения применены; иные дозы и тройное сочетание — экстраполяция.",
        );
    }
    ui.separator();
    ui.label(RichText::new("Нейронное действие").strong());
    muted(
        ui,
        "Для M/T нет проверенной количественной модели действия в мозге человека. Совместную занятость рецепторов и сумму активности по этим концентрациям определить нельзя. Это неизвестность, а не отсутствие эффекта.",
    );
    muted(
        ui,
        "Кофеин: прежние PET и гипотеза E/I доступны в разделе «Кофеин». Они не переносятся на M/T.",
    );
    ui.add_space(7.0);
    muted(
        ui,
        "Наведите курсор на научное обозначение для объяснения. Словарь доступен в верхней панели.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_playback_invalidates_and_replay_starts_paused() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 7);
        s.duration_min = 5.0;
        let mut c = CirculationLab::from_scenario(s, &a).unwrap();
        assert!(!c.playing);
        c.speed = 0.05;
        c.playing = true;
        for _ in 0..120 {
            c.tick(1.0 / 60.0, true);
        }
        assert_eq!(c.time_min, 0.1);
        let before = c.time_min;
        c.tick(1.0, false);
        c.tick(f64::NAN, true);
        assert_eq!(c.time_min, before);
        c.set_time(2.0);
        c.tick(0.1, true);
        assert_eq!(c.time_min, 2.0);
        c.invalidate();
        assert!(c.trace.is_none() && !c.playing && c.needs_run);
    }
    #[test]
    fn schematic_paths_are_finite_and_can_be_picked_on_actual_triangles() {
        let mut lab = BrainLab::new().unwrap();
        lab.circulation.mode = true;
        let mesh = lab.flow_geometry.meshes.last().unwrap();
        let tri = &mesh.indices[..3];
        let point = (mesh.positions[tri[0] as usize]
            + mesh.positions[tri[1] as usize]
            + mesh.positions[tri[2] as usize])
            / 3.0;
        let cam = super::super::camera_transform(lab.scene.camera);
        let local = cam.rotation.inverse() * (point - cam.translation);
        let tan = (super::super::FIELD_OF_VIEW * 0.5).tan();
        let ndc = bevy::prelude::Vec2::new(local.x / (-local.z * tan), local.y / (-local.z * tan));
        assert!(lab.flow_geometry.pick(&lab, ndc, 1.0).is_some());
        lab.circulation.arteries = false;
        lab.circulation.capillaries = false;
        lab.circulation.veins = false;
        assert!(lab.flow_geometry.pick(&lab, ndc, 1.0).is_none());
        assert_eq!(
            lab.flow_geometry
                .paths
                .iter()
                .filter(|p| p.stage == Stage::Capillary)
                .count(),
            162
        );
        assert_eq!(
            lab.flow_geometry
                .paths
                .iter()
                .filter(|p| p.stage == Stage::Sinus)
                .count(),
            4
        );
        assert!(
            lab.flow_geometry
                .meshes
                .iter()
                .all(|m| m.positions.iter().all(|p| p.is_finite())
                    && m.indices.iter().all(|i| (*i as usize) < m.positions.len()))
        );
        let p = &lab.flow_geometry.paths[0].points;
        assert_eq!(geometry::along(p, 0.0), p[0]);
        assert!(geometry::along(p, 0.5).is_finite());
    }
}
