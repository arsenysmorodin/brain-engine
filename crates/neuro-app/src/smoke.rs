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
        100 => {
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
            std::fs::write(smoke.directory.join("passed.txt"), "GUI startup; claim selection; a=3,b=6; save; mutate; reload; CSV; NaN rejection: passed\n").expect("smoke report");
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
