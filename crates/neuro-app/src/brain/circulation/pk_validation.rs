use super::*;

pub fn show(ctx: &egui::Context, c: &mut CirculationLab, atlas: &BrainAtlas) {
    if !c.show_pk_reference {
        return;
    }
    let mut open = true;
    let mut reset = false;
    egui::Window::new("Сверка плазменной PK")
        .open(&mut open).default_width(640.0).default_height(490.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if ui.button("Открыть кривые исследований и подбор по всем точкам").clicked() { c.show_study_curves = true; }
                if !c.scenario.calibration_curve_ids.is_empty() { ui.colored_label(PURPLE,"Активен протокол оцифрованной кривой. Сводные показатели других исследований к нему не подставляются."); }
                ui.label("Сравниваются сводные показатели исследования и текущей модели. Это не подгонка временного ряда и не независимая валидация.");
                muted(ui, format!("Модель: ревизия {}. В новом референсе кофеин использует V/F и CL/F когорты He 2017; всасывание привязано к Tmax. Это референс конкретных малых исследований.", c.scenario.model_revision));
                if let Some(trace) = &c.trace {
                    match neuro_engine::compare_purine_pk(trace) {
                        Ok(rows) if rows.is_empty() => { ui.colored_label(PURPLE, "Для этого состава и доз нет сопоставимого встроенного референса.");
                            muted(ui, "Пики другого состава, разнесённых/повторных доз или другой дозировки не выдаются за измерения текущего протокола. Доступные оцифрованные временные ряды находятся в отдельном окне «Кривые исследований»."); }
                        Ok(rows) => { for (i, row) in rows.iter().enumerate() {
                            let r = &row.reference;
                            ui.separator();
                            ui.colored_label(DRUG_COLORS[r.substance.index()], r.substance.label());
                            ui.hyperlink_to(&r.title, &r.url);
                            muted(ui, format!("Плазма человека, n={}; C/M/T = {:.0}/{:.0}/{:.0} мг; Tmax: {}.", r.n, r.doses_mg[0], r.doses_mg[1], r.doses_mg[2], r.tmax_statistic));
                            egui::Grid::new(("pk-reference-table", i)).spacing([18.0, 6.0]).striped(true).show(ui, |ui| {
                                for text in ["Показатель", "Модель", "Исследование", "Разница"] { ui.strong(text); } ui.end_row();
                                for (label, model, observed, error) in [
                                    ("Cmax, мг/л", format!("{:.4}", row.model_cmax_mg_l), format!("{:.4}", r.cmax_mg_l()), if row.peak_captured { format!("{:+.1}%", row.cmax_error_pct) } else { "Пик не охвачен".into() }),
                                    ("Tmax после приёма, мин", format!("{:.1}", row.model_tmax_after_dose_min), format!("{:.1}", r.tmax_min), if row.peak_captured { format!("{:+.1} мин", row.tmax_error_min) } else { "Короткий протокол".into() }),
                                    ("AUC∞, мг·ч/л", format!("{:.3}", row.model_auc_inf_mg_h_l), format!("{:.3}", r.auc_inf_mg_h_l()), format!("{:+.1}%", row.auc_inf_error_pct)),
                                    ("t½ CL/V / терминальное, ч", format!("{:.2}", row.model_distribution_half_h), format!("{:.2}", r.half_life_h), "Разные оценки".into()),
                                ] { ui.label(label); ui.label(model); ui.label(observed); ui.label(error); ui.end_row(); }
                            });
                            muted(ui, "AUC∞ модели = доза / CL/F, это полный будущий интеграл, а не AUC до текущего кадра. t½ CL/V не заменяет терминальную оценку статьи. Средние/медианы разных показателей не описывают одного участника.");
                            if r.cmax_dose_normalized { muted(ui, "Cmax в статье нормирован на дозу (сноска/подпись рисунка), несмотря на заголовок ng/mL; здесь умножен на дозу и переведён в мг/л. AUC также денормирована."); }
                        }}
                        Err(e) => { ui.colored_label(ERROR, e.to_string()); }
                    }
                } else { muted(ui, "Сначала пересчитайте протокол."); }
                ui.separator();
                muted(ui, "Референсы: C150; M100; C150+M100 (M); T125; C150+T125 (T). Другие дозы и смесь остаются экстраполяцией. Новая ревизия не меняет сохранённые проекты автоматически.");
                if ui.button("Восстановить референсные V/F, CL/F и всасывание · ревизия 4").clicked() { reset = true; }
                muted(ui, "Кнопка заменит системные PK-параметры всех трёх веществ. Дозы, тканевый обмен и кровоток сохранятся; потребуется пересчёт.");
            });
        });
    c.show_pk_reference = open;
    if reset {
        c.restore_pk_reference(atlas);
    }
}
