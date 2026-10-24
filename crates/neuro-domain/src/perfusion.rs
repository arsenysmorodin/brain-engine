//! Versioned, explicitly assumed regional perfusion beds; anatomical meshes are separate.
use crate::{BrainAtlas, BrainGroup, Error, Hemisphere, TissueKind, require};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PERFUSION_MODEL_ID: &str = "caffeine_regional_perfusion_v1";
pub const PERFUSION_PROVENANCE_ID: &str = "perfusion_sources_2026_10_24_v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArterialTerritory {
    AcaLeft,
    AcaRight,
    McaLeft,
    McaRight,
    PcaLeft,
    PcaRight,
    Vertebrobasilar,
    Mixed,
}
impl ArterialTerritory {
    pub const ALL: [Self; 8] = [
        Self::AcaLeft,
        Self::AcaRight,
        Self::McaLeft,
        Self::McaRight,
        Self::PcaLeft,
        Self::PcaRight,
        Self::Vertebrobasilar,
        Self::Mixed,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::AcaLeft => "ACA · слева",
            Self::AcaRight => "ACA · справа",
            Self::McaLeft => "MCA · слева",
            Self::McaRight => "MCA · справа",
            Self::PcaLeft => "PCA · слева",
            Self::PcaRight => "PCA · справа",
            Self::Vertebrobasilar => "Вертебробазилярный",
            Self::Mixed => "Смешанное снабжение",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::AcaLeft => "aca_left",
            Self::AcaRight => "aca_right",
            Self::McaLeft => "mca_left",
            Self::McaRight => "mca_right",
            Self::PcaLeft => "pca_left",
            Self::PcaRight => "pca_right",
            Self::Vertebrobasilar => "vertebrobasilar",
            Self::Mixed => "mixed",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionalPerfusion {
    pub region_id: String,
    /// Dominant supply label for a coarse whole structure, not a voxel territory atlas.
    pub territory: ArterialTerritory,
    pub tissue_mass_g: f64,
    pub baseline_cbf_ml_100g_min: f64,
    pub flow_fraction: f64,
    pub partition_tissue_blood: f64,
    pub transit_sec: f64,
}
impl RegionalPerfusion {
    pub fn volume_l(&self) -> f64 {
        self.tissue_mass_g / 1000.0
    }
    pub fn baseline_flow_ml_min(&self) -> f64 {
        self.baseline_cbf_ml_100g_min * self.tissue_mass_g / 100.0
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaffeinePerfusion {
    pub model_id: String,
    pub model_revision: u32,
    pub provenance_id: String,
    pub vasoconstriction_enabled: bool,
    /// Uncalibrated concentration/flow hypothesis, NOT a PET binding parameter.
    pub max_flow_reduction: f64,
    pub flow_half_effect_mg_l: f64,
    pub regions: Vec<RegionalPerfusion>,
}
impl CaffeinePerfusion {
    pub fn reference(atlas: &BrainAtlas) -> Self {
        let count = atlas
            .regions
            .iter()
            .filter(|r| r.kind != TissueKind::Cavity)
            .count();
        Self {
            model_id: PERFUSION_MODEL_ID.into(),
            model_revision: 1,
            provenance_id: PERFUSION_PROVENANCE_ID.into(),
            vasoconstriction_enabled: true,
            max_flow_reduction: 0.4,
            flow_half_effect_mg_l: 3.0,
            regions: atlas
                .regions
                .iter()
                .filter(|r| r.kind != TissueKind::Cavity)
                .map(|r| {
                    let left = r.hemisphere == Hemisphere::Left;
                    let territory =
                        if matches!(r.kind, TissueKind::Cerebellum | TissueKind::Brainstem) {
                            ArterialTerritory::Vertebrobasilar
                        } else if r.hemisphere == Hemisphere::Bilateral {
                            ArterialTerritory::Mixed
                        } else if r.group == BrainGroup::Occipital
                            || ["hippocampus", "thalamus", "parahippocampal"]
                                .iter()
                                .any(|s| r.name_en.contains(s))
                        {
                            if left {
                                ArterialTerritory::PcaLeft
                            } else {
                                ArterialTerritory::PcaRight
                            }
                        } else if r.name_en.contains("superior frontal")
                            || r.name_en.contains("cingulate")
                        {
                            if left {
                                ArterialTerritory::AcaLeft
                            } else {
                                ArterialTerritory::AcaRight
                            }
                        } else if left {
                            ArterialTerritory::McaLeft
                        } else {
                            ArterialTerritory::McaRight
                        };
                    RegionalPerfusion {
                        region_id: r.id.clone(),
                        territory,
                        tissue_mass_g: 1400.0 / count as f64,
                        baseline_cbf_ml_100g_min: if r.kind == TissueKind::WhiteMatter {
                            20.0
                        } else {
                            50.0
                        },
                        flow_fraction: 1.0,
                        partition_tissue_blood: 1.0,
                        transit_sec: 2.0,
                    }
                })
                .collect(),
        }
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.model_id == PERFUSION_MODEL_ID
                && self.model_revision == 1
                && self.provenance_id == PERFUSION_PROVENANCE_ID,
            "Неизвестная версия региональной перфузии",
        )?;
        require(
            self.max_flow_reduction.is_finite()
                && (0.0..=0.8).contains(&self.max_flow_reduction)
                && self.flow_half_effect_mg_l.is_finite()
                && (0.1..=20.0).contains(&self.flow_half_effect_mg_l),
            "Сосудистая гипотеза: снижение 0…0.8, C50 0.1…20 мг/л",
        )?;
        let expected: HashSet<_> = atlas
            .regions
            .iter()
            .filter(|r| r.kind != TissueKind::Cavity)
            .map(|r| r.id.as_str())
            .collect();
        let actual: HashSet<_> = self.regions.iter().map(|r| r.region_id.as_str()).collect();
        require(
            expected == actual && actual.len() == self.regions.len(),
            "Перфузия требует каждую неполостную область ровно один раз",
        )?;
        for r in &self.regions {
            for (v, min, max) in [
                (r.tissue_mass_g, 0.1, 500.0),
                (r.baseline_cbf_ml_100g_min, 1.0, 150.0),
                (r.flow_fraction, 0.0, 2.0),
                (r.partition_tissue_blood, 0.5, 3.0),
                (r.transit_sec, 0.0, 30.0),
            ] {
                require(
                    v.is_finite() && (min..=max).contains(&v),
                    format!("Недопустимый параметр перфузии {}", r.region_id),
                )?;
            }
        }
        require(
            self.regions.iter().map(|r| r.tissue_mass_g).sum::<f64>() <= 2500.0,
            "Масса моделируемой ткани не больше 2500 г (технический предел)",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn perfusion_requires_complete_unique_finite_beds_and_versions() {
        PerfusionKnowledge::bundled().unwrap();
        let a = BrainAtlas::bundled().unwrap();
        let mut p = CaffeinePerfusion::reference(&a);
        p.validate(&a).unwrap();
        assert!((p.regions.iter().map(|r| r.tissue_mass_g).sum::<f64>() - 1400.0).abs() < 1e-9);
        p.regions[0].flow_fraction = 0.0;
        p.validate(&a).unwrap();
        p.regions[0].transit_sec = f64::NAN;
        assert!(p.validate(&a).is_err());
        p = CaffeinePerfusion::reference(&a);
        p.regions[0] = p.regions[1].clone();
        assert!(p.validate(&a).is_err());
        p = CaffeinePerfusion::reference(&a);
        p.model_revision = 2;
        assert!(p.validate(&a).is_err());
        let legacy = crate::CaffeineScenario::from_json(
            include_str!("../../../scenarios/caffeine.json"),
            &a,
        )
        .unwrap();
        assert!(legacy.perfusion.is_none());
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfusionKnowledge {
    pub id: String,
    pub model_id: String,
    pub model_revision: u32,
    pub species: String,
    pub rights: String,
    pub sources: Vec<crate::CaffeineSource>,
}
impl PerfusionKnowledge {
    pub fn bundled() -> Result<Self, Error> {
        let k: Self = serde_json::from_str(include_str!("../../../data/perfusion-model.json"))?;
        require(
            k.id == PERFUSION_PROVENANCE_ID
                && k.model_id == PERFUSION_MODEL_ID
                && k.model_revision == 1
                && !k.species.is_empty()
                && !k.rights.is_empty(),
            "Неизвестное происхождение перфузии",
        )?;
        let mut ids = HashSet::new();
        for s in &k.sources {
            require(
                ids.insert(&s.id)
                    && s.url.starts_with("https://")
                    && [&s.title, &s.doi, &s.context, &s.verification, &s.rights]
                        .iter()
                        .all(|v| !v.is_empty()),
                "Неполный источник перфузии",
            )?;
        }
        require(!ids.is_empty(), "Нет источников перфузии")?;
        Ok(k)
    }
}
