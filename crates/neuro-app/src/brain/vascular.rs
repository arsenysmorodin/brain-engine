//! Arterial anatomy and the explicitly assumed regional perfusion experiment.
use super::{
    BrainLab,
    caffeine::{BLOOD, CaffeineLab, CaffeineLayer, CaffeinePlot},
};
use crate::theme::*;
use bevy_egui::egui::{self, Color32, RichText};
use neuro_domain::{ArterialTerritory, CaffeinePerfusion};

pub fn territory_rgb(t: ArterialTerritory) -> [u8; 3] {
    match t {
        ArterialTerritory::AcaLeft => [101, 218, 201],
        ArterialTerritory::AcaRight => [95, 166, 155],
        ArterialTerritory::McaLeft => [255, 178, 82],
        ArterialTerritory::McaRight => [207, 136, 82],
        ArterialTerritory::PcaLeft => [185, 161, 244],
        ArterialTerritory::PcaRight => [142, 122, 207],
        ArterialTerritory::Vertebrobasilar => [108, 166, 229],
    }
}
fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
fn fields(ui: &mut egui::Ui, rows: impl IntoIterator<Item = (String, String)>) {
    egui::Grid::new(ui.id().with("vascular-values"))
        .num_columns(2)
        .spacing([9.0, 5.0])
        .show(ui, |ui| {
            for (label, value) in rows {
                ui.label(label);
                ui.strong(value);
                ui.end_row();
            }
        });
}
pub fn view_controls(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let c = &mut lab.caffeine;
    let before = (c.show_vessels, c.tissue_opacity);
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut c.show_vessels, "Артерии");
        ui.add(
            egui::Slider::new(&mut c.tissue_opacity, 0.0..=1.0)
                .text("Плотность ткани")
                .max_decimals(2),
        );
        if ui.small_button("Слева").clicked() {
            lab.scene.camera.yaw = -std::f32::consts::FRAC_PI_2;
            lab.scene.camera.pitch = 0.0;
        }
        if ui.small_button("Центр").clicked() {
            lab.scene.camera = neuro_domain::BrainCamera::default();
        }
        if ui
            .small_button("Снизу")
            .on_hover_text("Виллизиев круг и основания артерий")
            .clicked()
        {
            lab.scene.camera.pitch = -0.9;
            lab.scene.camera.yaw = std::f32::consts::PI;
            lab.scene.camera.distance = 4.6;
            lab.scene.camera.target = [0.0; 3];
        }
    });
    if c.show_vessels {
        lab.scene.separation = 0.0;
    }
    if before != (c.show_vessels, c.tissue_opacity) {
        c.revision = c.revision.wrapping_add(1);
    }
}
pub fn sidebar(ui: &mut egui::Ui, lab: &mut BrainLab) {
    let before = lab.caffeine.scenario.clone();
    egui::ScrollArea::vertical().id_salt("vascular-sidebar").auto_shrink([false,false]).show(ui,|ui| {
        ui.heading("Кровоснабжение");
        let mut enabled=lab.caffeine.scenario.perfusion.is_some();
        if ui.checkbox(&mut enabled,"Региональная доставка").changed() {
            lab.caffeine.scenario.perfusion=enabled.then(||CaffeinePerfusion::reference(&lab.atlas));
            lab.caffeine.comparison=None;
        }
        if let Some(p)=&mut lab.caffeine.scenario.perfusion {
            ui.collapsing("Сосудистый ответ кофеина",|ui| {
                ui.checkbox(&mut p.vasoconstriction_enabled,"Снижение CBF с концентрацией");
                super::caffeine::parameter(ui,"Макс. снижение",&mut p.max_flow_reduction,0.0..=0.8,0.01);
                super::caffeine::parameter(ui,"C50 потока, мг/л",&mut p.flow_half_effect_mg_l,0.1..=20.0,0.1);
                muted(ui,"Форма и коэффициенты — гипотеза. Не PET-параметры.");
            });
            if let Some(id)=&lab.scene.selected_region && let Some(r)=p.regions.iter_mut().find(|r|&r.region_id==id) {
                ui.add_space(6.0);
                ui.strong("Перфузия выбранной области");
                egui::ComboBox::from_id_salt("regional-territory").width(ui.available_width()).selected_text(r.territory.label()).show_ui(ui,|ui| {
                    for t in ArterialTerritory::ALL {ui.selectable_value(&mut r.territory,t,t.label());}
                });
                super::caffeine::parameter(ui,"CBF₀, мл/100 г/мин",&mut r.baseline_cbf_ml_100g_min,1.0..=150.0,1.0);
                super::caffeine::parameter(ui,"Множитель потока",&mut r.flow_fraction,0.0..=2.0,0.02);
                super::caffeine::parameter(ui,"Задержка, с",&mut r.transit_sec,0.0..=30.0,0.5);
                super::caffeine::parameter(ui,"Масса ткани, г",&mut r.tissue_mass_g,0.1..=500.0,1.0);
                super::caffeine::parameter(ui,"Kp ткань/кровь",&mut r.partition_tissue_blood,0.5..=3.0,0.05);
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Поток ×0.3").clicked() {r.flow_fraction=0.3;}
                    if ui.small_button("Поток ×1").clicked() {r.flow_fraction=1.0;}
                });
                muted(ui,"Редактируемые допущения; бассейн приблизительный, масса не измерена по мешу.");
            } else {muted(ui,"Щёлкните по ткани на 3D-мозге или выберите область в списке.");}
            ui.add_space(6.0);
            if ui.button("Сбросить перфузию").clicked() { *p=CaffeinePerfusion::reference(&lab.atlas); }
        } else {muted(ui,"Общий Ce сохраняет старый режим. Включите региональную доставку для потоков и Ct.");}
        ui.separator();
        ui.strong(format!("Артерии · {} структур",lab.vascular_atlas.vessels.len()));
        ui.add(egui::TextEdit::singleline(&mut lab.caffeine.vessel_search).hint_text("Артерия или FMA ID").desired_width(ui.available_width()));
        let search=lab.search.trim().to_lowercase();
        egui::ScrollArea::vertical().id_salt("vascular-list").max_height(235.0).auto_shrink([false,false]).show(ui,|ui| {
            for (i,v) in lab.vascular_atlas.vessels.iter().enumerate() {
                if !search.is_empty() && !format!("{} {} {} {}",v.name,v.name_en,v.id,v.supply).to_lowercase().contains(&search) {continue;}
                if ui.add(egui::Button::new(RichText::new(&v.name).size(11.5)).selected(lab.caffeine.selected_vessel==Some(i)).wrap().min_size(egui::vec2(ui.available_width(),29.0))).on_hover_text(format!("{}\n{}",v.id,v.name_en)).clicked() {
                    lab.caffeine.selected_vessel=Some(i);lab.caffeine.selection=None;lab.caffeine.revision+=1;
                }
            }
        });
        muted(ui,"Геометрия BodyParts3D; вены и капилляры не включены.");
        ui.add_space(6.0);
        ui.collapsing("Бассейны сейчас",|ui| {
            if let Some(t)=&lab.caffeine.trace && let Some(tr)=&t.perfusion {
                for territory in ArterialTerritory::ALL {
                    let (q,j)=tr.territory_flow(&t.scenario,lab.caffeine.time_min,territory);
                    ui.colored_label(color(territory_rgb(territory)),territory.label());
                    muted(ui,format!("{q:.1} мл/мин · {j:.4} мг/мин"));
                }
            }
            muted(ui,"Суммы региональных участков, не измеренные потоки отдельных артерий.");
        });
    });
    if lab.caffeine.scenario != before {
        lab.caffeine.refresh(&lab.atlas);
    }
}
/// Geometry carries supply-family colors only. No radius/velocity/pressure is inferred.
pub fn vessel_rgb(lab: &BrainLab, i: usize) -> [u8; 3] {
    let c = &lab.caffeine;
    if c.selected_vessel == Some(i) {
        return [234, 240, 246];
    }
    let v = &lab.vascular_atlas.vessels[i];
    if v.supply == "communicating" {
        return [120, 130, 147];
    }
    let Some(config) = &c.scenario.perfusion else {
        return [217, 104, 103];
    };
    let Some(tr) = c.trace.as_ref().and_then(|t| t.perfusion.as_ref()) else {
        return [65, 69, 77];
    };
    let mut weighted = 0.0;
    let mut weight = 0.0;
    for (r, s) in config.regions.iter().zip(&tr.at(c.time_min).regions) {
        let include = v.supply == r.territory.key()
            || (v.supply == "ica_left"
                && matches!(
                    r.territory,
                    ArterialTerritory::AcaLeft | ArterialTerritory::McaLeft
                ))
            || (v.supply == "ica_right"
                && matches!(
                    r.territory,
                    ArterialTerritory::AcaRight | ArterialTerritory::McaRight
                ));
        if include {
            let q = s.cbf_ml_100g_min * r.tissue_mass_g;
            weighted += q * s.arterial_mg_l;
            weight += q;
        }
    }
    if weight == 0.0 {
        return [65, 69, 77];
    }
    if c.layer == CaffeineLayer::Territories {
        return ArterialTerritory::ALL
            .into_iter()
            .find(|t| t.key() == v.supply)
            .map_or([217, 104, 103], territory_rgb);
    }
    let ca = weighted / weight;
    let f = (ca / 5.0).clamp(0.0, 1.0).sqrt() as f32;
    std::array::from_fn(|i| {
        ([147.0, 75.0, 85.0][i] + f * ([255.0, 206.0, 113.0][i] - [147.0, 75.0, 85.0][i])) as u8
    })
}
pub fn legend(ui: &mut egui::Ui, c: &CaffeineLab) -> bool {
    match c.layer {
        CaffeineLayer::Territories => {
            ui.horizontal_wrapped(|ui| {
                for t in ArterialTerritory::ALL {
                    ui.colored_label(color(territory_rgb(t)), t.label());
                }
            });
        }
        CaffeineLayer::Tissue | CaffeineLayer::Cbf | CaffeineLayer::Delivery => {
            let (max, unit) = match c.layer {
                CaffeineLayer::Tissue => (10.0, "мг/л"),
                CaffeineLayer::Cbf => (100.0, "мл/100 г/мин"),
                _ => (300.0, "мкг/100 г/мин"),
            };
            ui.horizontal_wrapped(|ui| {
                ui.small("Ткань:");
                for f in [0.0, 0.5, 1.0] {
                    ui.colored_label(
                        color(super::simulation::heat_rgb(f)),
                        format!("{:.0}{}", f * max, if f == 1.0 { "+" } else { "" }),
                    );
                }
                ui.small(unit);
                ui.small("Серый — нет расчёта");
            });
        }
        _ => return false,
    }
    if c.show_vessels {
        ui.label(RichText::new(if c.layer==CaffeineLayer::Territories {"Артерии: семейства снабжения ≈ · белый — выбранный сосуд"} else {"Артерии: Ca 0 → 5+ мг/л, тёмно-красный → янтарный"}).size(10.5).color(MUTED)).on_hover_text("Цвет связан с модельным бассейном, не со скоростью в сосуде. Соединительные артерии серые: переток не рассчитан. Региональные параметры — редактируемые допущения.");
    }
    true
}
pub fn inspector(ui: &mut egui::Ui, lab: &mut BrainLab) -> bool {
    let vascular_layer = matches!(
        lab.caffeine.layer,
        CaffeineLayer::Tissue
            | CaffeineLayer::Cbf
            | CaffeineLayer::Delivery
            | CaffeineLayer::Territories
    );
    if lab.caffeine.route || (!vascular_layer && lab.caffeine.selected_vessel.is_none()) {
        return false;
    }
    tag(ui, "Кровоток и доставка", TEAL);
    ui.add_space(7.0);
    if let Some(i) = lab.caffeine.selected_vessel {
        let v = &lab.vascular_atlas.vessels[i];
        ui.heading(&v.name);
        muted(ui, format!("{} · {}", v.id, v.name_en));
        ui.label(if lab.caffeine.layer==CaffeineLayer::Territories {"Анатомическая геометрия BodyParts3D. Цвет обозначает семейство снабжения; бассейны целых структур приблизительны."} else {"Анатомическая геометрия BodyParts3D в исходном положении. Цвет отражает среднюю артериальную концентрацию связанного модельного бассейна, не скорость крови в этом сосуде."});
        if v.supply == "communicating" {
            muted(
                ui,
                "Коллатеральный поток и направление через соединительные артерии не рассчитаны.",
            );
        } else {
            muted(ui, format!("Группа снабжения: {}", v.supply));
        }
        if ui.button("Вернуться к ткани").clicked() {
            lab.caffeine.selected_vessel = None;
            lab.caffeine.revision += 1;
        }
    } else if let Some(id) = lab.scene.selected_region.clone()
        && let Some(r) = lab.atlas.region(&id)
    {
        ui.heading(&r.name);
        muted(ui, &r.id);
        if let Some((p, s)) = lab.caffeine.regional(&id) {
            tag(ui, "Расчёт · параметры illustrative", PURPLE);
            fields(
                ui,
                [
                    ("Бассейн ≈".into(), p.territory.label().into()),
                    (
                        "Артериальная Ca".into(),
                        format!("{:.4} мг/л", s.arterial_mg_l),
                    ),
                    ("В ткани Ct".into(), format!("{:.4} мг/л", s.tissue_mg_l)),
                    (
                        "CBF".into(),
                        format!("{:.2} мл/100 г/мин", s.cbf_ml_100g_min),
                    ),
                    (
                        "Объёмный поток".into(),
                        format!("{:.2} мл/мин", s.cbf_ml_100g_min * p.tissue_mass_g / 100.0),
                    ),
                    (
                        "Приносится".into(),
                        format!("{:.3} мкг/мин", s.inflow_mg_min * 1000.0),
                    ),
                    (
                        "Уносится".into(),
                        format!("{:.3} мкг/мин", s.outflow_mg_min * 1000.0),
                    ),
                    ("В участке".into(), format!("{:.4} мг", s.tissue_mg)),
                ],
            );
            ui.collapsing("Проверка локального баланса",|ui|{
                ui.label(format!("Пришло {:.6} мг\nУшло {:.6} мг\nОсталось {:.6} мг\nНевязка {:.2e} мг",s.cumulative_in_mg,s.cumulative_out_mg,s.tissue_mg,s.cumulative_in_mg-s.cumulative_out_mg-s.tissue_mg));
                muted(ui,"Повторный проход с кровью входит в интеграл притока. Это не уникальная поглощённая доза.");
            });
        } else {
            muted(
                ui,
                "У полостей нет тканевой перфузии; без расширения локальные значения неизвестны.",
            );
        }
        if ui.button("Приблизить область").clicked() {
            lab.focus_selected();
        }
    } else {
        ui.heading("Артерии мозга");
        ui.label("Выберите сосуд или участок ткани на мозге. Мышь вращает сцену, колесо приближает. Снизьте плотность ткани, чтобы рассмотреть внутренние артерии.");
        if let (Some(p), Some(t)) = (
            &lab.caffeine.scenario.perfusion,
            lab.caffeine
                .trace
                .as_ref()
                .and_then(|t| t.perfusion.as_ref()),
        ) {
            let f = t.at(lab.caffeine.time_min);
            let q: f64 = p
                .regions
                .iter()
                .zip(&f.regions)
                .map(|(r, s)| r.tissue_mass_g * s.cbf_ml_100g_min / 100.0)
                .sum();
            let jin: f64 = f.regions.iter().map(|s| s.inflow_mg_min).sum();
            fields(
                ui,
                [
                    ("Модельных участков".into(), p.regions.len().to_string()),
                    ("Суммарный поток".into(), format!("{q:.1} мл/мин")),
                    ("Суммарный приток".into(), format!("{jin:.4} мг/мин")),
                ],
            );
        }
    }
    ui.add_space(8.0);
    if ui
        .checkbox(
            &mut lab.caffeine.scenario.neural_hypothesis.enabled,
            "Гипотеза связи с E/I",
        )
        .changed()
    {
        lab.caffeine.refresh(&lab.atlas);
    }
    if ui
        .add_enabled(
            lab.caffeine.scenario.neural_hypothesis.enabled && lab.caffeine.trace.is_some(),
            egui::Button::new("Сравнить E/I при этом времени"),
        )
        .clicked()
    {
        lab.caffeine.selected_vessel = None;
        lab.caffeine.compute_neural(&lab.atlas);
    }
    if let Some(e) = &lab.caffeine.neural_error {
        ui.colored_label(ERROR, e);
    }
    muted(
        ui,
        "Местная Ct → дополнительный вход E/I: непроверенная гипотеза. CBF не равен активности.",
    );
    ui.separator();
    ui.collapsing("Модель, источники и границы",|ui| {
        ui.label("Приток → хорошо перемешанная ткань → отток. Артериальная Cp задана общей PK, венозный возврат на неё не влияет. Региональную массу нельзя прибавлять к системной.");
        muted(ui,"Масса, CBF₀, Kp, задержка и концентрационный ответ выбраны как допущения. Бассейн всей извилины приблизителен; воксельной карты нет.");
        for s in &lab.caffeine.perfusion_knowledge.sources {ui.hyperlink_to(&s.title,&s.url);muted(ui,&s.context);muted(ui,&s.verification);}
        ui.hyperlink_to("BodyParts3D · CC BY 4.0",&lab.vascular_atlas.source.url);
        muted(ui,"Нет модели давления, коллатералей, капиллярной/венозной геометрии, кислорода или ишемии.");
    });
    true
}
pub fn plot(ui: &mut egui::Ui, c: &mut CaffeineLab, id: Option<&str>) {
    let Some(config) = &c.scenario.perfusion else {
        muted(
            ui,
            "Включите региональную доставку в настройках кровоснабжения.",
        );
        return;
    };
    let Some(trace) = c.trace.as_ref().and_then(|t| t.perfusion.as_ref()) else {
        muted(ui, "Нет расчёта перфузии.");
        return;
    };
    let index = id.and_then(|id| config.regions.iter().position(|r| r.region_id == id));
    if id.is_some() && index.is_none() {
        muted(ui, "Для этой структуры перфузия не задана.");
        return;
    }
    let selected = if index.is_some() {
        "Выбранная область"
    } else {
        "Среднее по массе ткани"
    };
    let (title, lines): (String, Vec<super::caffeine::PlotSeries>) = match c.plot {
        CaffeinePlot::RegionalFlow => (
            format!(
                "{selected} · CBF, мл/100 г/мин · серый = без сосудистого ответа, бирюзовый = с ответом"
            ),
            vec![(MUTED, Vec::new()), (TEAL, Vec::new())],
        ),
        CaffeinePlot::RegionalFlux => (
            format!("{selected} · мкг/100 г/мин · оранжевый = приток, фиолетовый = отток"),
            vec![(BLOOD, Vec::new()), (PURPLE, Vec::new())],
        ),
        _ => (
            format!("{selected} · мг/л · Ca оранжевый, Ct бирюзовый"),
            vec![(BLOOD, Vec::new()), (TEAL, Vec::new())],
        ),
    };
    let mut lines = lines;
    for f in &trace.frames {
        let (mut a, mut b, mut mass) = (0.0, 0.0, 0.0);
        for (i, (r, s)) in config.regions.iter().zip(&f.regions).enumerate() {
            if index.is_some_and(|n| n != i) {
                continue;
            }
            let (x, y) = match c.plot {
                CaffeinePlot::RegionalFlow => (
                    r.baseline_cbf_ml_100g_min * r.flow_fraction,
                    s.cbf_ml_100g_min,
                ),
                CaffeinePlot::RegionalFlux => (
                    s.inflow_mg_min * 100_000.0 / r.tissue_mass_g,
                    s.outflow_mg_min * 100_000.0 / r.tissue_mass_g,
                ),
                _ => (s.arterial_mg_l, s.tissue_mg_l),
            };
            a += x * r.tissue_mass_g;
            b += y * r.tissue_mass_g;
            mass += r.tissue_mass_g;
        }
        lines[0].1.push((f.time_min, a / mass));
        lines[1].1.push((f.time_min, b / mass));
    }
    muted(ui, title);
    let ymax = lines
        .iter()
        .flat_map(|(_, s)| s.iter().map(|(_, y)| *y))
        .fold(0.1_f64, f64::max)
        * 1.05;
    let early = c.time_min <= 5.0;
    let xmax = if early { 5.0 } else { c.scenario.duration_min };
    // Keep full trace for exact cursor picking; chart clips outside its plot rectangle.
    for (_, points) in &mut lines {
        points.retain(|(x, _)| *x <= xmax);
    }
    let ymax = if early {
        lines
            .iter()
            .flat_map(|(_, s)| s.iter().map(|(_, y)| *y))
            .fold(0.1_f64, f64::max)
            * 1.05
    } else {
        ymax
    };

    if let Some(t) = super::caffeine::chart(
        ui,
        "regional-chart",
        xmax,
        ymax,
        c.time_min,
        &lines,
        if early { "min" } else { "h" },
    ) {
        c.playing = false;
        c.set_time(t);
    }
}
