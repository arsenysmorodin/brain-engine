use crate::{FileAction, Lab, View, theme::*};
use bevy_egui::egui;
use std::path::{Path, PathBuf};
pub fn show(ctx: &egui::Context, lab: &mut Lab) {
    let Some(action) = lab.dialog else {
        return;
    };
    let mut open = true;
    let mut submitted = false;
    let mut cancel = false;
    let title = if lab.view == View::Brain && lab.brain.circulation.mode {
        match action {
            FileAction::Save => "Сохранить проект циркуляции",
            FileAction::Open => "Открыть проект или протокол циркуляции",
            FileAction::Csv => "Экспорт PK трёх веществ",
            FileAction::RegionalCsv => "Экспорт капиллярного обмена и венозного возврата",
        }
    } else if lab.view == View::Brain && lab.brain.caffeine.mode {
        match action {
            FileAction::Save => "Сохранить проект кофеина",
            FileAction::Open => "Открыть проект или протокол кофеина",
            FileAction::Csv => "Экспорт всей фармакокинетической трассы",
            FileAction::RegionalCsv => "Экспорт регионального кровотока и доставки",
        }
    } else if lab.view == View::Brain && lab.brain.simulation.mode {
        match action {
            FileAction::Save => "Сохранить проект мозга",
            FileAction::Open => "Открыть проект, сеть или сцену",
            FileAction::Csv => "Экспорт вычисленной трассы E/I",
            FileAction::RegionalCsv => "Экспорт региональной перфузии",
        }
    } else if lab.view == View::Brain {
        match action {
            FileAction::Save => "Сохранить сцену мозга",
            FileAction::Open => "Открыть сцену или сценарий",
            FileAction::Csv => "Экспорт областей мозга",
            FileAction::RegionalCsv => "Экспорт региональной перфузии",
        }
    } else {
        action.title()
    };
    egui::Window::new(title).open(&mut open).collapsible(false).resizable(true).default_width(650.0).show(ctx, |ui| {
        ui.label("Полный путь к файлу");
        ui.add(egui::TextEdit::singleline(&mut lab.file_path).desired_width(f32::INFINITY));
        if matches!(action, FileAction::Open) {
            if lab.brain.circulation.dirty || lab.dirty || lab.response.dirty || lab.brain.dirty || lab.brain.simulation.dirty || lab.brain.caffeine.dirty { ui.colored_label(PURPLE, "Открытие заменит сцену или параметры соответствующей модели. Другие разделы сохранятся в памяти."); }
            if let Some(parent) = Path::new(&lab.file_path).parent() && let Ok(entries) = std::fs::read_dir(parent) {
                let mut files: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).take(128).collect(); files.sort();
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for file in files { if ui.selectable_label(false, file.file_name().unwrap_or_default().to_string_lossy()).clicked() { lab.file_path = file.display().to_string(); } }
                });
            }
        }
        let exists = !matches!(action, FileAction::Open) && Path::new(&lab.file_path).exists();
        if exists { ui.colored_label(PURPLE, "Файл уже существует. Подтверждение заменит его содержимое."); }
        if lab.status_error { ui.colored_label(ERROR, &lab.status); }
        ui.horizontal(|ui| {
            let label = if exists { "Заменить файл" } else { match action { FileAction::Save => "Сохранить JSON", FileAction::Open => "Открыть файл", FileAction::Csv | FileAction::RegionalCsv => "Экспортировать CSV" } };
            submitted = ui.button(label).clicked(); cancel = ui.button("Отмена").clicked();
        });
        muted(ui, if lab.view == View::Brain && lab.brain.circulation.mode { "JSON хранит сцену и полный протокол трёх веществ с параметрами взаимодействий и обмена. Открытие пересчитывает с t=0 на паузе. CSV — все вычисленные кадры, а не только текущий." } else if lab.view == View::Brain && lab.brain.caffeine.mode { "Проект хранит сцену, дозы, параметры PK, региональную перфузию и гипотезу E/I с её сетью. Открытие пересчитывает PK с t=0 на паузе. CSV содержит всю выбранную трассу, включая будущие относительно кадра точки." } else { "JSON проекта хранит сцену, сеть и протокол; открытие запускает их с t = 0 на паузе. CSV сети — вся вычисленная трасса, включая состояния после выбранного кадра истории." });
    });
    if submitted {
        let path = PathBuf::from(lab.file_path.trim());
        if lab.perform(action, &path) {
            lab.dialog = None;
        }
    } else if cancel || !open {
        lab.dialog = None;
    }
}
