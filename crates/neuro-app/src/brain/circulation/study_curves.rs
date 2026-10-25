use super::*;
use neuro_engine::{PkCurveFit, coupled_curve_metrics, matching_fitted_curves};

fn label(fit: &PkCurveFit) -> String {
    let study = if fit.curve.id.starts_with("mondal") {
        "Mondal 2022"
    } else {
        "Wang 2020 · смесь"
    };
    format!("{} · {}", fit.curve.substance.label(), study)
}

pub fn show(ctx: &egui::Context, c: &mut CirculationLab, atlas: &BrainAtlas) {
    if !c.show_study_curves {
        return;
    }
    let mut open = true;
    let mut load = None;
    let mut picked = None;
    egui::Window::new("Кривые исследований")
        .open(&mut open).default_pos([350.0, 100.0]).default_width(790.0).default_height(640.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label("Пять опубликованных кривых; три доступных протокола. Подбор учитывает пик и дальнейшее снижение концентрации.");
                ui.colored_label(PURPLE, "Оцифровка рисунков. Индивидуальные числовые записи не получены; это предварительная калибровка.");
                egui::ComboBox::from_id_salt("study-curve")
                    .selected_text(label(&c.fitted_report.fits[c.study_curve]))
                    .show_ui(ui, |ui| {
                        for (i, fit) in c.fitted_report.fits.iter().enumerate() {
                            ui.selectable_value(&mut c.study_curve, i, label(fit));
                        }
                    });
                let fit = &c.fitted_report.fits[c.study_curve];
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(DRUG_COLORS[fit.curve.substance.index()], fit.curve.substance.label());
                    ui.label(format!("C/M/T: {:.0}/{:.0}/{:.0} мг, перорально в 0 мин; n={}", fit.curve.doses_mg[0], fit.curve.doses_mg[1], fit.curve.doses_mg[2], fit.curve.n));
                    if let Some(index) = c.fitted_protocols.iter().position(|s| s.calibration_curve_ids.contains(&fit.curve.id))
                        { let action = ui.button("Запустить этот протокол");
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("study-launch-button"), action.rect));
                        if action.clicked() { load = Some(index); }
                    }
                });
                let matching = c.trace.as_ref().and_then(|tr| matching_fitted_curves(tr, &c.fitted_report).into_iter().find(|(f, _)| f.curve.id == fit.curve.id).map(|(_, t)| t));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut c.study_log, "Логарифмическая концентрация").on_hover_text("Равные расстояния означают одинаковые отношения концентраций. Так виден поздний спад; это не изменение данных.");
                    ui.checkbox(&mut c.study_early, "Первые 3 часа");
                });
                picked = chart(ui, fit, c.trace.as_ref().zip(matching), c.study_log, c.study_early, c.time_min);
                muted(ui, "● — опубликованная точка; усы — погрешность чтения рисунка, не SD/SEM. Серый контур — исключённый фон; ↓ — ниже LLOQ. Серый график — аналитический подбор; цветной — текущий кровеносный контур.");
                let selected = &fit.selected;
                ui.label(format!("Подбор: Cmax {:.4} мг/л при {:.1} мин. Максимум оцифрованных точек: {:.4} мг/л.", fit.fitted_cmax_mg_l, fit.fitted_tmax_min, fit.sampled_cmax_mg_l));
                ui.label(format!("Средняя ошибка подбора {:.1}%; прогноз исключаемых точек {:.1}%. Это проверка на том же исследовании, а не независимая валидация.", selected.training.mean_absolute_relative_error_pct, selected.leave_one_time_out.mean_absolute_relative_error_pct));
                if let (Some(tr), Some(t)) = (&c.trace, matching) {
                    let metrics = coupled_curve_metrics(tr, fit, t);
                    let source_end = fit.curve.observations().last().unwrap().time_min;
                    if c.time_min > t + source_end { ui.colored_label(PURPLE, "Текущее время за пределами измерений этого вещества: экстраполяция."); }
                    ui.strong(format!("Текущий контур: средняя ошибка {:.1}%, максимальная {:.1}% ({} точек).", metrics.mean_absolute_relative_error_pct, metrics.max_absolute_relative_error_pct, metrics.observations));
                } else {
                    muted(ui, "Запустите этот протокол, чтобы сверить фактическую трассу мозга. Другой состав, дозировка или повторный приём не считаются измерениями этого исследования.");
                }
                for warning in &selected.warnings { ui.colored_label(PURPLE, warning); }
                ui.collapsing("Время, измерения и отклонения", |ui| {
                    egui::Grid::new("study-observation-table").striped(true).show(ui, |ui| {
                        for title in ["Мин", "Данные, мг/л", "Подбор, мг/л", "Контур, мг/л", "Ошибка контура", "Статус"] { ui.strong(title); } ui.end_row();
                        for p in &fit.predictions {
                            let o = &p.observation;
                            ui.label(format!("{:.1}", o.time_min)); ui.label(format!("{:.5}", o.upper_limit_mg_l.unwrap_or(o.concentration_mg_l)));
                            ui.label(format!("{:.5}", p.predicted_mg_l));
                            let actual = c.trace.as_ref().zip(matching).filter(|(tr,t)| o.time_min + t <= tr.scenario.duration_min).map(|(tr,t)| tr.at(o.time_min+t).systemic[fit.curve.substance.index()].central_mg_l);
                            ui.label(actual.map_or("—".into(), |v| format!("{v:.5}")));
                            ui.label(if o.exclude.is_some() || o.upper_limit_mg_l.is_some() { "—".into() } else { actual.map_or("—".into(), |v| format!("{:+.1}%",100.0*(v/o.concentration_mg_l-1.0))) });
                            ui.label(if o.exclude.is_some() { "Фон исключён" } else if o.upper_limit_mg_l.is_some() { "Верхняя граница LLOQ" } else { "Оцифрованная точка" }); ui.end_row();
                        }
                    });
                });
                ui.collapsing("Параметры, источник и границы применимости", |ui| {
                    let p = &selected.parameters;
                    ui.label(format!("{} системных компартмента; {} ступени всасывания. V/F {:.2} л; CL/F {:.3} л/ч; ka {:.3} ч⁻¹; Vp/F {:.2} л; Qd/F {:.3} л/ч.", selected.compartments, p.absorption_stages, p.volume_l, p.clearance_l_h, p.absorption_rate_h, p.peripheral_volume_l, p.distribution_clearance_l_h));
                    muted(ui, "Vp и Qd описывают форму плазменной кривой. Они не являются измеренными объёмом органа и кровотоком. Слабая устойчивость параметров может сохраняться при хорошем совпадении графика.");
                    ui.hyperlink_to(&fit.curve.study, &fit.curve.source_url);
                    ui.hyperlink_to("Рисунок первоисточника", &fit.curve.figure_url);
                    ui.label(&fit.curve.verification);
                    muted(ui, "Подбор смеси уже учитывает её условия; парные поправки повторно не применяются. Между исследованиями различается абсолютный масштаб концентраций. Это не доказательство эффекта смеси. Тканевое распределение и E/I здесь не калибровались.");
                });
                ui.separator();
                ui.colored_label(PURPLE, "Одиночный теакрин: временного ряда для честной калибровки пока нет.");
                ui.collapsing("Каких данных не хватает", |ui| { for gap in &c.fitted_report.scope_gaps { ui.label(gap); } });
            });
        });
    c.show_study_curves = open;
    if let Some(index) = load {
        if let Err(e) = c.load_study_protocol(index, atlas) {
            c.error = Some(e.to_string());
        }
    } else if let Some(time) = picked {
        c.playing = false;
        c.set_time(time);
    }
}

fn chart(
    ui: &mut egui::Ui,
    fit: &PkCurveFit,
    trace: Option<(&CirculationTrace, f64)>,
    log: bool,
    early: bool,
    current_time: f64,
) -> Option<f64> {
    let horizon = fit.curve.observations().last().unwrap().time_min;
    let xmax = if early { 180.0 } else { horizon };
    let observations = fit.curve.observations();
    let dose = fit.curve.doses_mg[fit.curve.substance.index()];
    let points: Vec<_> = (0..=360)
        .map(|i| {
            let t = xmax * i as f64 / 360.0;
            (t, fit.selected.parameters.concentration(dose, t))
        })
        .collect();
    let max = observations
        .iter()
        .map(|o| o.concentration_mg_l * o.log_concentration_error.exp())
        .chain(points.iter().map(|(_, y)| *y))
        .chain(trace.into_iter().flat_map(|(tr, offset)| {
            tr.frames
                .iter()
                .filter(move |f| f.time_min >= offset && f.time_min <= offset + xmax)
                .map(|f| f.systemic[fit.curve.substance.index()].central_mg_l)
        }))
        .fold(0.001_f64, f64::max)
        * 1.2;
    let min = observations
        .iter()
        .filter(|o| o.exclude.is_none())
        .map(|o| o.upper_limit_mg_l.unwrap_or(o.concentration_mg_l))
        .fold(max, f64::min)
        * 0.5;
    let (ymin, ymax) = if log {
        (min.log10().floor(), max.log10().ceil())
    } else {
        (0.0, max)
    };
    let (outer, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 220.0),
        egui::Sense::click_and_drag(),
    );
    let rect = egui::Rect::from_min_max(
        outer.min + egui::vec2(64.0, 9.0),
        outer.max - egui::vec2(12.0, 28.0),
    );
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("study-curve-chart"), rect));
    let y = |value: f64| {
        let v = if log {
            value.max(10.0_f64.powf(ymin)).log10()
        } else {
            value
        };
        rect.bottom() - rect.height() * ((v - ymin) / (ymax - ymin)) as f32
    };
    let painter = ui.painter();
    for i in 0..=4 {
        let t = xmax * i as f64 / 4.0;
        let x = rect.left() + rect.width() * i as f32 / 4.0;
        painter.text(
            egui::pos2(x, rect.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            format!("{:.1} ч", t / 60.0),
            egui::FontId::proportional(11.0),
            MUTED,
        );
        let v = ymin + (ymax - ymin) * i as f64 / 4.0;
        let yy = rect.bottom() - rect.height() * i as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(rect.left(), yy), egui::pos2(rect.right(), yy)],
            egui::Stroke::new(0.5_f32, BORDER),
        );
        painter.text(
            egui::pos2(rect.left() - 6.0, yy),
            egui::Align2::RIGHT_CENTER,
            if log {
                format!("{:.4}", 10.0_f64.powf(v))
            } else {
                format!("{v:.3}")
            },
            egui::FontId::proportional(11.0),
            MUTED,
        );
    }
    let line = |points: Vec<(f64, f64)>, color| {
        let positions = points
            .into_iter()
            .map(|(t, v)| egui::pos2(rect.left() + rect.width() * (t / xmax) as f32, y(v)))
            .collect::<Vec<_>>();
        ui.painter_at(rect).add(egui::Shape::line(
            positions,
            egui::Stroke::new(1.7_f32, color),
        ));
    };
    line(points, MUTED);
    if let Some((tr, offset)) = trace {
        line(
            tr.frames
                .iter()
                .filter(|f| f.time_min >= offset && f.time_min <= offset + xmax)
                .map(|f| {
                    (
                        f.time_min - offset,
                        f.systemic[fit.curve.substance.index()].central_mg_l,
                    )
                })
                .collect(),
            DRUG_COLORS[fit.curve.substance.index()],
        );
    }
    draw_observations(ui, rect, fit, 0.0, xmax, y);
    if let Some((_, offset)) = trace
        && (0.0..=xmax).contains(&(current_time - offset))
    {
        let x = rect.left() + rect.width() * ((current_time - offset) / xmax) as f32;
        ui.painter_at(rect).line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0_f32, TEXT),
        );
    }
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
        && rect.contains(pos)
    {
        return trace
            .map(|(_, offset)| offset + ((pos.x - rect.left()) / rect.width()) as f64 * xmax);
    }
    None
}

pub(super) fn draw_observations(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    fit: &PkCurveFit,
    offset: f64,
    xmax: f64,
    y: impl Fn(f64) -> f32,
) {
    let painter = ui.painter_at(rect);
    for (i, o) in fit.curve.observations().iter().enumerate() {
        let t = o.time_min + offset;
        if t > xmax {
            continue;
        }
        let concentration = o.upper_limit_mg_l.unwrap_or(o.concentration_mg_l);
        let point = egui::pos2(
            rect.left() + rect.width() * (t / xmax) as f32,
            y(concentration),
        );
        let color = if o.exclude.is_some() {
            MUTED
        } else {
            DRUG_COLORS[fit.curve.substance.index()]
        };
        if o.exclude.is_some() {
            painter.circle_stroke(point, 3.5, egui::Stroke::new(1.3_f32, color));
        } else if o.upper_limit_mg_l.is_some() {
            painter.text(
                point,
                egui::Align2::CENTER_CENTER,
                "↓",
                egui::FontId::proportional(18.0),
                color,
            );
        } else {
            painter.line_segment(
                [
                    egui::pos2(point.x, y(concentration * o.log_concentration_error.exp())),
                    egui::pos2(
                        point.x,
                        y(concentration * (-o.log_concentration_error).exp()),
                    ),
                ],
                egui::Stroke::new(1.0_f32, color),
            );
            let dx = rect.width() * (o.time_error_min / xmax) as f32;
            painter.line_segment(
                [point - egui::vec2(dx, 0.0), point + egui::vec2(dx, 0.0)],
                egui::Stroke::new(1.0_f32, color),
            );
            painter.circle_filled(point, 3.2, color);
        }
        ui.interact(
            egui::Rect::from_center_size(point, egui::vec2(14.0, 14.0)),
            egui::Id::new(("study-observation", &fit.curve.id, i, rect.min.x.to_bits())),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "{} · {:.1} мин: {:.5} мг/л. {}",
            fit.curve.study,
            o.time_min,
            concentration,
            o.exclude
                .as_deref()
                .unwrap_or(if o.upper_limit_mg_l.is_some() {
                    "Верхняя граница LLOQ"
                } else {
                    "Погрешность оцифровки; не SD/SEM участников"
                })
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_fits_keep_source_points_and_model_predictions_in_sync() {
        let a = BrainAtlas::bundled().unwrap();
        let mut c = CirculationLab::new(&a).unwrap();
        let data = neuro_domain::PkCurveDataset::bundled().unwrap();
        assert_eq!(c.fitted_report.dataset_id, data.id);
        assert_eq!(c.fitted_report.fits.len(), data.curves.len());
        assert_eq!(c.fitted_protocols.len(), 3);
        for (fit, curve) in c.fitted_report.fits.iter().zip(&data.curves) {
            assert_eq!(
                serde_json::to_value(&fit.curve).unwrap(),
                serde_json::to_value(curve).unwrap()
            );
            for point in &fit.predictions {
                assert!(
                    (fit.selected.parameters.concentration(
                        curve.doses_mg[curve.substance.index()],
                        point.observation.time_min
                    ) - point.predicted_mg_l)
                        .abs()
                        < 1e-12
                );
            }
        }
        assert!(
            !c.fitted_report
                .fits
                .iter()
                .any(|f| f.curve.doses_mg == [0.0, 0.0, 125.0])
        );
        let before = c.scenario.clone();
        assert!(c.load_study_protocol(999, &a).is_err());
        assert_eq!(c.scenario, before);
    }
    #[test]
    fn study_launch_button_runs_selected_complete_protocol() {
        let a = BrainAtlas::bundled().unwrap();
        let mut c = CirculationLab::new(&a).unwrap();
        c.study_curve = 1;
        c.show_study_curves = true;
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1380.0, 900.0));
        for _ in 0..2 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ctx| show(ctx, &mut c, &a),
            );
        }
        let rect = ctx
            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("study-launch-button")))
            .unwrap();
        assert!(screen.contains_rect(rect));
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| show(ctx, &mut c, &a),
            );
        }
        assert_eq!(c.scenario.calibration_curve_ids, ["mondal2022-c150-mean"]);
        assert!(c.trace.is_some() && c.mode && c.show_plot && c.playing && c.dirty);
        assert_eq!(c.selected, Purine::Caffeine);
        assert!(!c.scenario.interactions);
    }
}
