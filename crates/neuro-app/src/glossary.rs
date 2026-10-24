//! Learning overlays use public egui paint/text APIs, including painter-drawn chart labels.
use crate::theme::*;
use bevy_egui::egui::{self, Pos2};
use neuro_domain::{BrainAtlas, Error, LearningCatalog, LearningTerm as Term, VascularAtlas};
pub struct Glossary {
    pub terms: Vec<Term>,
    pub enabled: bool,
    pub open: bool,
    pub search: String,
    pub selected: Option<usize>,
    hover: Option<(usize, f64)>,
    pub visible_tooltip: Option<usize>,
}
impl Glossary {
    pub fn new(atlas: &BrainAtlas, vessels: &VascularAtlas) -> Result<Self, Error> {
        let mut catalog = LearningCatalog::bundled()?;
        for r in &atlas.regions {
            catalog.terms.push(Term{term:r.name.clone(),aliases:vec![r.name.clone(),r.name_en.clone(),r.id.clone()],explanation:format!("{} — {}. Анатомическая группа: {}. Отдельная структура трёхмерного атласа; её геометрия не определяет плотность рецепторов, число нейронов или кровоток. Функциональные параметры задаются отдельно.",r.name,r.kind.label(),r.group.label()),context:format!("{}; {}. {}. Геометрия BodyParts3D, {}.",r.id,r.name_en,atlas.source.attribution,atlas.source.license)});
        }
        for v in &vessels.vessels {
            catalog.terms.push(Term{term:v.name.clone(),aliases:vec![v.name.clone(),v.name_en.clone(),v.id.clone()],explanation:format!("Артериальная структура: {}. {}. Название и геометрия из BodyParts3D; цвет сосуда в симуляции — концентрация модельного входа, не измерение скорости, давления или потока в этой отдельной ветви.",v.name,v.name_en),context:format!("{}; {}. Источник: {}.",v.id,vessels.source.attribution,vessels.source.license)});
        }
        Ok(Self {
            terms: catalog.terms,
            enabled: true,
            open: false,
            search: String::new(),
            selected: None,
            hover: None,
            visible_tooltip: None,
        })
    }
    pub fn term_at(&self, text: &str, byte: usize) -> Option<usize> {
        let lower = text.to_lowercase();
        let mut byte = byte.min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        let lower_byte = text[..byte].to_lowercase().len();
        let mut found = None;
        let mut longest = 0;
        for (i, term) in self.terms.iter().enumerate() {
            for alias in &term.aliases {
                let single = alias.len() == 1 && alias.is_ascii();
                let needle = if single {
                    alias.clone()
                } else {
                    alias.to_lowercase()
                };
                let hay = if single { text } else { &lower };
                let byte = if single { byte } else { lower_byte };
                for (start, m) in hay.match_indices(&needle) {
                    let end = start + m.len();
                    let left = hay[..start].chars().next_back();
                    let right = hay[end..].chars().next();
                    let first = needle.chars().next().unwrap();
                    let last = needle.chars().next_back().unwrap();
                    let boundary = (!first.is_alphanumeric()
                        || left.is_none_or(|c| !c.is_alphanumeric() && c != '_'))
                        && (!last.is_alphanumeric()
                            || right.is_none_or(|c| !c.is_alphanumeric() && c != '_'));
                    if boundary
                        && ((start..end).contains(&byte) || (byte == end && byte > start))
                        && m.len() > longest
                    {
                        found = Some(i);
                        longest = m.len();
                    }
                }
            }
        }
        found
    }
    fn hovered(&self, ctx: &egui::Context) -> Option<(usize, egui::LayerId, Pos2)> {
        if ctx.input(|i| i.pointer.any_down()) {
            return None;
        }
        let pos = ctx.pointer_hover_pos()?;
        let layer = ctx.layer_id_at(pos)?;
        if layer.order == egui::Order::Tooltip {
            return None;
        }
        let local = ctx
            .layer_transform_from_global(layer)
            .map_or(pos, |t| t * pos);
        let text = ctx.graphics(|g| {
            let list = g.get(layer)?;
            let mut found = None;
            for entry in list.all_entries() {
                if entry.clip_rect.contains(local)
                    && let Some(hit) = hit_text(&entry.shape, local)
                {
                    found = Some(hit);
                }
            }
            found
        })?;
        self.term_at(&text.0, text.1).map(|i| (i, layer, pos))
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        self.visible_tooltip = None;
        let mut open = self.open;
        if open {
            egui::Window::new("Учебник NeuroLab").open(&mut open).default_width(650.0).default_height(480.0).resizable(true).show(ctx,|ui|{
            ui.checkbox(&mut self.enabled,"Подсказки при наведении");muted(ui,"Наведите курсор на термин или обозначение в любой панели и на графиках. Здесь можно читать объяснения без наведения.");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Найти: CBF, ГЭБ, метиллиберин, AUC…").desired_width(f32::INFINITY));
            let needle=self.search.trim().to_lowercase();ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("learning-terms").max_height(200.0).show(ui,|ui|{let mut count=0;for (i,t) in self.terms.iter().enumerate(){if needle.is_empty()||t.aliases.iter().any(|a|a.to_lowercase().contains(&needle)){count+=1;if ui.selectable_label(self.selected==Some(i),&t.term).clicked(){self.selected=Some(i);}}}if count==0{muted(ui,"Термин не найден. Попробуйте другое написание или сокращение.");}});
            ui.separator();if let Some(t)=self.selected.and_then(|i|self.terms.get(i)){ui.heading(&t.term);ui.label(&t.explanation);ui.add_space(8.0);muted(ui,&t.context);}else{muted(ui,format!("{} понятий и анатомических названий. Выберите термин в списке.",self.terms.len()));}
        });
        }
        self.open = open;
        if !self.enabled {
            self.hover = None;
            return;
        }
        let Some((index, layer, pos)) = self.hovered(ctx) else {
            self.hover = None;
            return;
        };
        let now = ctx.input(|i| i.time);
        let since = match self.hover {
            Some((last, since)) if last == index => since,
            _ => {
                self.hover = Some((index, now));
                now
            }
        };
        if now - since < 0.45 {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
            return;
        }
        let term = &self.terms[index];
        egui::Tooltip::always_open(
            ctx.clone(),
            layer,
            egui::Id::new(("scientific-term", index)),
            pos,
        )
        .at_pointer()
        .width(390.0)
        .show(|ui| {
            ui.label(egui::RichText::new(&term.term).strong().color(TEAL));
            ui.label(&term.explanation);
            ui.add_space(5.0);
            muted(ui, &term.context);
        });
        self.visible_tooltip = Some(index);
    }
}
fn hit_text(shape: &egui::Shape, pos: Pos2) -> Option<(String, usize)> {
    match shape {
        egui::Shape::Text(t) => {
            let local = egui::emath::Rot2::from_angle(-t.angle) * (pos - t.pos);
            if !t.galley.rect.contains(local.to_pos2()) {
                return None;
            }
            let text = t.galley.text();
            let cursor = t.galley.cursor_from_pos(local).index;
            let byte = text
                .char_indices()
                .nth(cursor)
                .map_or(text.len(), |(i, _)| i);
            Some((text.to_owned(), byte))
        }
        egui::Shape::Vec(shapes) => shapes.iter().rev().find_map(|s| hit_text(s, pos)),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::Rect;
    fn glossary() -> Glossary {
        Glossary::new(
            &BrainAtlas::bundled().unwrap(),
            &VascularAtlas::bundled().unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn unicode_longest_match_boundaries_and_anatomical_ids() {
        let g = glossary();
        assert!(g.terms.len() >= 250);
        for (text, term) in [
            ("CBF = 50", "CBF"),
            ("Ct = 2 мг/л", "Ct"),
            ("V/F = 35", "V/F"),
            ("E/I — популяции", "E/I"),
            ("A2A · PET в PD", "PET в PD"),
            ("FMA72976", "Затылочная доля · слева"),
        ] {
            let pos = if text.starts_with("A2A") { 8 } else { 1 };
            let i = g.term_at(text, pos).unwrap();
            assert_eq!(g.terms[i].term, term, "{text}");
        }
        assert!(g.term_at("someCBFinside", 5).is_none());
        let text = "Концентрация: метиллиберин";
        let i = g
            .term_at(text, text.find("метиллиберин").unwrap() + 4)
            .unwrap();
        assert_eq!(g.terms[i].term, "Метиллиберин");
    }
    #[test]
    fn pointer_hover_covers_labels_and_painted_text_and_honors_clipping() {
        let ctx = egui::Context::default();
        crate::theme::configure(&ctx);
        let mut g = glossary();
        for (time, pos, clipped, expected) in [
            (0.0, Pos2::new(25.0, 108.0), false, false),
            (0.6, Pos2::new(25.0, 108.0), false, true),
            (1.0, Pos2::new(12.0, 14.0), false, false),
            (1.6, Pos2::new(12.0, 14.0), false, true),
            (2.2, Pos2::new(25.0, 208.0), true, false),
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
                    time: Some(time),
                    events: vec![egui::Event::PointerMoved(pos)],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Cp плазмы");
                        let painter = if clipped {
                            ui.painter_at(Rect::from_min_max(
                                Pos2::new(200.0, 200.0),
                                Pos2::new(300.0, 250.0),
                            ))
                        } else {
                            ui.painter().clone()
                        };
                        painter.text(
                            Pos2::new(20.0, if clipped { 200.0 } else { 100.0 }),
                            egui::Align2::LEFT_TOP,
                            "Ccap",
                            egui::FontId::proportional(16.0),
                            TEXT,
                        );
                    });
                    g.show(ctx);
                },
            );
            assert_eq!(g.visible_tooltip.is_some(), expected, "time {time}");
        }
    }
}
