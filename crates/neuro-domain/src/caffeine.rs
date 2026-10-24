//! Human-reference caffeine PK, empirical PET estimates and an optional neural hypothesis.
use crate::{
    BrainAtlas, BrainRegion, BrainScene, Error, FORMAT_VERSION, NetworkScenario, TissueKind,
    atomic_write, on_time_grid, read_small_file, require,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CAFFEINE_MODEL_ID: &str = "caffeine_oral_pk_pet_v1";
pub const CAFFEINE_MODEL_REVISION: u32 = 1;
pub const CAFFEINE_PROVENANCE_ID: &str = "caffeine_sources_2026_10_24_v1";
pub const CAFFEINE_STEP_MIN: f64 = 0.05;
pub const CAFFEINE_SAMPLE_MIN: f64 = 0.5;
pub const A1_PET_C50_MG_L: f64 = 12.5;
pub const A2A_PD_PET_C50_MG_L: f64 = 2.2;
pub const A2A_PD_PET_MAX: f64 = 0.811;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineDose {
    pub time_min: f64,
    pub amount_mg: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineParameters {
    pub body_mass_kg: f64,
    pub distribution_l_kg: f64,
    pub bioavailability: f64,
    pub absorption_half_min: f64,
    pub elimination_half_h: f64,
    /// An assumed, non-mass-bearing effect site; not measured brain tissue PK.
    pub brain_equilibration_half_min: f64,
}
impl Default for CaffeineParameters {
    fn default() -> Self {
        Self {
            body_mass_kg: 70.0,
            distribution_l_kg: 0.7,
            bioavailability: 1.0,
            absorption_half_min: 7.5,
            elimination_half_h: 5.0,
            brain_equilibration_half_min: 5.0,
        }
    }
}
impl CaffeineParameters {
    pub fn volume_l(&self) -> f64 {
        self.body_mass_kg * self.distribution_l_kg
    }
    pub fn ka(&self) -> f64 {
        std::f64::consts::LN_2 / self.absorption_half_min
    }
    pub fn ke(&self) -> f64 {
        std::f64::consts::LN_2 / (60.0 * self.elimination_half_h)
    }
    pub fn kb(&self) -> f64 {
        std::f64::consts::LN_2 / self.brain_equilibration_half_min
    }
    pub fn clearance_l_h(&self) -> f64 {
        self.ke() * self.volume_l() * 60.0
    }
    pub fn validate(&self) -> Result<(), Error> {
        for (name, value, min, max) in [
            ("Масса, кг", self.body_mass_kg, 30.0, 200.0),
            ("Vd, л/кг", self.distribution_l_kg, 0.4, 1.0),
            ("Биодоступность F", self.bioavailability, 0.0, 1.0),
            (
                "Полувремя всасывания, мин",
                self.absorption_half_min,
                0.5,
                60.0,
            ),
            ("Полувремя выведения, ч", self.elimination_half_h, 1.0, 20.0),
            (
                "Полувремя уравновешивания Ce, мин",
                self.brain_equilibration_half_min,
                0.5,
                60.0,
            ),
        ] {
            require(
                value.is_finite() && (min..=max).contains(&value),
                format!("{name}: допустимо {min}–{max}"),
            )?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineNeuralHypothesis {
    pub enabled: bool,
    pub a1_drive_gain: f64,
    pub a2a_striatal_drive_gain: f64,
    pub network: NetworkScenario,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineScenario {
    pub format_version: u32,
    pub document_kind: String,
    pub model_id: String,
    pub model_revision: u32,
    pub provenance_id: String,
    pub name: String,
    pub parameters: CaffeineParameters,
    pub doses: Vec<CaffeineDose>,
    pub duration_min: f64,
    pub neural_hypothesis: CaffeineNeuralHypothesis,
}
impl CaffeineScenario {
    pub fn reference(atlas: &BrainAtlas) -> Result<Self, Error> {
        Ok(Self {
            format_version: FORMAT_VERSION,
            document_kind: "caffeine_scenario".into(),
            model_id: CAFFEINE_MODEL_ID.into(),
            model_revision: CAFFEINE_MODEL_REVISION,
            provenance_id: CAFFEINE_PROVENANCE_ID.into(),
            name: "Кофеин: пероральный приём, исследовательская модель".into(),
            parameters: CaffeineParameters::default(),
            doses: vec![CaffeineDose {
                time_min: 0.0,
                amount_mg: 100.0,
            }],
            duration_min: 1440.0,
            neural_hypothesis: CaffeineNeuralHypothesis {
                enabled: false,
                a1_drive_gain: 2.0,
                a2a_striatal_drive_gain: 0.0,
                network: NetworkScenario::bundled(atlas)?,
            },
        })
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION
                && self.document_kind == "caffeine_scenario"
                && self.model_id == CAFFEINE_MODEL_ID
                && self.model_revision == CAFFEINE_MODEL_REVISION
                && self.provenance_id == CAFFEINE_PROVENANCE_ID,
            "Неизвестная версия модели кофеина или её источников",
        )?;
        require(
            !self.name.trim().is_empty() && self.name.len() <= 512,
            "Название: 1–512 байт",
        )?;
        self.parameters.validate()?;
        require(
            (60.0..=2880.0).contains(&self.duration_min)
                && on_time_grid(self.duration_min, CAFFEINE_SAMPLE_MIN),
            "Горизонт: 60–2880 мин, кратно 0.5 мин",
        )?;
        require(self.doses.len() <= 32, "Не больше 32 приёмов")?;
        for dose in &self.doses {
            require(
                dose.time_min >= 0.0
                    && dose.time_min <= self.duration_min
                    && on_time_grid(dose.time_min, CAFFEINE_SAMPLE_MIN)
                    && dose.amount_mg.is_finite()
                    && (0.0..=500.0).contains(&dose.amount_mg),
                "Приём: 0–500 мг, время 0…горизонт с шагом 0.5 мин",
            )?;
        }
        require(
            self.doses.iter().map(|d| d.amount_mg).sum::<f64>() <= 2000.0,
            "Техническая область модели: суммарно до 2000 мг; это не граница безопасности",
        )?;
        let h = &self.neural_hypothesis;
        require(
            [h.a1_drive_gain, h.a2a_striatal_drive_gain]
                .iter()
                .all(|x| x.is_finite() && (-5.0..=5.0).contains(x)),
            "Коэффициенты гипотезы E/I: −5…5",
        )?;
        h.network.validate(atlas)?;
        require(
            h.network.stimuli.len() + h.network.populations.len() <= 128,
            "В сети нет места для входов гипотезы кофеина (лимит 128 стимулов)",
        )
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        let scenario =
            if value.get("document_kind").and_then(|v| v.as_str()) == Some("caffeine_project") {
                CaffeineProject::from_json(text, atlas)?.scenario
            } else {
                serde_json::from_value(value)?
            };
        scenario.validate(atlas)?;
        Ok(scenario)
    }
    pub fn load(path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::from_json(&read_small_file(path.as_ref())?, atlas)
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        self.validate(atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineProject {
    pub format_version: u32,
    pub document_kind: String,
    pub scene: BrainScene,
    pub scenario: CaffeineScenario,
}
impl CaffeineProject {
    pub fn new(scene: BrainScene, scenario: CaffeineScenario) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            document_kind: "caffeine_project".into(),
            scene,
            scenario,
        }
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION && self.document_kind == "caffeine_project",
            "Неизвестная версия проекта кофеина",
        )?;
        self.scene.validate(atlas)?;
        self.scenario.validate(atlas)
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let project: Self = serde_json::from_str(text)?;
        project.validate(atlas)?;
        Ok(project)
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        self.validate(atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}
/// This is a display/transfer scope, not a measured regional receptor-density map.
pub fn caffeine_a1_scope(r: &BrainRegion) -> bool {
    matches!(r.kind, TissueKind::Cortex | TissueKind::GrayMatter)
}
/// Only the atlas's caudate and putamen correspond to the striatal PET scope.
pub fn caffeine_a2a_scope(r: &BrainRegion) -> bool {
    ["FMA72827", "FMA72826", "FMA72829", "FMA72828"].contains(&r.id.as_str())
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineSource {
    pub id: String,
    pub title: String,
    pub url: String,
    pub doi: String,
    pub context: String,
    pub verification: String,
    pub rights: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineNode {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub kind: String,
    pub source_ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineLink {
    pub id: String,
    pub from: String,
    pub to: String,
    pub title: String,
    pub explanation: String,
    pub kind: String,
    pub source_ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeineKnowledge {
    pub id: String,
    pub model_id: String,
    pub model_revision: u32,
    pub species: String,
    pub rights: String,
    pub sources: Vec<CaffeineSource>,
    pub nodes: Vec<CaffeineNode>,
    pub links: Vec<CaffeineLink>,
}
impl CaffeineKnowledge {
    pub fn bundled() -> Result<Self, Error> {
        let k: Self = serde_json::from_str(include_str!("../../../data/caffeine-model.json"))?;
        require(
            k.id == CAFFEINE_PROVENANCE_ID
                && k.model_id == CAFFEINE_MODEL_ID
                && k.model_revision == CAFFEINE_MODEL_REVISION,
            "Версии метаданных кофеина не совпадают",
        )?;
        let mut ids = std::collections::HashSet::new();
        for s in &k.sources {
            require(
                ids.insert(&s.id)
                    && s.url.starts_with("https://")
                    && !s.verification.is_empty()
                    && !s.context.is_empty()
                    && !s.rights.is_empty(),
                "Некорректный источник кофеина",
            )?;
        }
        let mut nodes = std::collections::HashSet::new();
        for n in &k.nodes {
            require(
                nodes.insert(&n.id) && n.source_ids.iter().all(|id| ids.contains(id)),
                "Некорректный узел карты кофеина",
            )?;
        }
        let mut links = std::collections::HashSet::new();
        for e in &k.links {
            require(
                links.insert(&e.id)
                    && nodes.contains(&e.from)
                    && nodes.contains(&e.to)
                    && e.source_ids.iter().all(|id| ids.contains(id)),
                "Некорректная связь карты кофеина",
            )?;
        }
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caffeine_documents_validate_units_limits_and_versions() {
        let atlas = BrainAtlas::bundled().unwrap();
        CaffeineKnowledge::bundled().unwrap();
        let mut s = CaffeineScenario::reference(&atlas).unwrap();
        let text =
            serde_json::to_string(&CaffeineProject::new(BrainScene::default(), s.clone())).unwrap();
        assert_eq!(CaffeineScenario::from_json(&text, &atlas).unwrap(), s);
        s.doses[0].time_min = 0.1;
        assert!(s.validate(&atlas).is_err());
        s.doses[0].time_min = 0.0;
        s.parameters.elimination_half_h = f64::NAN;
        assert!(s.validate(&atlas).is_err());
        s.parameters.elimination_half_h = 5.0;
        s.model_revision = 2;
        assert!(s.validate(&atlas).is_err());
        assert_eq!(
            atlas
                .regions
                .iter()
                .filter(|r| caffeine_a2a_scope(r))
                .count(),
            4
        );
        assert!(!caffeine_a1_scope(
            atlas
                .regions
                .iter()
                .find(|r| r.kind == TissueKind::Cavity)
                .unwrap()
        ));
    }
}
