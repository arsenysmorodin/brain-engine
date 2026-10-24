use crate::{FileAction, Lab, Selection, View};
use bevy::{prelude::*, winit::WinitSettings};
use neuro_domain::Parameters;
use std::path::PathBuf;
#[derive(Resource)]
struct Smoke {
    frame: u32,
    directory: PathBuf,
}
pub fn configure(app: &mut App) {
    if let Some(directory) = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|p| p[0] == "--smoke-test")
        .map(|p| PathBuf::from(&p[1]))
    {
        if let Err(error) = std::fs::create_dir_all(&directory) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        app.insert_resource(Smoke {
            frame: if std::env::args().any(|a| a == "--circulation-only") {
                680
            } else {
                0
            },
            directory,
        })
        .insert_resource(WinitSettings::continuous())
        .add_systems(Update, run);
    }
}
fn run(
    mut smoke: ResMut<Smoke>,
    mut lab: ResMut<Lab>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
    mut egui_inputs: Query<&mut bevy_egui::EguiInput>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    smoke.frame += 1;
    match smoke.frame {
        25 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("brain-overview.png")));
        }
        30 => {
            lab.view = View::Map;
        }
        35 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("map.png")));
        }
        40 => {
            lab.selection = Selection::Claim("caffeine-a2a".into());
        }
        50 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("source.png")));
        }
        65 => {
            lab.view = View::Experiment;
            lab.draft_a = "3".into();
            lab.draft_b = "6".into();
            lab.apply_drafts();
            let scenario = smoke.directory.join("scenario.json");
            assert!(lab.perform(FileAction::Save, &scenario));
            lab.draft_a = "17".into();
            lab.apply_drafts();
            assert!(lab.perform(FileAction::Open, &scenario));
            assert_eq!(lab.scenario.parameters, Parameters { a: 3.0, b: 6.0 });
            assert!(lab.perform(FileAction::Csv, &smoke.directory.join("gui.csv")));
        }
        80 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("experiment.png")));
        }
        90 => {
            let corrupt = smoke.directory.join("corrupt.json");
            std::fs::write(&corrupt, "{broken").expect("write corrupt fixture");
            let before = lab.scenario.clone();
            assert!(!lab.perform(FileAction::Open, &corrupt));
            assert_eq!(lab.scenario, before);
            assert!(lab.status.contains("Повреждённый JSON"));
            lab.dialog = Some(FileAction::Open);
            lab.file_path = corrupt.display().to_string();
        }
        95 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("corrupt-file.png")));
        }
        100 => {
            lab.dialog = None;
            lab.draft_a = "NaN".into();
            lab.apply_drafts();
            assert!(lab.parameter_error.is_some());
        }
        110 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("invalid-input.png")));
        }
        130 => {
            lab.draft_a = "3".into();
            lab.apply_drafts();
            lab.view = View::Response;
            lab.response.draft_x = "1".into();
            lab.response.draft_tau = "10".into();
            lab.response.apply_drafts();
            let path = smoke.directory.join("response.json");
            assert!(lab.perform(FileAction::Save, &path));
            lab.response.draft_tau = "0.1".into();
            lab.response.apply_drafts();
            assert!(lab.perform(FileAction::Open, &path));
            assert_eq!(
                lab.response.scenario.parameters,
                neuro_domain::ResponseParameters { x: 1.0, tau: 10.0 }
            );
            assert_eq!(lab.scenario.parameters, Parameters { a: 3.0, b: 6.0 });
            assert!(lab.perform(FileAction::Csv, &smoke.directory.join("response-gui.csv")));
        }
        145 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("response.png")));
        }
        155 => {
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
        }
        165 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("response-zero.png")));
        }
        175 => {
            lab.response.draft_tau = "NaN".into();
            lab.response.apply_drafts();
            assert!(lab.response.evaluation.is_none());
            assert!(!lab.perform(
                FileAction::Save,
                &smoke.directory.join("invalid-response.json")
            ));
        }
        185 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("response-invalid.png")));
        }
        200 => {
            lab.activate(View::Brain);
            lab.brain.scene = neuro_domain::BrainScene::default();
        }
        220 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("brain.png")));
        }
        235 => {
            lab.brain.scene.selected_region = Some("FMA258716".into());
            lab.brain.scene.layers.cortex = false;
            lab.brain.scene.layers.cerebellum = false;
            lab.brain.scene.separation = 0.35;
        }
        250 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("brain-deep.png")));
        }
        265 => {
            lab.brain.scene.isolate = true;
            lab.brain.focus_selected();
            let mesh = lab
                .brain
                .meshes
                .iter()
                .find(|m| lab.brain.atlas.regions[m.region].id == "FMA258716")
                .unwrap();
            let t = &mesh.indices[..3];
            let point = t.iter().map(|&i| mesh.positions[i as usize]).sum::<Vec3>() / 3.0
                + lab
                    .brain
                    .offset(lab.brain.atlas.region("FMA258716").unwrap());
            let camera = crate::brain::camera_transform(lab.brain.scene.camera);
            let local = camera.rotation.inverse() * (point - camera.translation);
            let tan = (crate::brain::FIELD_OF_VIEW * 0.5).tan();
            let ndc = Vec2::new(local.x / (-local.z * tan * 1.2), local.y / (-local.z * tan));
            assert_eq!(lab.brain.pick(ndc, 1.2).as_deref(), Some("FMA258716"));
            let path = smoke.directory.join("brain-scene.json");
            let scene = lab.brain.scene.clone();
            assert!(lab.perform(FileAction::Save, &path));
            lab.brain.scene.camera.distance = 12.0;
            lab.brain.scene.selected_region = None;
            lab.brain.scene.isolate = false;
            assert!(lab.perform(FileAction::Open, &path));
            assert_eq!(lab.brain.scene, scene);
            assert!(lab.perform(FileAction::Csv, &smoke.directory.join("brain-regions.csv")));
        }
        280 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("brain-selected.png")));
        }
        295 => {
            let mut bad = lab.brain.scene.clone();
            bad.atlas_revision = 99;
            let path = smoke.directory.join("brain-unknown-version.json");
            std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            let scene = lab.brain.scene.clone();
            assert!(!lab.perform(FileAction::Open, &path));
            assert_eq!(lab.brain.scene, scene);
        }
        310 => {
            lab.brain.scene.selected_region = Some("FMA67944".into());
            lab.brain.scene.isolate = true;
            lab.brain.focus_selected();
        }
        330 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("brain-cerebellum.png")));
        }
        350 => {
            lab.activate(View::Brain);
            lab.brain.scene = neuro_domain::BrainScene::default();
            lab.brain.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.brain.scene.camera.pitch = 0.0;
            let mut simulation = crate::brain::SimulationLab::new(&lab.brain.atlas).unwrap();
            simulation.mode = true;
            simulation.revision = lab.brain.simulation.revision + 1;
            lab.brain.simulation = simulation;
        }
        360 => {
            lab.brain.simulation.advance(320);
        }
        370 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("simulation-80ms.png")));
        }
        380 => {
            lab.brain.simulation.advance(400);
        }
        390 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("simulation-180ms.png")));
        }
        400 => {
            lab.brain.simulation.advance(240);
            lab.brain.scene.layers.cortex = false;
        }
        410 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("simulation-deep-240ms.png"),
                ));
        }
        420 => {
            lab.brain.simulation.advance(3040);
            assert!(lab.brain.simulation.engine.as_ref().unwrap().finished());
            let scenario = lab.brain.simulation.scenario.clone();
            let csv = smoke.directory.join("network-gui.csv");
            let project = smoke.directory.join("brain-project.json");
            assert!(lab.perform(FileAction::Save, &project));
            assert!(lab.perform(FileAction::Csv, &csv));
            assert!(!lab.perform(FileAction::Csv, &project));
            lab.brain.simulation.scenario.connections[0].enabled = false;
            let atlas = lab.brain.atlas.clone();
            lab.brain.simulation.reset(&atlas);
            assert!(lab.perform(FileAction::Open, &project));
            assert_eq!(lab.brain.simulation.scenario, scenario);
            assert_eq!(lab.brain.simulation.display_time(), 0.0);
            assert!(!lab.brain.simulation.playing);
            lab.brain.simulation.advance(4000);
            let again = smoke.directory.join("network-reopened.csv");
            assert!(lab.perform(FileAction::Csv, &again));
            assert_eq!(std::fs::read(&csv).unwrap(), std::fs::read(&again).unwrap());
            lab.brain.simulation.cursor = Some(120);
            lab.brain.simulation.revision += 1;
            assert_eq!(lab.brain.simulation.display_time(), 240.0);
        }
        430 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("simulation-history.png")));
        }
        440 => {
            let before = lab.brain.simulation.scenario.clone();
            let mut bad = before.clone();
            bad.connections[0].delay_ms = 0.0;
            let path = smoke.directory.join("network-invalid.json");
            std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(!lab.perform(FileAction::Open, &path));
            assert_eq!(lab.brain.simulation.scenario, before);
            assert_eq!(lab.brain.simulation.display_time(), 240.0);
            lab.brain.simulation.scenario.populations[0].tau_e_ms = f64::NAN;
            let atlas = lab.brain.atlas.clone();
            lab.brain.simulation.reset(&atlas);
            assert!(!lab.perform(
                FileAction::Save,
                &smoke.directory.join("invalid-project.json")
            ));
            assert!(!lab.perform(
                FileAction::Csv,
                &smoke.directory.join("invalid-network.csv")
            ));
            assert!(lab.brain.simulation.state("FMA72976").is_none());
            lab.brain.simulation.scenario = before;
            lab.brain.simulation.reset(&atlas);
        }
        450 => {
            std::fs::write(smoke.directory.join("network-passed.txt"),"E/I activity at 80/180/240 ms; project roundtrip resets to paused t=0; deterministic replay CSV identical; history frame 240 ms; invalid topology leaves previous state intact; invalid parameters block output and clear activity; legacy smoke also passed\n").unwrap();
            std::fs::write(smoke.directory.join("passed.txt"), "Native brain startup; overview/deep/isolated scenes; ray picks selected thalamus; brain save/mutate/reload/CSV; unknown atlas rejected without mutation; binding and response save/mutate/reload/CSV; invalid numerical inputs rejected: passed\n").expect("smoke report");
        }
        460 => {
            lab.activate(View::Brain);
            lab.brain.simulation.mode = false;
            lab.brain.simulation.playing = false;
            lab.brain.scene = neuro_domain::BrainScene::default();
            lab.brain.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.brain.scene.camera.pitch = 0.0;
            lab.brain.caffeine =
                crate::brain::caffeine::CaffeineLab::new(&lab.brain.atlas).unwrap();
            lab.brain.caffeine.mode = true;
            lab.brain
                .caffeine
                .scenario
                .doses
                .push(neuro_domain::CaffeineDose {
                    time_min: 240.0,
                    amount_mg: 100.0,
                });
            let atlas = lab.brain.atlas.clone();
            lab.brain.caffeine.refresh(&atlas);
            lab.brain.caffeine.set_time(60.0);
            assert!(lab.brain.caffeine.sample().unwrap().plasma_mg_l > 1.0);
        }
        470 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("caffeine-brain-60min.png"),
                ));
        }
        480 => {
            lab.brain.caffeine.show_plot = false;
            lab.brain.caffeine.route = true;
            lab.brain.caffeine.selection = Some(crate::brain::caffeine::MapSelection::Link(1));
        }
        490 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("caffeine-pathway.png")));
        }
        500 => {
            lab.brain.caffeine.show_plot = true;
            lab.brain.caffeine.route = false;
            lab.brain.caffeine.layer = crate::brain::caffeine::CaffeineLayer::A2a;
            lab.brain.caffeine.plot = crate::brain::caffeine::CaffeinePlot::Occupancy;
            lab.brain.caffeine.revision += 1;
            lab.brain.caffeine.selection = None;
            lab.brain.scene.layers.cortex = false;
            lab.brain.scene.layers.cerebellum = false;
            lab.brain.scene.layers.brainstem = false;
            lab.brain.scene.selected_region = Some("FMA72829".into());
            lab.brain.scene.camera = neuro_domain::BrainCamera::default();
        }
        510 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("caffeine-a2a.png")));
        }
        520 => {
            let atlas = lab.brain.atlas.clone();
            lab.brain.caffeine.scenario.neural_hypothesis.enabled = true;
            lab.brain.caffeine.refresh(&atlas);
            lab.brain.caffeine.set_time(60.0);
            lab.brain.caffeine.compute_neural(&atlas);
            assert!(lab.brain.caffeine.comparison.is_some());
            std::fs::write(
                smoke.directory.join("caffeine-neural-gui.json"),
                serde_json::to_vec_pretty(lab.brain.caffeine.comparison.as_ref().unwrap()).unwrap(),
            )
            .unwrap();
            lab.brain.scene.layers.cortex = true;
            lab.brain.scene.selected_region = Some("FMA72976".into());
            lab.brain.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.brain.scene.camera.pitch = 0.0;
        }
        530 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("caffeine-neural-hypothesis.png"),
                ));
        }
        540 => {
            let project = smoke.directory.join("caffeine-project.json");
            let csv = smoke.directory.join("caffeine-gui.csv");
            let scenario = lab.brain.caffeine.scenario.clone();
            let trace = lab.brain.caffeine.trace.as_ref().unwrap().csv();
            assert!(lab.perform(FileAction::Save, &project));
            assert!(lab.perform(FileAction::Csv, &csv));
            assert_eq!(std::fs::read_to_string(&csv).unwrap(), trace);
            assert!(!lab.perform(FileAction::Csv, &project));
            lab.brain.caffeine.scenario.doses.clear();
            let atlas = lab.brain.atlas.clone();
            lab.brain.caffeine.refresh(&atlas);
            assert!(lab.perform(FileAction::Open, &project));
            assert_eq!(lab.brain.caffeine.scenario, scenario);
            assert_eq!(lab.brain.caffeine.time_min, 0.0);
            assert!(!lab.brain.caffeine.playing && lab.brain.caffeine.comparison.is_none());
            assert_eq!(lab.brain.caffeine.trace.as_ref().unwrap().csv(), trace);
            let before = lab.brain.caffeine.scenario.clone();
            let bad = smoke.directory.join("caffeine-invalid.json");
            let mut invalid = before.clone();
            invalid.parameters.bioavailability = 1.5;
            std::fs::write(&bad, serde_json::to_string_pretty(&invalid).unwrap()).unwrap();
            assert!(!lab.perform(FileAction::Open, &bad));
            assert_eq!(lab.brain.caffeine.scenario, before);
            lab.brain.caffeine.scenario.parameters.body_mass_kg = f64::NAN;
            lab.brain.caffeine.refresh(&atlas);
            assert!(lab.brain.caffeine.trace.is_none());
            assert!(!lab.perform(
                FileAction::Save,
                &smoke.directory.join("caffeine-invalid-project.json")
            ));
            assert!(!lab.perform(
                FileAction::Csv,
                &smoke.directory.join("caffeine-invalid.csv")
            ));
            lab.brain.caffeine.scenario = before;
            lab.brain.caffeine.refresh(&atlas);
            lab.brain.caffeine.set_time(300.0);
            lab.brain.caffeine.route = false;
            lab.brain.caffeine.show_plot = true;
            lab.brain.caffeine.plot = crate::brain::caffeine::CaffeinePlot::Mass;
        }
        550 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("caffeine-mass.png")));
        }
        560 => {
            std::fs::write(smoke.directory.join("caffeine-passed.txt"),"Caffeine two-dose PK; 60 min brain / pathway / striatal PET / optional E/I comparison; project roundtrip resets paused t=0; CSV identical after replay; invalid input preserves previous scenario; NaN clears PK and blocks save/export; input protected from CSV overwrite.\n").unwrap();
            lab.brain.caffeine.show_plot = false;
            lab.brain.caffeine.layer = crate::brain::caffeine::CaffeineLayer::A1;
            lab.brain.caffeine.revision += 1;
            lab.brain.scene = neuro_domain::BrainScene::default();
            lab.brain.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.brain.scene.camera.pitch = 0.0;
            lab.status_error = false;
            lab.status = "Кофеин: крупный 3D-мозг; график скрыт отдельным переключателем.".into();
        }
        570 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("caffeine-brain-focus.png"),
                ));
        }
        580 => {
            lab.response.dirty = false;
            let a = lab.brain.atlas.clone();
            lab.brain.caffeine = crate::brain::caffeine::CaffeineLab::new(&a).unwrap();
            lab.brain.caffeine.mode = true;
            lab.brain.caffeine.set_time(60.0);
            lab.brain.caffeine.show_plot = false;
            lab.brain.caffeine.tissue_opacity = 0.12;
            lab.brain.scene = neuro_domain::BrainScene::default();
            lab.status =
                "Артериальная сеть · цвет Ca; региональная доставка по редактируемым допущениям."
                    .into();
        }
        590 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("vascular-overview.png")));
        }
        600 => {
            lab.brain.caffeine.layer = crate::brain::caffeine::CaffeineLayer::Territories;
            lab.brain.caffeine.selected_vessel = lab
                .brain
                .vascular_atlas
                .vessels
                .iter()
                .position(|v| v.id == "FMA50542");
            lab.brain.caffeine.tissue_opacity = 0.07;
            lab.brain.caffeine.revision += 1;
            lab.brain.scene.camera.pitch = -0.7;
            lab.brain.scene.camera.yaw = std::f32::consts::PI;
        }
        610 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("vascular-circle-of-willis.png"),
                ));
        }
        620 => {
            let a = lab.brain.atlas.clone();
            let c = &mut lab.brain.caffeine;
            c.scenario
                .perfusion
                .as_mut()
                .unwrap()
                .regions
                .iter_mut()
                .find(|r| r.region_id == "FMA72976")
                .unwrap()
                .flow_fraction = 0.3;
            c.refresh(&a);
            c.set_time(1.5);
            c.selected_vessel = None;
            c.selection = None;
            c.layer = crate::brain::caffeine::CaffeineLayer::Tissue;
            c.show_plot = true;
            c.plot = crate::brain::caffeine::CaffeinePlot::RegionalConcentration;
            c.tissue_opacity = 0.25;
            lab.brain.scene.selected_region = Some("FMA72976".into());
            lab.brain.scene.camera = neuro_domain::BrainCamera::default();
            lab.brain.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.brain.scene.camera.pitch = 0.0;
            assert!(
                lab.brain
                    .caffeine
                    .regional("FMA72976")
                    .unwrap()
                    .1
                    .tissue_mg_l
                    < lab
                        .brain
                        .caffeine
                        .regional("FMA72975")
                        .unwrap()
                        .1
                        .tissue_mg_l
            );
        }
        630 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("vascular-regional-delivery.png"),
                ));
        }
        640 => {
            lab.brain.caffeine.set_time(60.0);
            lab.brain.caffeine.layer = crate::brain::caffeine::CaffeineLayer::Cbf;
            lab.brain.caffeine.plot = crate::brain::caffeine::CaffeinePlot::RegionalFlow;
            lab.brain.caffeine.tissue_opacity = 0.8;
            lab.brain.caffeine.revision += 1;
            let project = smoke.directory.join("perfusion-project.json");
            let regional = smoke.directory.join("perfusion-gui.csv");
            assert!(lab.perform(FileAction::Save, &project));
            assert!(lab.perform(FileAction::RegionalCsv, &regional));
            let expected = lab
                .brain
                .caffeine
                .trace
                .as_ref()
                .unwrap()
                .perfusion
                .as_ref()
                .unwrap()
                .regional_csv(&lab.brain.caffeine.scenario);
            assert_eq!(std::fs::read_to_string(&regional).unwrap(), expected);
            assert!(!lab.perform(FileAction::RegionalCsv, &project));
            let a = lab.brain.atlas.clone();
            lab.brain.caffeine.scenario.perfusion = None;
            lab.brain.caffeine.refresh(&a);
            assert!(!lab.perform(
                FileAction::RegionalCsv,
                &smoke.directory.join("missing-perfusion.csv")
            ));
            assert!(lab.perform(FileAction::Open, &project));
            assert_eq!(lab.brain.caffeine.time_min, 0.0);
            assert!(lab.brain.caffeine.scenario.perfusion.is_some());
            assert!(!lab.brain.caffeine.playing);
            lab.brain.caffeine.set_time(60.0);
            lab.brain.caffeine.layer = crate::brain::caffeine::CaffeineLayer::Cbf;
            lab.brain.caffeine.plot = crate::brain::caffeine::CaffeinePlot::RegionalFlow;
            lab.brain.caffeine.show_plot = true;
        }
        650 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("vascular-regional-flow.png"),
                ));
        }
        660 => {
            let a = lab.brain.atlas.clone();
            let c = &mut lab.brain.caffeine;
            c.scenario.neural_hypothesis.enabled = true;
            c.refresh(&a);
            c.set_time(2.0);
            c.compute_neural(&a);
            std::fs::write(
                smoke.directory.join("perfusion-neural-gui.json"),
                serde_json::to_vec_pretty(c.comparison.as_ref().unwrap()).unwrap(),
            )
            .unwrap();
            assert!(lab.perform(
                FileAction::Save,
                &smoke.directory.join("perfusion-neural-project.json")
            ));
            let id = lab
                .brain
                .caffeine
                .scenario
                .neural_hypothesis
                .network
                .populations[0]
                .region_id
                .clone();
            let c = &mut lab.brain.caffeine;
            let x = c
                .scenario
                .perfusion
                .as_mut()
                .unwrap()
                .regions
                .iter_mut()
                .find(|r| r.region_id == id)
                .unwrap();
            x.flow_fraction = f64::NAN;
            c.refresh(&a);
            assert!(c.trace.is_none() && c.comparison.is_none());
            assert!(!lab.perform(
                FileAction::RegionalCsv,
                &smoke.directory.join("perfusion-invalid.csv")
            ));
            assert!(lab.perform(
                FileAction::Open,
                &smoke.directory.join("perfusion-neural-project.json")
            ));
            lab.brain.caffeine.set_time(60.0);
            lab.brain.caffeine.compute_neural(&a);
            lab.brain.caffeine.neural_time_ms = 100.0;
            lab.brain.caffeine.tissue_opacity = 1.0;
            lab.brain.caffeine.revision += 1;
        }
        670 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("vascular-neural-effect.png"),
                ));
        }
        680 => {
            std::fs::write(smoke.directory.join("perfusion-passed.txt"),"127 arterial meshes / 85 named structures; regional flow reduction and local Ct; 3 s early playback; regional CSV and project replay; input overwrite blocked; disabled/NaN perfusion blocks regional export; regional E/I drivers replay.\n").unwrap();
        }

        690 => {
            let atlas = lab.brain.atlas.clone();
            let mut scenario = neuro_domain::CirculationScenario::reference(&atlas, 7);
            scenario.duration_min = 60.0;
            let mut c =
                crate::brain::circulation::CirculationLab::from_scenario(scenario, &atlas).unwrap();
            c.mode = true;
            c.set_time(1.5);
            lab.brain.circulation = c;
            lab.brain.caffeine.mode = false;
            lab.brain.simulation.mode = false;
            lab.brain.scene = Default::default();
            lab.view = View::Brain;
            lab.status = "Контур: три вещества, капиллярный обмен и венозный возврат.".into();
        }
        700 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("circulation-overview.png"),
                ));
        }
        710 => {
            lab.brain.scene.selected_region = Some("FMA72976".into());
            lab.brain.circulation.opacity = 0.04;
            lab.brain.circulation.revision += 1;
            lab.brain.focus_selected();
        }
        720 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("circulation-capillaries.png"),
                ));
        }
        730 => {
            lab.brain.scene.camera = Default::default();
            lab.brain.circulation.selected = neuro_domain::Purine::Methylliberine;
            lab.brain.circulation.early = false;
            lab.brain.circulation.set_time(60.0);
        }
        740 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("circulation-methylliberine.png"),
                ));
        }
        750 => {
            lab.brain.circulation.selected = neuro_domain::Purine::Theacrine;
            lab.brain.circulation.arteries = false;
            lab.brain.circulation.capillaries = false;
            lab.brain.circulation.selected_path = Some(lab.brain.flow_geometry.paths.len() - 1);
            lab.brain.circulation.revision += 1;
        }
        760 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(
                    smoke.directory.join("circulation-venous-return.png"),
                ));
        }
        770 => {
            let path = smoke.directory.join("circulation-project.json");
            assert!(lab.perform(FileAction::Save, &path));
            assert!(lab.perform(
                FileAction::Csv,
                &smoke.directory.join("circulation-gui.csv")
            ));
            assert!(lab.perform(
                FileAction::RegionalCsv,
                &smoke.directory.join("circulation-regions-gui.csv")
            ));
            assert!(!lab.perform(FileAction::Csv, &path));
            let before = lab.brain.circulation.scenario.clone();
            lab.brain.circulation.invalidate();
            assert!(!lab.perform(
                FileAction::Csv,
                &smoke.directory.join("circulation-stale.csv")
            ));
            assert!(lab.perform(FileAction::Open, &path));
            assert_eq!(lab.brain.circulation.scenario, before);
            assert_eq!(lab.brain.circulation.time_min, 0.0);
            assert!(!lab.brain.circulation.playing);
            assert!(lab.perform(
                FileAction::Csv,
                &smoke.directory.join("circulation-replay.csv")
            ));
            assert_eq!(
                std::fs::read(smoke.directory.join("circulation-gui.csv")).unwrap(),
                std::fs::read(smoke.directory.join("circulation-replay.csv")).unwrap()
            );
        }
        780 => {
            std::fs::write(smoke.directory.join("circulation-passed.txt"),"Closed three-substance circulation; capillary and venous views; replay from t=0; invalid/stale/input overwrite export guards; GUI and replay CSV identical.\n").unwrap();
        }

        790 => {
            lab.brain.circulation.selected = neuro_domain::Purine::Caffeine;
            lab.brain.circulation.set_time(60.0);
            lab.brain.circulation.early = false;
        }
        800 => {
            for mut input in &mut egui_inputs {
                input
                    .events
                    .push(bevy_egui::egui::Event::PointerMoved(bevy_egui::egui::pos2(
                        1080.0, 170.0,
                    )));
            }
        }
        860 => {
            assert!(
                lab.glossary.visible_tooltip.is_some(),
                "Scientific tooltip should open over Cp in native UI"
            );
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("learning-hover.png")));
        }
        870 => {
            lab.glossary.open = true;
            lab.glossary.search = "ГЭБ".into();
            lab.glossary.selected = lab.glossary.terms.iter().position(|t| t.term == "ГЭБ");
            for mut input in &mut egui_inputs {
                input
                    .events
                    .push(bevy_egui::egui::Event::PointerMoved(bevy_egui::egui::pos2(
                        1000.0, 80.0,
                    )));
            }
        }
        890 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(smoke.directory.join("learning-glossary.png")));
        }
        900 => {
            std::fs::write(
                smoke.directory.join("learning-passed.txt"),
                "Native hover tooltip over Cp; searchable learning glossary; screenshot review.\n",
            )
            .unwrap();
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
