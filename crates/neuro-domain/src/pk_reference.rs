//! Selected published plasma summary endpoints, not digitized time-series observations.
use crate::{CirculationScenario, Error, Purine, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurinePkReference {
    pub id: String,
    pub title: String,
    pub url: String,
    pub n: usize,
    pub substance: Purine,
    pub doses_mg: [f64; 3],
    pub reported_cmax_ng_ml: f64,
    pub cmax_dose_normalized: bool,
    pub reported_auc_h_ng_ml_per_mg: f64,
    pub tmax_min: f64,
    pub tmax_statistic: String,
    pub half_life_h: f64,
}
impl PurinePkReference {
    pub fn bundled() -> Result<Vec<Self>, Error> {
        let refs: Vec<Self> =
            serde_json::from_str(include_str!("../../../data/purine-pk-references.json"))?;
        let mut ids = std::collections::HashSet::new();
        for r in &refs {
            require(
                ids.insert(&r.id)
                    && r.n > 0
                    && r.url.starts_with("https://")
                    && !r.title.is_empty()
                    && !r.tmax_statistic.is_empty(),
                "Неполный PK-референс",
            )?;
            require(
                r.doses_mg.iter().all(|x| x.is_finite() && *x >= 0.0)
                    && r.doses_mg[r.substance.index()] > 0.0,
                "Недопустимые дозы PK-референса",
            )?;
            for x in [
                r.reported_cmax_ng_ml,
                r.reported_auc_h_ng_ml_per_mg,
                r.tmax_min,
                r.half_life_h,
            ] {
                require(x.is_finite() && x > 0.0, "Недопустимый PK-ориентир")?;
            }
        }
        Ok(refs)
    }
    pub fn cmax_mg_l(&self) -> f64 {
        self.reported_cmax_ng_ml / 1000.0
            * if self.cmax_dose_normalized {
                self.doses_mg[self.substance.index()]
            } else {
                1.0
            }
    }
    pub fn auc_inf_mg_h_l(&self) -> f64 {
        self.reported_auc_h_ng_ml_per_mg * self.doses_mg[self.substance.index()] / 1000.0
    }
    /// No scaling a measured endpoint to another dose, repeated protocol or triple mixture.
    pub fn matched_dose_time(&self, s: &CirculationScenario) -> Option<f64> {
        let positive: Vec<_> = s.doses.iter().filter(|d| d.amount_mg > 0.0).collect();
        let t = positive.first()?.time_min;
        for p in Purine::ALL {
            let doses: Vec<_> = positive.iter().filter(|d| d.substance == p).collect();
            let expected = self.doses_mg[p.index()];
            if expected == 0.0 {
                if !doses.is_empty() {
                    return None;
                }
            } else if doses.len() != 1
                || (doses[0].amount_mg - expected).abs() > 1e-9
                || doses[0].time_min != t
            {
                return None;
            }
        }
        Some(t)
    }
}

/// Bateman Tmax for ka > ke; the reference targets lie below 1/ke.
/// This identifies an absorption assumption, not a fit to individual concentration samples.
pub(crate) fn absorption_half_at_peak(volume: f64, clearance: f64, tmax_min: f64) -> f64 {
    let ke = clearance / volume / 60.0;
    let mut lo = ke;
    let mut hi = 1000.0;
    for _ in 0..100 {
        let ka = (lo + hi) * 0.5;
        let peak = ((ka - ke) / ke).ln_1p() / (ka - ke);
        if peak > tmax_min {
            lo = ka;
        } else {
            hi = ka;
        }
    }
    std::f64::consts::LN_2 / ((lo + hi) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_units_and_protocol_scope_are_explicit() {
        let refs = PurinePkReference::bundled().unwrap();
        assert!((refs[0].cmax_mg_l() - 5.01).abs() < 1e-12);
        assert!((refs[1].cmax_mg_l() - 0.254).abs() < 1e-12);
        assert!((refs[3].cmax_mg_l() - 3.2).abs() < 1e-12);
        assert!((refs[3].auc_inf_mg_h_l() - 92.0).abs() < 1e-12);
        let a = crate::BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 4);
        s.doses[0].time_min = 30.0;
        assert_eq!(refs[3].matched_dose_time(&s), Some(30.0));
        s.doses[0].amount_mg = 50.0;
        assert_eq!(refs[3].matched_dose_time(&s), None);
        assert!(refs.iter().all(|r| {
            r.matched_dose_time(&CirculationScenario::reference(&a, 7))
                .is_none()
        }));
    }
}
