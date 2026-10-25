//! Closed, mass-bearing three-compound educational circulation. Not a clinical PBPK fit.
use crate::{
    BrainAtlas, BrainScene, CaffeinePerfusion, Error, atomic_write, read_small_file, require,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
pub const CIRCULATION_MODEL_REVISION: u32 = 4;
pub const CIRCULATION_MODEL_ID: &str = "purine_closed_circulation_v1";
pub const CIRCULATION_PROVENANCE_ID: &str = "purine_sources_2026_10_24_v1";
fn one_stage() -> u8 {
    1
}
fn is_one_stage(v: &u8) -> bool {
    *v == 1
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purine {
    Caffeine,
    Methylliberine,
    Theacrine,
}
impl Purine {
    pub const ALL: [Self; 3] = [Self::Caffeine, Self::Methylliberine, Self::Theacrine];
    pub fn index(self) -> usize {
        match self {
            Self::Caffeine => 0,
            Self::Methylliberine => 1,
            Self::Theacrine => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Caffeine => "Кофеин",
            Self::Methylliberine => "Метиллиберин",
            Self::Theacrine => "Теакрин",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Caffeine => "caffeine",
            Self::Methylliberine => "methylliberine",
            Self::Theacrine => "theacrine",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurineParameters {
    /// Apparent oral volume and clearance; F=1 is a scaling convention, not measured absolute F.
    pub apparent_volume_l: f64,
    pub apparent_clearance_l_h: f64,
    pub absorption_half_min: f64,
    #[serde(default = "one_stage", skip_serializing_if = "is_one_stage")]
    pub absorption_stages: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub peripheral_volume_l: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub distribution_clearance_l_h: f64,
    /// Assumed brain distribution and permeability. No human brain measurements for M/T.
    pub tissue_blood_partition: f64,
    pub exchange_min_inv: f64,
}
impl PurineParameters {
    pub fn reference(p: Purine) -> Self {
        Self::reference_revision(p, CIRCULATION_MODEL_REVISION)
    }
    pub fn reference_revision(p: Purine, revision: u32) -> Self {
        let (v, cl, abs) = match p {
            Purine::Caffeine if revision >= 3 => (33.5, 4.1, 7.5),
            Purine::Caffeine => (49.0, 49.0 * std::f64::consts::LN_2 / 5.0, 7.5),
            Purine::Methylliberine => (303.0, 149.0, 10.0),
            Purine::Theacrine => (51.0, 1.6, 10.0),
        };
        let peak = match p {
            Purine::Caffeine => 48.0,
            Purine::Methylliberine => 46.2,
            Purine::Theacrine => 108.0,
        };
        Self {
            apparent_volume_l: v,
            apparent_clearance_l_h: cl,
            absorption_half_min: if revision >= 3 {
                crate::pk_reference::absorption_half_at_peak(v, cl, peak)
            } else {
                abs
            },
            absorption_stages: 1,
            peripheral_volume_l: 0.0,
            distribution_clearance_l_h: 0.0,
            tissue_blood_partition: 1.0,
            exchange_min_inv: 0.5,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurineDose {
    pub substance: Purine,
    pub time_min: f64,
    pub amount_mg: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CirculationScenario {
    pub format_version: u32,
    pub document_kind: String,
    pub model_id: String,
    pub model_revision: u32,
    pub provenance_id: String,
    pub name: String,
    pub parameters: [PurineParameters; 3],
    pub doses: Vec<PurineDose>,
    pub duration_min: f64,
    /// Phenomenological interpolation of oral study ratios, not enzyme kinetics.
    pub interactions: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calibration_curve_ids: Vec<String>,
    pub beds: CaffeinePerfusion,
    pub capillary_volume_fraction: f64,
    pub venous_volume_fraction: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_hypothesis: Option<crate::PurineNeuralHypothesis>,
}
impl CirculationScenario {
    pub fn reference(atlas: &BrainAtlas, mask: u8) -> Self {
        let mut s = Self {
            format_version: 1,
            document_kind: "circulation_scenario".into(),
            model_id: CIRCULATION_MODEL_ID.into(),
            model_revision: CIRCULATION_MODEL_REVISION,
            provenance_id: CIRCULATION_PROVENANCE_ID.into(),
            name: "Пурины: замкнутый кровеносный контур".into(),
            parameters: Purine::ALL.map(PurineParameters::reference),
            doses: vec![],
            duration_min: 1440.0,
            interactions: true,
            calibration_curve_ids: vec![],
            beds: CaffeinePerfusion::reference(atlas),
            capillary_volume_fraction: 0.03,
            venous_volume_fraction: 0.04,
            neural_hypothesis: None,
        };
        for p in Purine::ALL {
            if mask & (1 << p.index()) != 0 {
                s.doses.push(PurineDose {
                    substance: p,
                    time_min: 0.0,
                    amount_mg: match p {
                        Purine::Caffeine => 150.0,
                        Purine::Methylliberine => 100.0,
                        Purine::Theacrine => {
                            if mask == 7 {
                                50.0
                            } else {
                                125.0
                            }
                        }
                    },
                });
            }
        }
        s
    }
    /// Revision 1 retains t=0-only semantics; revision 2 accepts a shared oral dose time.
    pub fn interaction_profile_active(&self) -> bool {
        let positive: Vec<_> = self.doses.iter().filter(|d| d.amount_mg > 0.0).collect();
        self.calibration_curve_ids.is_empty()
            && self.interactions
            && Purine::ALL
                .iter()
                .all(|p| positive.iter().filter(|d| d.substance == *p).count() <= 1)
            && if self.model_revision == 1 {
                positive.iter().all(|d| d.time_min == 0.0)
            } else {
                positive
                    .first()
                    .is_some_and(|first| positive.iter().all(|d| d.time_min == first.time_min))
            }
    }
    pub fn effective_parameters(&self) -> [PurineParameters; 3] {
        let mut p = self.parameters.clone();
        let present = Purine::ALL.map(|p| {
            self.doses
                .iter()
                .any(|d| d.substance == p && d.amount_mg > 0.0)
        });
        if self.interaction_profile_active() {
            if present[0] && present[1] {
                p[0].apparent_clearance_l_h *= 0.42;
                p[1].apparent_volume_l *= 0.81;
                p[1].apparent_clearance_l_h *= 0.81;
                if self.model_revision >= 3 {
                    let base = PurineParameters::reference(Purine::Methylliberine);
                    p[1].absorption_half_min *= crate::pk_reference::absorption_half_at_peak(
                        303.0 * 0.81,
                        149.0 * 0.81,
                        57.6,
                    ) / base.absorption_half_min;
                }
            }
            if present[0] && present[2] {
                p[2].apparent_volume_l *= 35.4 / 51.0;
                p[2].apparent_clearance_l_h *= 1.2 / 1.6;
                if self.model_revision >= 3 {
                    let base = PurineParameters::reference(Purine::Theacrine);
                    p[2].absorption_half_min *=
                        crate::pk_reference::absorption_half_at_peak(35.4, 1.2, 60.0)
                            / base.absorption_half_min;
                }
            }
        }
        p
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == 1
                && self.document_kind == "circulation_scenario"
                && self.model_id == CIRCULATION_MODEL_ID
                && (1..=CIRCULATION_MODEL_REVISION).contains(&self.model_revision)
                && self.provenance_id == CIRCULATION_PROVENANCE_ID,
            "Неизвестная версия замкнутой циркуляции",
        )?;
        require(
            !self.name.trim().is_empty() && self.name.len() <= 512,
            "Название: 1…512 байт",
        )?;
        self.beds.validate(atlas)?;
        require(
            self.calibration_curve_ids.len() <= 3
                && self
                    .calibration_curve_ids
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == self.calibration_curve_ids.len()
                && self.calibration_curve_ids.iter().all(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                })
                && (self.calibration_curve_ids.is_empty()
                    || (self.model_revision >= 4 && !self.interactions)),
            "Подобранный протокол не должен повторно применять парные поправки",
        )?;
        if let Some(h) = &self.neural_hypothesis {
            h.validate(atlas)?;
        }
        require(
            self.duration_min.is_finite()
                && (5.0..=2880.0).contains(&self.duration_min)
                && crate::on_time_grid(self.duration_min, 0.5),
            "Длительность: 5…2880 мин, шаг 0.5",
        )?;
        for f in [self.capillary_volume_fraction, self.venous_volume_fraction] {
            require(
                f.is_finite() && (0.01..=0.1).contains(&f),
                "Доля сосудистого объёма: 0.01…0.1",
            )?;
        }
        for p in &self.parameters {
            require(
                (1..=4).contains(&p.absorption_stages)
                    && p.peripheral_volume_l.is_finite()
                    && p.distribution_clearance_l_h.is_finite()
                    && ((p.peripheral_volume_l == 0.0 && p.distribution_clearance_l_h == 0.0)
                        || ((1.0..=5000.0).contains(&p.peripheral_volume_l)
                            && (0.01..=5000.0).contains(&p.distribution_clearance_l_h)))
                    && (self.model_revision >= 4
                        || (p.absorption_stages == 1
                            && p.peripheral_volume_l == 0.0
                            && p.distribution_clearance_l_h == 0.0)),
                "Транзит/периферическое распределение требуют ревизии 4 и допустимых парных параметров",
            )?;
            for (v, min, max) in [
                (p.apparent_volume_l, 10.0, 1000.0),
                (p.apparent_clearance_l_h, 0.1, 500.0),
                (p.absorption_half_min, 0.5, 60.0),
                (p.tissue_blood_partition, 0.1, 3.0),
                (p.exchange_min_inv, 0.0, 5.0),
            ] {
                require(
                    v.is_finite() && (min..=max).contains(&v),
                    "Параметры PK/обмена за техническими пределами",
                )?;
            }
        }
        let capacity = self.beds.regions.iter().map(|r| r.volume_l()).sum::<f64>();
        for p in self.effective_parameters() {
            require(
                p.apparent_volume_l
                    - capacity
                        * (p.tissue_blood_partition
                            + self.capillary_volume_fraction
                            + self.venous_volume_fraction)
                    > 1.0,
                "Остаточный кажущийся объём должен быть больше 1 л",
            )?;
        }
        require(self.doses.len() <= 32, "Не больше 32 приёмов")?;
        for d in &self.doses {
            require(
                d.time_min.is_finite()
                    && (0.0..=self.duration_min).contains(&d.time_min)
                    && crate::on_time_grid(d.time_min, 0.5)
                    && d.amount_mg.is_finite()
                    && (0.0..=500.0).contains(&d.amount_mg),
                "Приём: 0…500 мг, время на сетке 0.5 мин",
            )?;
        }
        require(
            self.doses.iter().map(|d| d.amount_mg).sum::<f64>() <= 2000.0,
            "Сумма доз не больше 2000 мг — технический предел, не рекомендация",
        )
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        if value.get("document_kind").and_then(|v| v.as_str()) == Some("circulation_project") {
            return Ok(CirculationProject::from_json(text, atlas)?.scenario);
        }
        let s: Self = serde_json::from_value(value)?;
        s.validate(atlas)?;
        Ok(s)
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
pub struct CirculationProject {
    pub format_version: u32,
    pub document_kind: String,
    pub scene: BrainScene,
    pub scenario: CirculationScenario,
}
impl CirculationProject {
    pub fn new(scene: BrainScene, scenario: CirculationScenario) -> Self {
        Self {
            format_version: 1,
            document_kind: "circulation_project".into(),
            scene,
            scenario,
        }
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let p: Self = serde_json::from_str(text)?;
        require(
            p.format_version == 1 && p.document_kind == "circulation_project",
            "Неизвестный проект циркуляции",
        )?;
        p.scene.validate(atlas)?;
        p.scenario.validate(atlas)?;
        Ok(p)
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        Self::from_json(&serde_json::to_string(self)?, atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CirculationKnowledge {
    pub id: String,
    pub checked_on: String,
    pub limitations: String,
    pub sources: Vec<crate::CaffeineSource>,
}
impl CirculationKnowledge {
    pub fn bundled() -> Result<Self, Error> {
        let k: Self = serde_json::from_str(include_str!("../../../data/purine-model.json"))?;
        require(
            k.id == CIRCULATION_PROVENANCE_ID
                && !k.checked_on.is_empty()
                && !k.limitations.is_empty()
                && k.sources.len() >= 4,
            "Неполное происхождение модели пуринов",
        )?;
        let mut ids = std::collections::HashSet::new();
        for s in &k.sources {
            require(
                ids.insert(&s.id)
                    && s.url.starts_with("https://")
                    && [&s.title, &s.doi, &s.context, &s.verification, &s.rights]
                        .iter()
                        .all(|v| !v.is_empty()),
                "Неполный источник пуринов",
            )?;
        }
        Ok(k)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_profiles_and_unknown_values() {
        let a = BrainAtlas::bundled().unwrap();
        CirculationKnowledge::bundled().unwrap();
        for mask in 0..8 {
            let s = CirculationScenario::reference(&a, mask);
            s.validate(&a).unwrap();
            let project = CirculationProject::new(BrainScene::default(), s.clone());
            assert_eq!(
                CirculationScenario::from_json(&serde_json::to_string(&project).unwrap(), &a)
                    .unwrap(),
                s
            );
        }
        let mut s = CirculationScenario::reference(&a, 7);
        assert!(
            s.effective_parameters()[0].apparent_clearance_l_h
                < s.parameters[0].apparent_clearance_l_h
        );
        s.doses[1].time_min = 10.0;
        assert!(!s.interaction_profile_active());
        assert_eq!(s.effective_parameters(), s.parameters);
        s.parameters[2].exchange_min_inv = f64::NAN;
        assert!(s.validate(&a).is_err());
        s = CirculationScenario::reference(&a, 7);
        s.model_revision = 99;
        assert!(s.validate(&a).is_err());
    }
}
