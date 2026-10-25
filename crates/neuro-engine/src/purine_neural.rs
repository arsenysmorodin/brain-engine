//! Frozen-PK experiments for all three compounds. No inferred receptor affinity or clinical effect.
use crate::{CirculationTrace, NetworkTrace, simulate_network};
use neuro_domain::{BrainAtlas, Error, PURINE_NEURAL_MODEL_ID, PopulationStimulus, require};
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct PurineRegionalDrive {
    pub region_id: String,
    pub concentrations_mg_l: [f64; 3],
    pub substance_drives: [f64; 3],
    pub total_drive: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct PurineNetworkComparison {
    pub model_id: String,
    pub model_revision: u32,
    pub pk_time_min: f64,
    pub hypothesis: String,
    pub regional_drivers: Vec<PurineRegionalDrive>,
    pub baseline: NetworkTrace,
    pub with_hypothesis: NetworkTrace,
}
/// Direct analytic definition used as the oracle in tests; scale is explicitly illustrative.
pub fn purine_illustrative_drive(ct: [f64; 3], gains: [f64; 3], scales: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|d| gains[d] * ct[d] / (scales[d] + ct[d]))
}
pub fn compare_purine_network(
    trace: &CirculationTrace,
    time_min: f64,
    atlas: &BrainAtlas,
) -> Result<PurineNetworkComparison, Error> {
    trace.scenario.validate(atlas)?;
    require(
        time_min.is_finite() && (0.0..=trace.scenario.duration_min).contains(&time_min),
        "Время E/I-снимка вне трассы PK",
    )?;
    let h = trace
        .scenario
        .neural_hypothesis
        .as_ref()
        .filter(|h| h.enabled)
        .ok_or_else(|| {
            Error::Validation("Включите учебную гипотезу Ct → E/I перед сравнением".into())
        })?;
    let frame = trace.at(time_min);
    let mut network = h.network.clone();
    let mut drivers = Vec::new();
    for p in &h.network.populations {
        let bed = trace
            .scenario
            .beds
            .regions
            .iter()
            .position(|r| r.region_id == p.region_id)
            .expect("all grey populations have a bed");
        let ct = frame.regions[bed].map(|r| r.tissue_mg_l);
        let drives = purine_illustrative_drive(ct, h.drive_gains, h.concentration_scales_mg_l);
        let total = drives.iter().sum();
        network.stimuli.push(PopulationStimulus {
            region_id: p.region_id.clone(),
            start_ms: 0.0,
            duration_ms: network.duration_ms,
            amplitude: total,
        });
        drivers.push(PurineRegionalDrive {
            region_id: p.region_id.clone(),
            concentrations_mg_l: ct,
            substance_drives: drives,
            total_drive: total,
        });
    }
    Ok(PurineNetworkComparison{model_id:PURINE_NEURAL_MODEL_ID.into(),model_revision:1,pk_time_min:frame.time_min,hypothesis:"Учебная гипотеза: ΔP_E(r)=Σ g_d·Ct_d(r)/(S_d+Ct_d(r)). g и S заданы исследователем; нет измеренной человеческой нейронной калибровки M/T. Это не PET, не доказанная синергия и не прогноз поведения. Два одинаковых миллисекундных сетевых протокола при замороженном снимке PK.".into(),regional_drivers:drivers,baseline:simulate_network(h.network.clone(),atlas)?,with_hypothesis:simulate_network(network,atlas)?})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gain_rule_is_signed_additive_and_uses_a_concentration_scale() {
        assert_eq!(
            purine_illustrative_drive([1.0, 2.0, 4.0], [1.0, -2.0, 0.0], [1.0, 2.0, 4.0]),
            [0.5, -1.0, 0.0]
        );
        assert_eq!(
            purine_illustrative_drive([0.0; 3], [1.0; 3], [1.0; 3]),
            [0.0; 3]
        );
    }
    #[test]
    fn all_compounds_use_local_ct_zero_drivers_reproduce_baseline_and_disabled_is_rejected() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = neuro_domain::CirculationScenario::reference(&a, 7);
        s.duration_min = 5.0;
        s.neural_hypothesis = Some(neuro_domain::PurineNeuralHypothesis::reference(&a).unwrap());
        let disabled = crate::simulate_circulation(s.clone(), &a).unwrap();
        assert!(compare_purine_network(&disabled, 5.0, &a).is_err());
        s.neural_hypothesis.as_mut().unwrap().enabled = true;
        let first = s.neural_hypothesis.as_ref().unwrap().network.populations[0]
            .region_id
            .clone();
        s.beds
            .regions
            .iter_mut()
            .find(|r| r.region_id == first)
            .unwrap()
            .flow_fraction = 0.0;
        let t = crate::simulate_circulation(s.clone(), &a).unwrap();
        let zero = compare_purine_network(&t, 0.0, &a).unwrap();
        assert_eq!(zero.baseline.samples, zero.with_hypothesis.samples);
        let c = compare_purine_network(&t, 5.0, &a).unwrap();
        assert_eq!(c.regional_drivers[0].concentrations_mg_l, [0.0; 3]);
        assert!(
            c.regional_drivers
                .iter()
                .any(|r| r.substance_drives.iter().all(|g| *g > 0.0))
        );
        assert_ne!(c.baseline.samples, c.with_hypothesis.samples);
        assert!(compare_purine_network(&t, f64::NAN, &a).is_err());
        s.neural_hypothesis.as_mut().unwrap().drive_gains = [0.0; 3];
        let t = crate::simulate_circulation(s, &a).unwrap();
        let c = compare_purine_network(&t, 5.0, &a).unwrap();
        assert_eq!(c.baseline.samples, c.with_hypothesis.samples);
    }
    #[test]
    fn shared_delayed_dose_time_is_translation_invariant_but_revision_one_is_preserved() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = neuro_domain::CirculationScenario::reference(&a, 7);
        s.duration_min = 10.0;
        let original = crate::simulate_circulation(s.clone(), &a).unwrap();
        for d in &mut s.doses {
            d.time_min = 5.0;
        }
        assert!(s.interaction_profile_active());
        let shifted = crate::simulate_circulation(s.clone(), &a).unwrap();
        for d in 0..3 {
            assert_eq!(shifted.at(4.5).systemic[d].administered_mg, 0.0);
            assert!(
                (original.at(5.0).systemic[d].central_mg_l
                    - shifted.at(10.0).systemic[d].central_mg_l)
                    .abs()
                    < 1e-12
            );
        }
        s.model_revision = 1;
        assert!(!s.interaction_profile_active());
        let legacy = neuro_domain::CirculationScenario::from_json(
            include_str!("../../../scenarios/purine-circulation.json"),
            &a,
        )
        .unwrap();
        assert_eq!(legacy.model_revision, 1);
        assert!(legacy.neural_hypothesis.is_none());
        assert!(shifted.csv().lines().nth(1).unwrap().starts_with(&format!(
            "purine_closed_circulation_v1,{},",
            neuro_domain::CIRCULATION_MODEL_REVISION
        )));
    }
}
