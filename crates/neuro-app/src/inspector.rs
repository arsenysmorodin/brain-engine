use crate::{Lab, Selection, View, theme::*};
use bevy_egui::egui;

pub fn show(ui: &mut egui::Ui, lab: &mut Lab) {
    if lab.view == View::Brain {
        crate::brain::inspector(ui, &mut lab.brain);
        return;
    }
    if lab.view == View::Response {
        crate::response::inspector(ui, &lab.response);
        return;
    }
    if lab.view == View::Experiment {
        tag(ui, "Результат модели", PURPLE);
        ui.add_space(14.0);
        ui.heading("Как устроен расчёт");
        ui.add_space(8.0);
        ui.label("R + A ⇌ RA\nR + B ⇌ RB");
        ui.add_space(10.0);
        card().show(ui, |ui| {
            ui.label("Z = 1 + a + b\np_free = 1 / Z\np_A = a / Z\np_B = b / Z");
        });
        ui.add_space(10.0);
        ui.label("Единица — вес свободного рецептора. a и b — относительные веса двух связанных состояний. Деление на их сумму даёт доли.");
        ui.add_space(16.0);
        ui.label(egui::RichText::new("Допущения").size(18.0));
        for assumption in neuro_domain::ASSUMPTIONS {
            muted(ui, format!("• {assumption}"));
        }
        ui.add_space(14.0);
        tag(ui, "Граница интерпретации", TEAL);
        ui.label("Равновесие, без времени и кинетики. Лиганды A и B абстрактны. Значения Kd реальных веществ не заданы.");
        ui.label(
            "Занятость не определяет активацию, настроение, внимание или безопасность комбинаций.",
        );
        ui.add_space(12.0);
        muted(
            ui,
            format!(
                "Модель: {}\nРевизия: {} · CPU / f64\nФормат сценария: {}",
                lab.scenario.model_id, lab.scenario.model_revision, lab.scenario.format_version
            ),
        );
        return;
    }
    match &lab.selection.clone() {
        Selection::Entity(id) => {
            let entity = lab.catalog.entity(id);
            tag(
                ui,
                &entity.kind,
                if id == "caffeine" { PURPLE } else { TEAL },
            );
            ui.add_space(14.0);
            ui.heading(&entity.name);
            ui.add_space(10.0);
            ui.label(match id.as_str() {
                "caffeine" => "Выберите связь с A1 или A2A, чтобы увидеть основания для описания антагонизма.",
                "adenosine" => "Выберите связь с рецептором, чтобы изучить сведения об агонизме и связывании.",
                _ => "Отдельная молекулярная мишень. Данные о ней не объединены с другим подтипом рецептора.",
            });
            ui.add_space(16.0);
            ui.label(egui::RichText::new("Связи и источники").size(18.0));
            for claim in &lab.catalog.claims {
                if &claim.subject == id || &claim.object == id {
                    let title = format!(
                        "{} — {} — {}",
                        lab.catalog.entity(&claim.subject).name,
                        claim.relation.label(),
                        if claim.object == "a1" { "A1" } else { "A2A" }
                    );
                    if ui.add(egui::Button::new(title).wrap()).clicked() {
                        lab.selection = Selection::Claim(claim.id.clone());
                    }
                }
            }
            ui.add_space(18.0);
            ui.separator();
            ui.add_space(10.0);
            muted(
                ui,
                "В этой карте нет расчёта концентраций, дозировок или активности мозга.",
            );
            ui.add_space(14.0);
            if ui.button("Открыть эксперимент A / B").clicked() {
                lab.view = View::Experiment;
            }
        }
        Selection::Claim(id) => {
            let Some(claim) = lab.catalog.claims.iter().find(|c| &c.id == id) else {
                return;
            };
            tag(ui, "Опубликованное утверждение", TEAL);
            ui.add_space(14.0);
            ui.heading(format!(
                "{} → {}",
                lab.catalog.entity(&claim.subject).name,
                if claim.object == "a1" { "A1" } else { "A2A" }
            ));
            tag(
                ui,
                claim.relation.label(),
                if claim.relation == neuro_domain::Relation::Antagonist {
                    PURPLE
                } else {
                    TEAL
                },
            );
            ui.add_space(12.0);
            ui.label(&claim.explanation);
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);
            ui.strong("Доказательства");
            ui.label(&claim.evidence);
            muted(ui, &claim.context);
            muted(ui, &claim.species);
            ui.add_space(12.0);
            ui.strong("Границы утверждения");
            ui.label(&claim.limitations);
            ui.add_space(14.0);
            ui.strong("Источники");
            for source in lab
                .catalog
                .sources
                .iter()
                .filter(|s| claim.source_ids.contains(&s.id))
            {
                ui.hyperlink_to(&source.title, &source.url);
                if let Some(doi) = &source.doi {
                    ui.hyperlink_to(format!("DOI: {doi}"), format!("https://doi.org/{doi}"));
                }
                if let Some(pmid) = &source.pmid {
                    muted(ui, format!("PMID: {pmid}"));
                }
                tag(
                    ui,
                    source.verification.label(),
                    if source.verification == neuro_domain::Verification::Unverified {
                        ERROR
                    } else {
                        TEAL
                    },
                );
                muted(ui, format!("Проверено: {}", source.checked_on));
                ui.collapsing("Происхождение и права", |ui| {
                    muted(ui, &source.rights_note);
                });
            }
        }
    }
}
