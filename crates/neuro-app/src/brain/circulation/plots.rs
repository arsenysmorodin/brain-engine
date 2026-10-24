use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PurinePlot {
    Plasma,
    Regional,
    Flow,
    Flux,
    Mass,
    Occupancy,
    Neural,
}
impl PurinePlot {
    fn label(self) -> &'static str {
        match self {
            Self::Plasma => "Плазма, мг/л",
            Self::Regional => "Местные C, мг/л",
            Self::Flow => "CBF, мл/(100 г·мин)",
            Self::Flux => "Потоки, мг/мин",
            Self::Mass => "Масса, мг",
            Self::Occupancy => "Занятость PET, 0…1",
            Self::Neural => "Сравнение E/I",
        }
    }
}
pub fn show(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.circulation;
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("purine-plot-kind")
            .selected_text(c.plot.label())
            .show_ui(ui, |ui| {
                for p in [
                    PurinePlot::Plasma,
                    PurinePlot::Regional,
                    PurinePlot::Flow,
                    PurinePlot::Flux,
                    PurinePlot::Mass,
                    PurinePlot::Neural,
                ] {
                    ui.selectable_value(&mut c.plot, p, p.label());
                }
                if c.selected == Purine::Caffeine {
                    ui.selectable_value(
                        &mut c.plot,
                        PurinePlot::Occupancy,
                        PurinePlot::Occupancy.label(),
                    );
                }
            });
        if c.plot != PurinePlot::Neural {
            ui.checkbox(&mut c.early, "0–5 мин");
        }
        match c.plot {
            PurinePlot::Plasma => {
                for p in Purine::ALL {
                    ui.colored_label(DRUG_COLORS[p.index()], p.label());
                }
            }
            PurinePlot::Regional => {
                for (color, label) in [
                    (MUTED, "Cp"),
                    (DRUG_COLORS[0], "Ccap"),
                    (TEAL, "Ct"),
                    (PURPLE, "Cv"),
                ] {
                    ui.colored_label(color, label);
                }
            }
            PurinePlot::Flux => {
                ui.colored_label(DRUG_COLORS[0], "Вход");
                ui.colored_label(PURPLE, "Возврат");
            }
            PurinePlot::Mass => {
                for (color, label) in [
                    (TEAL, "ЖКТ"),
                    (DRUG_COLORS[0], "Организм"),
                    (PURPLE, "Мозг"),
                    (MUTED, "Устранено"),
                ] {
                    ui.colored_label(color, label);
                }
            }
            PurinePlot::Occupancy => {
                ui.colored_label(TEAL, "A1");
                ui.colored_label(PURPLE, "A2A, PD n=5");
            }
            _ => {}
        }
    });
    if c.plot == PurinePlot::Neural {
        neural(ui, c, lab.scene.selected_region.as_deref(), &lab.atlas);
        return;
    }
    if c.plot == PurinePlot::Occupancy && c.selected != Purine::Caffeine {
        muted(
            ui,
            "Количественная человеческая PET-занятость для M/T неизвестна. Выберите другой график; данные кофеина к ним не переносятся.",
        );
        return;
    }
    let Some(trace) = &c.trace else {
        muted(ui, "Пересчитайте протокол, чтобы получить графики.");
        return;
    };
    let d = c.selected.index();
    let xmax = if c.early {
        5.0
    } else {
        c.scenario.duration_min
    };
    let bed = lab
        .scene
        .selected_region
        .as_deref()
        .map_or(Some(0), |id| c.bed_index(id));
    if matches!(
        c.plot,
        PurinePlot::Regional | PurinePlot::Flow | PurinePlot::Flux
    ) && bed.is_none()
    {
        muted(
            ui,
            "Для выбранной полости нет модели перфузии. Выберите область ткани на 3D-мозге.",
        );
        return;
    }
    let bed = bed.unwrap_or(0);
    let frames: Vec<_> = trace.frames.iter().filter(|f| f.time_min <= xmax).collect();
    let series = |value: fn(&CirculationFrame, usize, usize) -> f64| {
        frames
            .iter()
            .map(|f| (f.time_min, value(f, bed, d)))
            .collect::<Vec<_>>()
    };
    let (_unit, lines): (&str, Vec<super::super::caffeine::PlotSeries>) = match c.plot {
        PurinePlot::Plasma => (
            "Cp, мг/л",
            Purine::ALL
                .iter()
                .map(|p| {
                    (
                        DRUG_COLORS[p.index()],
                        frames
                            .iter()
                            .map(|f| (f.time_min, f.systemic[p.index()].central_mg_l))
                            .collect(),
                    )
                })
                .collect(),
        ),
        PurinePlot::Regional => (
            "Концентрация, мг/л",
            vec![
                (MUTED, series(|f, _, d| f.systemic[d].central_mg_l)),
                (
                    DRUG_COLORS[0],
                    series(|f, i, d| f.regions[i][d].capillary_mg_l),
                ),
                (TEAL, series(|f, i, d| f.regions[i][d].tissue_mg_l)),
                (PURPLE, series(|f, i, d| f.regions[i][d].venous_mg_l)),
            ],
        ),
        PurinePlot::Flow => (
            "CBF, мл/(100 г·мин)",
            vec![(TEAL, series(|f, i, _| f.cbf_ml_100g_min[i]))],
        ),
        PurinePlot::Flux => (
            "Поток вещества, мг/мин",
            vec![
                (
                    DRUG_COLORS[0],
                    series(|f, i, d| f.regions[i][d].inflow_mg_min),
                ),
                (PURPLE, series(|f, i, d| f.regions[i][d].return_mg_min)),
            ],
        ),
        PurinePlot::Mass => (
            "Количество, мг",
            vec![
                (TEAL, series(|f, _, d| f.systemic[d].gut_mg)),
                (DRUG_COLORS[0], series(|f, _, d| f.systemic[d].rest_mg)),
                (PURPLE, series(|f, _, d| f.systemic[d].brain_mg)),
                (MUTED, series(|f, _, d| f.systemic[d].eliminated_mg)),
            ],
        ),
        PurinePlot::Occupancy => (
            "Доля занятости, 0…1",
            vec![
                (
                    TEAL,
                    series(|f, _, _| {
                        neuro_engine::caffeine_pet_estimates(f.systemic[0].central_mg_l).0
                    }),
                ),
                (
                    PURPLE,
                    series(|f, _, _| {
                        neuro_engine::caffeine_pet_estimates(f.systemic[0].central_mg_l).1
                    }),
                ),
            ],
        ),
        PurinePlot::Neural => unreachable!(),
    };
    let ymax = if c.plot == PurinePlot::Occupancy {
        1.0
    } else {
        lines
            .iter()
            .flat_map(|(_, p)| p.iter().map(|(_, y)| *y))
            .fold(0.1_f64, f64::max)
            * 1.1
    };
    let picked = super::super::caffeine::chart(
        ui,
        "purine-detail-chart",
        xmax,
        ymax,
        c.time_min.min(xmax),
        &lines,
        if c.early || xmax <= 120.0 { "min" } else { "h" },
    );
    if let Some(rect) = ui
        .ctx()
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("purine-detail-chart")))
    {
        let painter = ui.painter_at(rect);
        for dose in &c.scenario.doses {
            if dose.time_min <= xmax && dose.amount_mg > 0.0 {
                let x = rect.left() + rect.width() * (dose.time_min / xmax) as f32;
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    egui::Stroke::new(
                        0.7_f32,
                        DRUG_COLORS[dose.substance.index()].gamma_multiply(0.4),
                    ),
                );
            }
        }
    }
    if let Some(t) = picked {
        c.playing = false;
        c.set_time(t);
    }
    let mut time = c.time_min;
    if ui
        .add(
            egui::Slider::new(&mut time, 0.0..=c.scenario.duration_min)
                .show_value(false)
                .text("Время PK"),
        )
        .changed()
    {
        c.playing = false;
        c.set_time(time);
    }
    if let Some(f) = c.frame() {
        let s = f.systemic[d];
        muted(
            ui,
            format!(
                "Баланс: ЖКТ {:.2} + организм {:.2} + мозг {:.3} + устранено {:.2} = {:.2} мг; ошибка {:.1e} мг",
                s.gut_mg,
                s.rest_mg,
                s.brain_mg,
                s.eliminated_mg,
                s.administered_mg,
                s.balance_error_mg
            ),
        );
    }
}
fn neural(ui: &mut egui::Ui, c: &mut CirculationLab, selected: Option<&str>, atlas: &BrainAtlas) {
    let Some(comparison) = &c.comparison else {
        muted(
            ui,
            "Включите учебную гипотезу слева и нажмите «Сравнить E/I» справа. Изменение PK-времени очищает старое сравнение.",
        );
        return;
    };
    let id = selected.unwrap_or(&comparison.baseline.scenario.populations[0].region_id);
    let Some(n) = comparison
        .baseline
        .scenario
        .populations
        .iter()
        .position(|p| p.region_id == id)
    else {
        muted(
            ui,
            "Для выбранной структуры нет E/I-популяций. Выберите серое вещество.",
        );
        return;
    };
    muted(
        ui,
        format!(
            "{} · PK {:.2} мин · E: серый без входа, бирюзовый с гипотезой; I: фиолетовый",
            atlas.region(id).map_or(id, |r| r.name.as_str()),
            comparison.pk_time_min
        ),
    );
    let lines = vec![
        (
            MUTED,
            comparison
                .baseline
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].e))
                .collect(),
        ),
        (
            TEAL,
            comparison
                .with_hypothesis
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].e))
                .collect(),
        ),
        (
            PURPLE,
            comparison
                .with_hypothesis
                .samples
                .iter()
                .map(|s| (s.time_ms, s.states[n].i))
                .collect(),
        ),
    ];
    if let Some(t) = super::super::caffeine::chart(
        ui,
        "purine-neural-chart",
        comparison.baseline.elapsed_ms,
        1.0,
        c.neural_time_ms,
        &lines,
        "ms",
    ) {
        c.neural_time_ms = t;
        c.revision = c.revision.wrapping_add(1);
    }
    muted(
        ui,
        "g и S — выбранные параметры учебной модели. ΔE не является клиническим эффектом вещества.",
    );
}
