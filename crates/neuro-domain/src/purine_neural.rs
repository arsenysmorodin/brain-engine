//! Explicit uncalibrated concentration-to-network experiment, distinct from receptor PET.
use crate::{BrainAtlas, Error, NetworkScenario, require};
use serde::{Deserialize, Serialize};
pub const PURINE_NEURAL_MODEL_ID: &str = "purine_illustrative_drive_v1";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurineNeuralHypothesis {
    pub model_id: String,
    pub model_revision: u32,
    pub enabled: bool,
    /// Signed dimensionless input gains for caffeine, methylliberine, theacrine.
    pub drive_gains: [f64; 3],
    /// Illustrative concentration scales, NOT receptor Kd/Ki/IC50 or measured EC50.
    pub concentration_scales_mg_l: [f64; 3],
    pub network: NetworkScenario,
}
impl PurineNeuralHypothesis {
    pub fn reference(atlas: &BrainAtlas) -> Result<Self, Error> {
        Ok(Self {
            model_id: PURINE_NEURAL_MODEL_ID.into(),
            model_revision: 1,
            enabled: false,
            drive_gains: [0.35; 3],
            concentration_scales_mg_l: [1.0; 3],
            network: NetworkScenario::bundled(atlas)?,
        })
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.model_id == PURINE_NEURAL_MODEL_ID && self.model_revision == 1,
            "Неизвестная версия учебной нейронной гипотезы",
        )?;
        for (g, s) in self.drive_gains.iter().zip(self.concentration_scales_mg_l) {
            require(
                g.is_finite()
                    && (-5.0..=5.0).contains(g)
                    && s.is_finite()
                    && (0.01..=100.0).contains(&s),
                "Гипотеза E/I: g от −5 до 5, масштаб Ct от 0.01 до 100 мг/л",
            )?;
        }
        self.network.validate(atlas)
    }
}
