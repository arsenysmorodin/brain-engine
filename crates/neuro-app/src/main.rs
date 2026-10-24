mod brain;
mod experiment;
mod files;
mod inspector;
mod map;
mod plot;
mod response;
mod smoke;
mod theme;
use bevy::{
    prelude::*,
    winit::{UpdateMode, WinitSettings},
};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use neuro_domain::{Catalog, Experiment, Parameters, Scenario, WorkspaceDocument};
use neuro_engine::{Evaluation, evaluate};
use std::path::{Path, PathBuf};
use theme::*;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Brain,
    Map,
    Experiment,
    Response,
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
    brain: brain::BrainLab,
    catalog: Catalog,
    scenario: Scenario,
    evaluation: Option<Evaluation>,
    response: response::ResponseLab,
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
    fn activate(&mut self, view: View) {
        if view != View::Brain {
            self.brain.simulation.playing = false;
        }
        self.view = view;
        self.status_error = false;
        self.status = match view {
            View::Brain => "Мозг: выберите область на поверхности или в списке.",
            View::Map => "Карта механизмов: выберите сущность или связь.",
            View::Experiment => "Конкурентное связывание: абстрактные лиганды A и B.",
            View::Response => "Операционный ответ: абстрактный агонист.",
        }
        .into();
    }
    fn new() -> Result<Self, neuro_domain::Error> {
        let scenario = Scenario::default();
        Ok(Self {
            brain: brain::BrainLab::new()?,
            catalog: Catalog::bundled()?,
            evaluation: Some(evaluate(&scenario)?),
            scenario,
            response: response::ResponseLab::new()?,
            view: View::Brain,
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
        let response = self.view == View::Response;
        let brain = self.view == View::Brain;
        let network = brain && self.brain.simulation.mode;
        let last_path = if network {
            &self.brain.simulation.last_path
        } else if brain {
            &self.brain.last_scene_path
        } else if response {
            &self.response.last_scenario_path
        } else {
            &self.last_scenario_path
        };
        self.file_path = match action {
            FileAction::Csv => cwd.join(if network {
                "neuro-brain-trace.csv"
            } else if brain {
                "neuro-brain-regions.csv"
            } else if response {
                "neuro-response-results.csv"
            } else {
                "neuro-results.csv"
            }),
            FileAction::Open => last_path.clone().unwrap_or_else(|| {
                cwd.join(if network {
                    "scenarios/brain-network.json"
                } else if brain {
                    "scenes/brain.json"
                } else if response {
                    "scenarios/response.json"
                } else {
                    "scenarios/competition.json"
                })
            }),
            FileAction::Save => last_path.clone().unwrap_or_else(|| {
                cwd.join(if network {
                    "neuro-brain-project.json"
                } else if brain {
                    "neuro-brain-scene.json"
                } else if response {
                    "neuro-response-scenario.json"
                } else {
                    "neuro-scenario.json"
                })
            }),
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
        if self.view != View::Brain && !matches!(action, FileAction::Open) {
            neuro_domain::require(
                if self.view == View::Response {
                    self.response.parameter_error.is_none()
                } else {
                    self.parameter_error.is_none()
                },
                "Исправьте параметры перед сохранением или экспортом",
            )?;
        }
        match action {
            FileAction::Save => {
                if self.view == View::Brain && self.brain.simulation.mode {
                    neuro_domain::require(
                        self.brain.simulation.error.is_none(),
                        "Исправьте параметры сети перед сохранением",
                    )?;
                    neuro_domain::BrainProject::new(
                        self.brain.scene.clone(),
                        self.brain.simulation.scenario.clone(),
                    )
                    .save(path, &self.brain.atlas)?;
                    self.brain.simulation.last_path = Some(path.into());
                    self.brain.simulation.dirty = false;
                    self.brain.dirty = false;
                } else if self.view == View::Brain {
                    self.brain.scene.save(path, &self.brain.atlas)?;
                    self.brain.last_scene_path = Some(path.into());
                    self.brain.dirty = false;
                } else if self.view == View::Response {
                    self.response.scenario.save(path)?;
                    self.response.last_scenario_path = Some(path.into());
                    self.response.dirty = false;
                } else {
                    self.scenario.save(path)?;
                    self.last_scenario_path = Some(path.into());
                    self.dirty = false;
                }
            }
            FileAction::Open => match WorkspaceDocument::load(path, &self.brain.atlas)? {
                WorkspaceDocument::Brain(scene) => {
                    self.brain.simulation.playing = false;
                    self.brain.simulation.mode = false;
                    self.brain.scene = scene;
                    self.brain.last_scene_path = Some(path.into());
                    self.brain.dirty = false;
                    self.view = View::Brain;
                }
                WorkspaceDocument::Project(project) => {
                    let mut simulation =
                        brain::SimulationLab::from_scenario(project.network, &self.brain.atlas)?;
                    simulation.mode = true;
                    simulation.last_path = Some(path.into());
                    simulation.revision = self.brain.simulation.revision.wrapping_add(1);
                    self.brain.scene = project.scene;
                    self.brain.simulation = simulation;
                    self.brain.dirty = false;
                    self.view = View::Brain;
                }
                WorkspaceDocument::Network(network) => {
                    let mut simulation =
                        brain::SimulationLab::from_scenario(network, &self.brain.atlas)?;
                    simulation.mode = true;
                    simulation.revision = self.brain.simulation.revision.wrapping_add(1);
                    simulation.source_path = Some(path.into());
                    self.brain.simulation = simulation;
                    self.view = View::Brain;
                }
                WorkspaceDocument::Experiment(experiment) => match experiment {
                    Experiment::Binding(scenario) => {
                        let result = evaluate(&scenario)?;
                        self.brain.simulation.playing = false;
                        self.draft_a = scenario.parameters.a.to_string();
                        self.draft_b = scenario.parameters.b.to_string();
                        self.scenario = scenario;
                        self.evaluation = Some(result);
                        self.parameter_error = None;
                        self.last_scenario_path = Some(path.into());
                        self.dirty = false;
                        self.view = View::Experiment;
                    }
                    Experiment::Response(scenario) => {
                        let result = neuro_engine::evaluate_response(&scenario)?;
                        self.brain.simulation.playing = false;
                        self.response.draft_x = scenario.parameters.x.to_string();
                        self.response.draft_tau = scenario.parameters.tau.to_string();
                        self.response.scenario = scenario;
                        self.response.evaluation = Some(result);
                        self.response.parameter_error = None;
                        self.response.last_scenario_path = Some(path.into());
                        self.response.dirty = false;
                        self.view = View::Response;
                    }
                },
            },
            FileAction::Csv => {
                for scenario_path in [
                    &self.last_scenario_path,
                    &self.response.last_scenario_path,
                    &self.brain.last_scene_path,
                    &self.brain.simulation.last_path,
                    &self.brain.simulation.source_path,
                ]
                .into_iter()
                .flatten()
                {
                    neuro_domain::require(
                        path.canonicalize()
                            .ok()
                            .zip(scenario_path.canonicalize().ok())
                            .is_none_or(|(a, b)| a != b),
                        "CSV не должен перезаписывать сценарий",
                    )?;
                }
                if self.view == View::Brain && self.brain.simulation.mode {
                    neuro_domain::require(
                        self.brain.simulation.error.is_none(),
                        "Нет корректной трассы сети для экспорта",
                    )?;
                    self.brain
                        .simulation
                        .engine
                        .as_ref()
                        .ok_or_else(|| {
                            neuro_domain::Error::Validation("Расчёт сети отсутствует".into())
                        })?
                        .trace()
                        .export_csv(path)?;
                } else if self.view == View::Brain {
                    neuro_domain::atomic_write(path, self.brain.atlas.csv().as_bytes())?;
                } else if self.view == View::Response {
                    self.response
                        .evaluation
                        .as_ref()
                        .ok_or_else(|| {
                            neuro_domain::Error::Validation("Нет результата для экспорта".into())
                        })?
                        .export_csv(path)?;
                } else {
                    self.evaluation
                        .as_ref()
                        .ok_or_else(|| {
                            neuro_domain::Error::Validation("Нет результата для экспорта".into())
                        })?
                        .export_csv(path)?;
                }
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
                        FileAction::Save
                            if self.view == View::Brain && self.brain.simulation.mode =>
                            "Проект мозга сохранён (при открытии время = 0)",
                        FileAction::Open
                            if self.view == View::Brain && self.brain.simulation.mode =>
                            "Сеть открыта на паузе, время = 0",
                        FileAction::Csv
                            if self.view == View::Brain && self.brain.simulation.mode =>
                            "Вычисленная трасса E/I экспортирована",
                        FileAction::Save if self.view == View::Brain => "Сцена мозга сохранена",
                        FileAction::Open if self.view == View::Brain => "Сцена мозга открыта",
                        FileAction::Csv if self.view == View::Brain =>
                            "Каталог областей экспортирован",
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
    let plugins = DefaultPlugins.set(WindowPlugin {
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
    });
    // On macOS the render thread can await a main-thread task during shutdown,
    // while RenderAppChannels::drop blocks that thread waiting for the render app.
    // Keep the render schedules on the main thread; GPU rendering is unchanged.
    #[cfg(target_os = "macos")]
    let plugins = plugins.disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>();
    let mut app = App::new();
    app.insert_resource(lab)
        .insert_resource(ClearColor(Color::srgb_u8(17, 21, 28)))
        // A short fallback avoids stale rendered frames with Bevy's render pipeline.
        // Network simulation advances only while explicitly playing in the brain view.
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(std::time::Duration::from_millis(50)),
            unfocused_mode: UpdateMode::reactive_low_power(std::time::Duration::from_secs(1)),
        })
        .add_plugins(plugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, brain::setup)
        .add_systems(Update, brain::advance_network)
        .add_systems(EguiPrimaryContextPass, (ui, brain::sync).chain());
    smoke::configure(&mut app);
    app.run();
}
fn ui(
    mut contexts: EguiContexts,
    mut lab: ResMut<Lab>,
    viewport: Res<brain::BrainViewport>,
) -> Result {
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
                muted(ui, "Среда моделирования мозга");
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
                if lab.brain.dirty {
                    ui.colored_label(PURPLE, "Сцена мозга не сохранена");
                }
                if lab.brain.simulation.dirty {
                    ui.colored_label(PURPLE, "Сеть не сохранена");
                }
                if lab.dirty || lab.response.dirty {
                    let models = match (lab.dirty, lab.response.dirty) {
                        (true, true) => "A/B и ответ",
                        (true, false) => "A/B",
                        _ => "ответ",
                    };
                    ui.colored_label(PURPLE, format!("Не сохранено: {models}"));
                }
            });
        });
    let previous_view = lab.view;
    egui::SidePanel::left("catalog").exact_width(if lab.view==View::Brain {245.0}else{205.0}).resizable(false).frame(egui::Frame::new().fill(SURFACE).inner_margin(18)).show(ctx, |ui| {
        if ui.add_sized([ui.available_width(),36.0],egui::Button::new("Мозг · 3D").selected(lab.view==View::Brain)).clicked() {lab.view=View::Brain;}
        if lab.view==View::Brain {
            ui.collapsing("Другие разделы",|ui| {
                if ui.button("Карта механизмов").clicked() {lab.view=View::Map;}
                if ui.button("Связывание A / B").clicked() {lab.view=View::Experiment;}
                if ui.button("Функциональный ответ").clicked() {lab.view=View::Response;}
            });
            ui.separator();
            brain::sidebar(ui,&mut lab.brain);
            return;
        }
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
        if ui.add_sized([ui.available_width(), 40.0], egui::Button::new("Ответ рецептора").selected(lab.view == View::Response)).clicked() { lab.view = View::Response; }
        ui.add_space(24.0); tag(ui, "Исследовательская версия", TEAL); ui.add_space(8.0);
        ui.label(egui::RichText::new("Карта — опубликованные сведения.\n\nЭксперименты — отдельные математические демонстрации.").size(12.0).color(MUTED));
        ui.add_space(18.0); ui.separator(); ui.add_space(10.0); muted(ui, "Без облака и API-ключей");
    });
    if lab.view != previous_view {
        let view = lab.view;
        lab.activate(view);
    }
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
            View::Brain => brain::viewport(ui, &mut lab.brain, &viewport),
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
            View::Response => {
                egui::ScrollArea::vertical()
                    .id_salt("response-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.heading("Функциональный ответ");
                        ui.add_space(10.0);
                        response::show(ui, &mut lab.response);
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

    #[test]
    fn two_models_roundtrip_independently_and_protect_scenario_files() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("neurolab-app-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let binding = directory.join("binding.json");
        let response = directory.join("response.json");
        let csv = directory.join("response.csv");
        let mut lab = Lab::new().unwrap();
        lab.view = View::Experiment;
        lab.scenario.parameters = Parameters { a: 3.0, b: 6.0 };
        lab.refresh();
        lab.file_operation(FileAction::Save, &binding).unwrap();
        lab.dirty = true;
        lab.view = View::Response;
        lab.response.draft_x = "1,5".into();
        lab.response.draft_tau = "10".into();
        lab.response.apply_drafts();
        let expected = lab.response.scenario.clone();
        lab.file_operation(FileAction::Save, &response).unwrap();
        assert!(lab.dirty && !lab.response.dirty);
        lab.response.draft_tau = "0".into();
        lab.response.apply_drafts();
        assert_eq!(
            lab.response
                .evaluation
                .as_ref()
                .unwrap()
                .current
                .ec50_over_ka,
            None
        );
        lab.view = View::Experiment;
        lab.file_operation(FileAction::Open, &response).unwrap();
        assert!(lab.view == View::Response);
        assert_eq!(lab.response.scenario, expected);
        assert_eq!(lab.scenario.parameters, Parameters { a: 3.0, b: 6.0 });
        lab.file_operation(FileAction::Csv, &csv).unwrap();
        assert_eq!(
            std::fs::read_to_string(&csv).unwrap(),
            neuro_engine::evaluate_response(&expected).unwrap().csv()
        );
        for path in [&binding, &response] {
            let before = std::fs::read(path).unwrap();
            assert!(lab.file_operation(FileAction::Csv, path).is_err());
            assert_eq!(std::fs::read(path).unwrap(), before);
        }
        lab.response.draft_tau = "NaN".into();
        lab.response.apply_drafts();
        assert!(lab.response.evaluation.is_none());
        assert!(lab.file_operation(FileAction::Save, &response).is_err());
        assert!(lab.file_operation(FileAction::Csv, &csv).is_err());
        lab.view = View::Experiment;
        lab.file_operation(FileAction::Csv, &csv).unwrap();
        lab.file_operation(FileAction::Open, &binding).unwrap();
        assert!(lab.view == View::Experiment);
        let bad = directory.join("bad.json");
        std::fs::write(&bad, "{bad").unwrap();
        assert!(lab.file_operation(FileAction::Open, &bad).is_err());
        assert!(lab.view == View::Experiment);
        assert_eq!(lab.response.scenario, expected);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
