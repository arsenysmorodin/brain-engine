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
            std::fs::write(smoke.directory.join("passed.txt"), "GUI startup; claim selection; binding save/mutate/reload/CSV; corrupt file and NaN rejection; response save/mutate/reload/CSV; independent states; tau=0 undefined EC50; invalid response blocks saving: passed\n").expect("smoke report");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
