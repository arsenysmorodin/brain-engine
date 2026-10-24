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
            frame: 0,
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
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
