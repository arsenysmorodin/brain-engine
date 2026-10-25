//! Transactional user-facing protocol draft. No running state changes until calculation succeeds.
use crate::{Lab, View, brain, theme::*};
use bevy_egui::egui::{self, RichText};
use neuro_domain::{
    BrainAtlas, CIRCULATION_MODEL_REVISION, CirculationScenario, Error, Purine, PurineDose, require,
};
#[derive(Clone)]
pub struct DoseDraft {
    pub substance: Purine,
    pub amount: String,
    pub time: String,
}
pub struct SimulationLauncher {
    pub rows: Vec<DoseDraft>,
    pub duration: String,
    pub interactions: bool,
    pub error: Option<String>,
    base: CirculationScenario,
}
#[derive(PartialEq, Debug)]
enum Action {
    None,
    Cancel,
    Launch,
}
impl SimulationLauncher {
    pub fn new(base: CirculationScenario) -> Self {
        Self {
            rows: base
                .doses
                .iter()
                .map(|d| DoseDraft {
                    substance: d.substance,
                    amount: d.amount_mg.to_string(),
                    time: d.time_min.to_string(),
                })
                .collect(),
            duration: base.duration_min.to_string(),
            interactions: base.interactions,
            error: None,
            base,
        }
    }
    pub fn scenario(&self, atlas: &BrainAtlas) -> Result<CirculationScenario, Error> {
        fn parse(text: &str, label: &str) -> Result<f64, Error> {
            text.trim().replace(',', ".").parse::<f64>().map_err(|_| {
                Error::Validation(format!("{label}: введите число, например 75 или 0.5"))
            })
        }
        let mut scenario = self.base.clone();
        scenario.model_revision = CIRCULATION_MODEL_REVISION;
        scenario.duration_min = parse(&self.duration, "Длительность")?;
        scenario.interactions = self.interactions;
        scenario.doses.clear();
        require(!self.rows.is_empty(), "Добавьте хотя бы один приём")?;
        for (i, row) in self.rows.iter().enumerate() {
            let amount = parse(&row.amount, &format!("Приём {}: доза", i + 1))?;
            let time = parse(&row.time, &format!("Приём {}: время", i + 1))?;
            require(
                amount.is_finite() && (0.0..=500.0).contains(&amount),
                format!(
                    "Приём {}: доза должна быть от 0 до 500 мг (технический предел)",
                    i + 1
                ),
            )?;
            require(
                time.is_finite()
                    && (0.0..=scenario.duration_min).contains(&time)
                    && neuro_domain::on_time_grid(time, 0.5),
                format!(
                    "Приём {}: время от 0 до длительности, с шагом 0.5 мин",
                    i + 1
                ),
            )?;
            scenario.doses.push(PurineDose {
                substance: row.substance,
                time_min: time,
                amount_mg: amount,
            });
        }
        require(
            scenario.doses.iter().any(|d| d.amount_mg > 0.0),
            "Укажите ненулевую дозу хотя бы одного вещества",
        )?;
        scenario.name = format!(
            "Симуляция: {}",
            Purine::ALL
                .iter()
                .filter(|p| scenario
                    .doses
                    .iter()
                    .any(|d| d.substance == **p && d.amount_mg > 0.0))
                .map(|p| p.label())
                .collect::<Vec<_>>()
                .join(" + ")
        );
        scenario.validate(atlas)?;
        Ok(scenario)
    }
    fn render(&mut self, ctx: &egui::Context) -> Action {
        let mut action = Action::None;
        let response=egui::Modal::new(egui::Id::new("simulation-launcher")).show(ctx,|ui|{
            ui.set_width((ctx.content_rect().width()-70.0).clamp(280.0,660.0));
            ui.heading("Запустить симуляцию");muted(ui,"Выберите вещество или смесь. Способ: пероральный приём.");ui.add_space(12.0);
            let mut remove=None;
            egui::ScrollArea::vertical().id_salt("launch-doses").max_height((ctx.content_rect().height()*0.35).min(280.0)).show(ui,|ui|{
                egui::Grid::new("launch-grid").num_columns(4).spacing([12.0,8.0]).show(ui,|ui|{
                    ui.strong("Вещество");ui.strong("Доза, мг");ui.strong("Время, мин");ui.label("");ui.end_row();
                    for (i,row) in self.rows.iter_mut().enumerate(){
                        egui::ComboBox::from_id_salt(("launch-substance",i)).width(190.0).selected_text(RichText::new(row.substance.label()).color(brain::circulation::DRUG_COLORS[row.substance.index()])).show_ui(ui,|ui|{for p in Purine::ALL{ui.selectable_value(&mut row.substance,p,p.label());}});
                        let r=ui.add(egui::TextEdit::singleline(&mut row.amount).desired_width(100.0).id_salt(("launch-amount",i)));ctx.data_mut(|d|d.insert_temp(egui::Id::new(("launch-amount-rect",i)),r.rect));
                        ui.add(egui::TextEdit::singleline(&mut row.time).desired_width(110.0).id_salt(("launch-time",i))).on_hover_text("Минуты от начала симуляции. 0 — приём сразу; 30 — через полчаса. Шаг 0.5 мин.");
                        if ui.small_button("Удалить").clicked(){remove=Some(i);}ui.end_row();
                    }
                });
            });
            if let Some(i)=remove{self.rows.remove(i);}
            if ui.add_enabled(self.rows.len()<32,egui::Button::new("Добавить приём")).clicked(){self.rows.push(DoseDraft{substance:Purine::Caffeine,amount:"100".into(),time:"0".into()});}
            ui.add_space(8.0);muted(ui,"Время считается от начала сценария, а не по часам. Можно добавить несколько веществ и повторные приёмы.");
            ui.horizontal(|ui|{ui.label("Длительность, мин");ui.add(egui::TextEdit::singleline(&mut self.duration).desired_width(110.0));});
            if self.base.calibration_curve_ids.is_empty() { ui.checkbox(&mut self.interactions,"Учитывать исследованные PK-взаимодействия");muted(ui,"Профиль относится к одному общему моменту приёма. Другие дозы и тройное сочетание содержат экстраполяции."); }
            else { muted(ui,"Унаследованы параметры исследовательского протокола. Изменение состава и доз — экстраполяция; парные поправки выключены. Общие параметры можно восстановить в сверке PK."); }
            if let Some(e)=&self.error{ui.colored_label(ERROR,e);}
            ui.add_space(12.0);ui.horizontal(|ui|{if ui.button("Отмена").clicked(){action=Action::Cancel;}ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{let r=ui.add(egui::Button::new(RichText::new("Запустить симуляцию").color(BACKGROUND)).fill(TEAL));ctx.data_mut(|d|d.insert_temp(egui::Id::new("launch-submit-rect"),r.rect));if r.clicked(){action=Action::Launch;}});});
        });
        if action == Action::None && response.should_close() {
            Action::Cancel
        } else {
            action
        }
    }
}
impl Lab {
    pub fn open_launcher(&mut self) {
        let mut base = if self.brain.circulation.mode {
            self.brain.circulation.scenario.clone()
        } else {
            CirculationScenario::reference(&self.brain.atlas, 1)
        };
        if self.brain.caffeine.mode {
            let s = &self.brain.caffeine.scenario;
            base.doses = s
                .doses
                .iter()
                .map(|d| PurineDose {
                    substance: Purine::Caffeine,
                    time_min: d.time_min,
                    amount_mg: d.amount_mg,
                })
                .collect();
            base.duration_min = s.duration_min;
            base.parameters[0].apparent_volume_l = s.parameters.volume_l();
            base.parameters[0].apparent_clearance_l_h = s.parameters.clearance_l_h();
            base.parameters[0].absorption_half_min = s.parameters.absorption_half_min;
            if let Some(p) = &s.perfusion {
                base.beds = p.clone();
            }
        }
        self.launcher = Some(SimulationLauncher::new(base));
    }
    pub fn launch_simulation(&mut self) -> Result<(), Error> {
        let draft = self
            .launcher
            .as_ref()
            .ok_or_else(|| Error::Validation("Откройте окно запуска симуляции".into()))?;
        let scenario = draft.scenario(&self.brain.atlas)?;
        let selected = scenario
            .doses
            .iter()
            .find(|d| d.amount_mg > 0.0)
            .expect("validated positive dose")
            .substance;
        // Construct and validate first: failed calculation cannot replace a working simulation.
        let mut next =
            brain::circulation::CirculationLab::from_scenario(scenario, &self.brain.atlas)?;
        next.mode = true;
        next.selected = selected;
        next.playing = true;
        next.dirty = true;
        next.revision = self.brain.circulation.revision.wrapping_add(1);
        next.early = false;
        self.brain.circulation = next;
        self.brain.caffeine.mode = false;
        self.brain.caffeine.playing = false;
        self.brain.simulation.mode = false;
        self.brain.simulation.playing = false;
        self.view = View::Brain;
        self.dialog = None;
        self.launcher = None;
        self.status_error = false;
        self.status="Симуляция запущена: время приёма и дозы доступны слева; выберите 3D-мозг или путь вещества.".into();
        Ok(())
    }
}
pub fn show(ctx: &egui::Context, lab: &mut Lab) {
    let action = lab
        .launcher
        .as_mut()
        .map_or(Action::None, |d| d.render(ctx));
    match action {
        Action::None => {}
        Action::Cancel => lab.launcher = None,
        Action::Launch => {
            if let Err(e) = lab.launch_simulation()
                && let Some(d) = &mut lab.launcher
            {
                d.error = Some(e.to_string());
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_drafts_validate_comma_doses_future_times_repeated_and_invalid_values() {
        let a = BrainAtlas::bundled().unwrap();
        let mut d = SimulationLauncher::new(CirculationScenario::reference(&a, 2));
        d.duration = "60".into();
        d.rows[0].amount = "75,5".into();
        d.rows[0].time = "30".into();
        let s = d.scenario(&a).unwrap();
        assert_eq!(s.doses[0].amount_mg, 75.5);
        assert_eq!(s.doses[0].time_min, 30.0);
        assert_eq!(s.model_revision, CIRCULATION_MODEL_REVISION);
        for (dose, time) in [("NaN", "0"), ("100", "0.1"), ("501", "0"), ("100", "61")] {
            d.rows[0].amount = dose.into();
            d.rows[0].time = time.into();
            assert!(d.scenario(&a).is_err());
        }
    }
    #[test]
    fn invalid_launch_preserves_running_results_and_valid_launch_uses_selected_substance() {
        let mut lab = Lab::new().unwrap();
        lab.open_launcher();
        {
            let d = lab.launcher.as_mut().unwrap();
            d.duration = "10".into();
            d.rows = vec![DoseDraft {
                substance: Purine::Theacrine,
                amount: "NaN".into(),
                time: "5".into(),
            }];
        }
        let before = lab.brain.circulation.scenario.clone();
        assert!(lab.launch_simulation().is_err());
        assert_eq!(lab.brain.circulation.scenario, before);
        lab.launcher.as_mut().unwrap().rows[0].amount = "50".into();
        lab.launch_simulation().unwrap();
        assert!(lab.launcher.is_none());
        let c = &lab.brain.circulation;
        assert!(c.mode && c.playing && c.dirty);
        assert_eq!(c.selected, Purine::Theacrine);
        assert_eq!(
            c.trace.as_ref().unwrap().at(4.5).systemic[2].administered_mg,
            0.0
        );
        assert!(c.trace.as_ref().unwrap().at(6.0).systemic[2].central_mg_l > 0.0);
    }
    #[test]
    fn modal_launch_button_responds_to_pointer_and_cancel_keeps_draft_local() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let a = BrainAtlas::bundled().unwrap();
        let mut d = SimulationLauncher::new(CirculationScenario::reference(&a, 1));
        let mut action = Action::None;
        for frame in 0..4 {
            let pos = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("launch-submit-rect")))
                .map_or(egui::Pos2::ZERO, |r| r.center());
            let events = if frame >= 2 {
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            };
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| action = d.render(ctx),
            );
        }
        assert_eq!(action, Action::Launch);
    }
}
