use crate::{FileAction, Lab, theme::*};
use bevy_egui::egui;
use std::path::{Path, PathBuf};
pub fn show(ctx: &egui::Context, lab: &mut Lab) {
    let Some(action) = lab.dialog else {
        return;
    };
    let mut open = true;
    let mut submitted = false;
    let mut cancel = false;
    egui::Window::new(action.title()).open(&mut open).collapsible(false).resizable(true).default_width(650.0).show(ctx, |ui| {
        ui.label("Полный путь к файлу");
        ui.add(egui::TextEdit::singleline(&mut lab.file_path).desired_width(f32::INFINITY));
        if matches!(action, FileAction::Open) {
            if lab.dirty { ui.colored_label(PURPLE, "Открытие заменит текущие несохранённые параметры."); }
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
            let label = if exists { "Заменить файл" } else { match action { FileAction::Save => "Сохранить JSON", FileAction::Open => "Открыть и пересчитать", FileAction::Csv => "Экспортировать CSV" } };
            submitted = ui.button(label).clicked(); cancel = ui.button("Отмена").clicked();
        });
        muted(ui, "JSON сохраняет параметры, версию модели и диапазон графика. CSV содержит текущую точку и кривые.");
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
