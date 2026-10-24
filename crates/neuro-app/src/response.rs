use crate::{
    experiment::{number, parameter},
    plot::{self, Series},
    theme::*,
};
use bevy_egui::egui::{self, RichText};
use neuro_domain::{Error, ResponseModelInfo, ResponseParameters, ResponseScenario};
use neuro_engine::{
    ResponseBenchmark, ResponseEvaluation, calculate_response, evaluate_response,
    response_benchmark,
};
use std::path::PathBuf;

pub struct ResponseLab {
    pub scenario: ResponseScenario,
    pub evaluation: Option<ResponseEvaluation>,
    pub info: ResponseModelInfo,
    pub benchmark: ResponseBenchmark,
    pub draft_x: String,
    pub draft_tau: String,
    pub parameter_error: Option<String>,
    pub dirty: bool,
    pub last_scenario_path: Option<PathBuf>,
}
impl ResponseLab {
    pub fn new() -> Result<Self, Error> {
        let scenario = ResponseScenario::default();
        Ok(Self {
            evaluation: Some(evaluate_response(&scenario)?),
            draft_x: scenario.parameters.x.to_string(),
            draft_tau: scenario.parameters.tau.to_string(),
            scenario,
            info: ResponseModelInfo::bundled()?,
            benchmark: response_benchmark()?,
            parameter_error: None,
            dirty: false,
            last_scenario_path: None,
        })
    }
    pub fn refresh(&mut self) {
        match evaluate_response(&self.scenario) {
            Ok(result) => {
                self.evaluation = Some(result);
                self.parameter_error = None;
            }
            Err(e) => {
                self.evaluation = None;
                self.parameter_error = Some(e.to_string());
            }
        }
    }
    pub fn apply_drafts(&mut self) {
        let parse = |name, text: &str| {
            text.trim()
                .replace(',', ".")
                .parse::<f64>()
                .map_err(|_| format!("{name}: введите число; например, 1 или 1e6"))
        };
        let result = (|| -> Result<ResponseParameters, String> {
            let p = ResponseParameters {
                x: parse("x", &self.draft_x)?,
                tau: parse("τ", &self.draft_tau)?,
            };
            p.validate().map_err(|e| e.to_string())?;
            Ok(p)
        })();
        match result {
            Ok(p) => {
                if self.scenario.parameters != p || self.parameter_error.is_some() {
                    self.dirty |= self.scenario.parameters != p;
                    self.scenario.parameters = p;
                    self.refresh();
                }
            }
            Err(e) => {
                self.parameter_error = Some(e);
                self.evaluation = None;
            }
        }
    }
}

pub fn show(ui: &mut egui::Ui, lab: &mut ResponseLab) {
    ui.spacing_mut().item_spacing.y = 5.0;
    tag(ui, "Black–Leff · n = 1 · illustrative", PURPLE);
    muted(
        ui,
        "Один абстрактный агонист. Связывание и ответ системы различаются.",
    );
    ui.add_space(5.0);
    card().inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().interact_size.y = 22.0;
        ui.columns(2, |columns| {
            parameter(
                &mut columns[0],
                "x",
                "Свободная концентрация A / K_A",
                &mut lab.draft_x,
                &mut lab.scenario.parameters.x,
                PURPLE,
            );
            parameter(
                &mut columns[1],
                "τ",
                "Трансдукция R_T / K_E",
                &mut lab.draft_tau,
                &mut lab.scenario.parameters.tau,
                TEAL,
            );
        });
        ui.horizontal_wrapped(|ui| {
            muted(ui, "Теоретические примеры τ:");
            for tau in [0.1, 1.0, 10.0, 100.0, 1000.0] {
                if ui.small_button(number(tau)).clicked() {
                    lab.draft_tau = tau.to_string();
                }
            }
        });
        lab.apply_drafts();
        if let Some(error) = &lab.parameter_error {
            ui.colored_label(ERROR, error);
            muted(ui, "Исправьте ввод для расчёта, сохранения и экспорта.");
        }
    });
    if lab.parameter_error.is_some() {
        return;
    }
    ui.add_space(5.0);
    if let Some(result) = &lab.evaluation {
        let r = result.current;
        ui.columns(3, |columns| {
            for (i, (label, value, color)) in [
                ("Занятость θ", r.occupancy, PURPLE),
                ("Ответ E/E_m", r.fractional_response, TEAL),
                ("Предел ответа", r.response_ceiling, MUTED),
            ]
            .into_iter()
            .enumerate()
            {
                card().inner_margin(10).show(&mut columns[i], |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        RichText::new(format!("{:.2}%", value * 100.0))
                            .size(26.0)
                            .color(color),
                    );
                    ui.label(label);
                    ui.label(
                        RichText::new(format!("{value:.6e}"))
                            .size(11.0)
                            .color(MUTED),
                    );
                });
            }
        });
        match r.ec50_over_ka {
            Some(value) => {
                muted(
                    ui,
                    format!("EC50 / K_A = {value:.6e} · половина предела этой кривой"),
                );
            }
            None => {
                muted(ui, "EC50 не определена: при τ = 0 ответ всюду равен нулю.");
            }
        }
    }
    muted(
        ui,
        "E/E_m — нормированный ответ системы, не доля активных рецепторов. Параметры не измерены для кофеина.",
    );
    ui.add_space(6.0);
    card().inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Занятость и функциональный ответ").size(18.0));
            ui.label("Максимум x:");
            if ui
                .add(
                    egui::DragValue::new(&mut lab.scenario.plot.max_x)
                        .speed(1.0)
                        .custom_formatter(|v, _| number(v))
                        .range(0.001..=f64::MAX),
                )
                .changed()
            {
                lab.refresh();
                lab.dirty = true;
            }
            if ui.button("Показать текущую точку").clicked() {
                lab.scenario.plot.max_x = lab.scenario.parameters.x.max(10.0);
                lab.refresh();
                lab.dirty = true;
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(TEAL, "Ответ E/E_m");
            ui.colored_label(PURPLE, "Пунктир: занятость θ");
            ui.label("● Текущий ответ");
        });
        if let Some(result) = &lab.evaluation {
            let lines = [
                Series {
                    points: result
                        .curve
                        .iter()
                        .map(|p| (p.x, p.result.fractional_response))
                        .collect(),
                    color: TEAL,
                    dashed: false,
                },
                Series {
                    points: result
                        .curve
                        .iter()
                        .map(|p| (p.x, p.result.occupancy))
                        .collect(),
                    color: PURPLE,
                    dashed: true,
                },
            ];
            plot::show(
                ui,
                lab.scenario.plot.max_x,
                (
                    "x = [A]своб / K_A · шкала ln(1 + x), включая 0",
                    "θ и E/E_m · нормированные величины",
                ),
                &lines,
                (
                    lab.scenario.parameters.x,
                    result.current.fractional_response,
                ),
                |x| {
                    calculate_response(ResponseParameters {
                        x,
                        tau: lab.scenario.parameters.tau,
                    })
                    .ok()
                    .map(|r| {
                        (
                            r.fractional_response,
                            format!(
                                "x = {x:.6e}\nθ = {:.8}\nE/E_m = {:.8}",
                                r.occupancy, r.fractional_response
                            ),
                        )
                    })
                },
            );
        }
    });
    ui.add_space(5.0);
    if lab.benchmark.passed {
        tag(ui, "Вычислительный эталон проверен", TEAL);
    } else {
        tag(ui, "Ошибка вычислительного эталона", ERROR);
    }
    muted(
        ui,
        "Теоретические кривые без шума. Независимая экспериментальная валидация не выполнена.",
    );
}

pub fn inspector(ui: &mut egui::Ui, lab: &ResponseLab) {
    let info = &lab.info;
    tag(ui, "Опубликованная модель", TEAL);
    ui.add_space(12.0);
    ui.heading(&info.title);
    ui.add_space(10.0);
    card().show(ui, |ui| {
        for formula in &info.equations {
            ui.label(formula);
        }
    });
    ui.add_space(10.0);
    ui.label(&info.context);
    muted(ui, &info.species);
    ui.add_space(12.0);
    ui.strong("Допущения");
    for assumption in &info.assumptions {
        muted(ui, format!("• {assumption}"));
    }
    ui.add_space(12.0);
    ui.strong("Границы интерпретации");
    ui.label(&info.limitations);
    ui.add_space(12.0);
    ui.strong("Проверка вычислений");
    let benchmark = &lab.benchmark;
    ui.label(format!(
        "{} точек · max |ошибка| = {:.3e}\nRMSE = {:.3e}\nДопуск = {:.1e}",
        benchmark.points,
        benchmark.max_absolute_error,
        benchmark.rmse,
        benchmark.absolute_tolerance
    ));
    muted(ui, &info.validation_status);
    ui.add_space(12.0);
    ui.strong("Источник");
    ui.hyperlink_to(&info.source.title, &info.source.url);
    if let Some(doi) = &info.source.doi {
        ui.hyperlink_to(format!("DOI: {doi}"), format!("https://doi.org/{doi}"));
    }
    if let Some(pmid) = &info.source.pmid {
        muted(ui, format!("PMID: {pmid}"));
    }
    tag(ui, info.source.verification.label(), TEAL);
    muted(ui, format!("Проверено: {}", info.source.checked_on));
    ui.collapsing("Происхождение и права", |ui| {
        muted(ui, &info.source.rights_note);
    });
    ui.add_space(12.0);
    muted(
        ui,
        format!(
            "Модель: {}\nРевизия {} · CPU / f64",
            info.model_id, info.model_revision
        ),
    );
}
