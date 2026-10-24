mod experiment;
mod files;
mod inspector;
mod map;
mod smoke;
mod theme;
use bevy::{
    prelude::*,
    winit::{UpdateMode, WinitSettings},
};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use neuro_domain::{Catalog, Parameters, Scenario};
use neuro_engine::{Evaluation, evaluate};
use std::path::{Path, PathBuf};
use theme::*;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Map,
    Experiment,
}
#[derive(Clone)]
enum Selection {
    Entity(String),
    Claim(String),
}
#[derive(Clone, Copy)]
enum FileAction {
    Save,
    Open,
    Csv,
}
impl FileAction {
    fn title(self) -> &'static str {
        match self {
            Self::Save => "Сохранить сценарий",
            Self::Open => "Открыть сценарий",
            Self::Csv => "Экспорт численных результатов",
        }
    }
}
#[derive(Resource)]
struct Lab {
    catalog: Catalog,
    scenario: Scenario,
    evaluation: Option<Evaluation>,
    view: View,
    selection: Selection,
    map: map::MapState,
    draft_a: String,
    draft_b: String,
    parameter_error: Option<String>,
    status: String,
    status_error: bool,
    dirty: bool,
    dialog: Option<FileAction>,
    file_path: String,
    last_scenario_path: Option<PathBuf>,
    initialized: bool,
}
impl Lab {
    fn new() -> Result<Self, neuro_domain::Error> {
        let scenario = Scenario::default();
        Ok(Self {
            catalog: Catalog::bundled()?,
            evaluation: Some(evaluate(&scenario)?),
            scenario,
            view: View::Map,
            selection: Selection::Entity("caffeine".into()),
            map: map::MapState::default(),
            draft_a: "1".into(),
            draft_b: "1".into(),
            parameter_error: None,
            status: "Готово. Данные и расчёты доступны офлайн.".into(),
            status_error: false,
            dirty: false,
            dialog: None,
            file_path: String::new(),
            last_scenario_path: None,
            initialized: false,
        })
    }
    fn refresh(&mut self) {
        match evaluate(&self.scenario) {
            Ok(result) => {
                self.evaluation = Some(result);
                self.parameter_error = None;
            }
            Err(error) => {
                self.evaluation = None;
                self.parameter_error = Some(error.to_string());
            }
        }
    }
    fn apply_drafts(&mut self) {
        let parse = |name: &str, text: &str| {
            text.trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| format!("{name}: введите число; например, 1 или 1e6"))
        };
        let parsed = (|| -> Result<Parameters, String> {
            let p = Parameters {
                a: parse("a", &self.draft_a)?,
                b: parse("b", &self.draft_b)?,
            };
            p.validate().map_err(|e| e.to_string())?;
            Ok(p)
        })();
        match parsed {
            Ok(p) => {
                if self.scenario.parameters != p || self.parameter_error.is_some() {
                    self.scenario.parameters = p;
                    self.refresh();
                }
            }
            Err(error) => {
                self.parameter_error = Some(error);
                self.evaluation = None;
            }
        }
    }
    fn open_dialog(&mut self, action: FileAction) {
        let mut cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if !cwd.join("scenarios/competition.json").exists()
            && let Some(home) = std::env::var_os("HOME")
        {
            let home = PathBuf::from(home);
            cwd = if home.join("Documents").is_dir() {
                home.join("Documents")
            } else {
                home
            };
        }
        self.file_path = match action {
            FileAction::Csv => cwd.join("neuro-results.csv"),
            FileAction::Open => self
                .last_scenario_path
                .clone()
                .unwrap_or_else(|| cwd.join("scenarios/competition.json")),
            FileAction::Save => self
                .last_scenario_path
                .clone()
                .unwrap_or_else(|| cwd.join("neuro-scenario.json")),
        }
        .display()
        .to_string();
        self.dialog = Some(action);
    }
    fn file_operation(
        &mut self,
        action: FileAction,
        path: &Path,
    ) -> Result<(), neuro_domain::Error> {
        if !matches!(action, FileAction::Open) {
            neuro_domain::require(
                self.parameter_error.is_none(),
                "Исправьте параметры перед сохранением или экспортом",
            )?;
        }
        match action {
            FileAction::Save => {
                self.scenario.save(path)?;
                self.last_scenario_path = Some(path.into());
                self.dirty = false;
            }
            FileAction::Open => {
                let scenario = Scenario::load(path)?;
                let result = evaluate(&scenario)?;
                self.draft_a = scenario.parameters.a.to_string();
                self.draft_b = scenario.parameters.b.to_string();
                self.scenario = scenario;
                self.evaluation = Some(result);
                self.parameter_error = None;
                self.last_scenario_path = Some(path.into());
                self.dirty = false;
                self.view = View::Experiment;
            }
            FileAction::Csv => {
                if let Some(scenario_path) = &self.last_scenario_path {
                    neuro_domain::require(
                        path.canonicalize()
                            .ok()
                            .zip(scenario_path.canonicalize().ok())
                            .is_none_or(|(a, b)| a != b),
                        "CSV не должен перезаписывать сценарий",
                    )?;
                }
                self.evaluation
                    .as_ref()
                    .ok_or_else(|| {
                        neuro_domain::Error::Validation("Нет результата для экспорта".into())
                    })?
                    .export_csv(path)?;
            }
        }
        Ok(())
    }
    fn perform(&mut self, action: FileAction, path: &Path) -> bool {
        match self.file_operation(action, path) {
            Ok(()) => {
                self.status = format!(
                    "{}: {}",
                    match action {
                        FileAction::Save => "Сценарий сохранён",
                        FileAction::Open => "Сценарий открыт и пересчитан",
                        FileAction::Csv => "Результаты экспортированы",
                    },
                    path.display()
                );
                self.status_error = false;
                true
            }
            Err(error) => {
                self.status = error.to_string();
                self.status_error = true;
                false
            }
        }
    }
}
fn main() {
    let lab = match Lab::new() {
        Ok(lab) => lab,
        Err(error) => {
            eprintln!("Не удалось открыть NeuroLab: {error}");
            std::process::exit(1);
        }
    };
    let mut app = App::new();
    app.insert_resource(lab)
        .insert_resource(ClearColor(Color::srgb_u8(17, 21, 28)))
        // A short fallback avoids stale rendered frames with Bevy's render pipeline.
        // Numerical results are cached; no continuous simulation is running.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(std::time::Duration::from_millis(50)),
            unfocused_mode: UpdateMode::reactive_low_power(std::time::Duration::from_secs(1)),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "NeuroLab — исследовательская лаборатория".into(),
                resolution: (1380, 900).into(),
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 1100.0,
                    min_height: 720.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, ui);
    smoke::configure(&mut app);
    app.run();
}
fn ui(mut contexts: EguiContexts, mut lab: ResMut<Lab>) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !lab.initialized {
        configure(ctx);
        lab.initialized = true;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
        lab.open_dialog(FileAction::Save);
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
        lab.open_dialog(FileAction::Open);
    }
    egui::TopBottomPanel::top("header")
        .frame(
            egui::Frame::new()
                .fill(BACKGROUND)
                .inner_margin(egui::Margin::symmetric(22, 16)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("NeuroLab").size(24.0).strong());
                ui.add_space(16.0);
                muted(ui, "Личная исследовательская лаборатория");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Экспорт CSV").clicked() {
                        lab.open_dialog(FileAction::Csv);
                    }
                    if ui.button("Открыть").clicked() {
                        lab.open_dialog(FileAction::Open);
                    }
                    if ui.button("Сохранить").clicked() {
                        lab.open_dialog(FileAction::Save);
                    }
                });
            });
        });
    egui::TopBottomPanel::bottom("status")
        .frame(
            egui::Frame::new()
                .fill(BACKGROUND)
                .inner_margin(egui::Margin::symmetric(18, 8)),
        )
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(&lab.status)
                        .size(12.0)
                        .color(if lab.status_error { ERROR } else { MUTED }),
                );
                if lab.dirty {
                    ui.colored_label(PURPLE, "Есть несохранённые изменения");
                }
            });
        });
    egui::SidePanel::left("catalog").exact_width(205.0).resizable(false).frame(egui::Frame::new().fill(SURFACE).inner_margin(18)).show(ctx, |ui| {
        ui.add_space(9.0); ui.label(egui::RichText::new("Каталог").size(18.0)); ui.add_space(8.0);
        for entity in lab.catalog.entities.clone() {
            let selected = matches!(&lab.selection, Selection::Entity(id) if id == &entity.id) && lab.view == View::Map;
            let title = match entity.id.as_str() { "a1" => "Рецептор A1", "a2a" => "Рецептор A2A", _ => &entity.name };
            if ui.add_sized([ui.available_width(), 39.0], egui::Button::new(title).selected(selected)).clicked() {
                lab.selection = Selection::Entity(entity.id); lab.view = View::Map;
            }
        }
        ui.add_space(24.0); ui.separator(); ui.add_space(16.0); muted(ui, "Представление");
        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new("Карта механизма").selected(lab.view == View::Map)).clicked() { lab.view = View::Map; }
        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new("Эксперимент A / B").selected(lab.view == View::Experiment)).clicked() { lab.view = View::Experiment; }
        ui.add_space(24.0); tag(ui, "Первая итерация", TEAL); ui.add_space(8.0);
        ui.label(egui::RichText::new("Карта — опубликованные сведения.\n\nЭксперимент — отдельная математическая демонстрация.").size(12.0).color(MUTED));
        ui.add_space(18.0); ui.separator(); ui.add_space(10.0); muted(ui, "Без облака и API-ключей");
    });
    egui::SidePanel::right("inspector")
        .exact_width(325.0)
        .resizable(false)
        .frame(egui::Frame::new().fill(SURFACE).inner_margin(20))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inspector-scroll")
                .show(ui, |ui| inspector::show(ui, &mut lab));
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24))
        .show(ctx, |ui| match lab.view {
            View::Map => {
                ui.heading("Кофеин и аденозин");
                muted(ui, "Молекулярные мишени и происхождение утверждений");
                ui.add_space(18.0);
                let Lab {
                    catalog,
                    map,
                    selection,
                    ..
                } = &mut *lab;
                map::show(ui, catalog, map, selection);
            }
            View::Experiment => {
                egui::ScrollArea::vertical()
                    .id_salt("experiment-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.heading("Конкурентное связывание");
                        ui.add_space(10.0);
                        experiment::show(ui, &mut lab);
                    });
            }
        });
    files::show(ctx, &mut lab);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_uses_engine_and_rejects_nonfinite_drafts() {
        let mut lab = Lab::new().unwrap();
        lab.draft_a = "3".into();
        lab.draft_b = "6".into();
        lab.apply_drafts();
        assert_eq!(
            lab.evaluation.as_ref().unwrap().current,
            evaluate(&lab.scenario).unwrap().current
        );
        lab.draft_a = "NaN".into();
        lab.apply_drafts();
        assert!(lab.evaluation.is_none() && lab.parameter_error.is_some());
        lab.draft_a = "1,5".into();
        lab.apply_drafts();
        assert_eq!(lab.scenario.parameters.a, 1.5);
    }
}
