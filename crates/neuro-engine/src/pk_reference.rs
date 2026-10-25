use crate::CirculationTrace;
use neuro_domain::{Error, PurinePkReference};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct PurinePkComparison {
    pub reference: PurinePkReference,
    pub dose_time_min: f64,
    pub model_cmax_mg_l: f64,
    pub model_tmax_after_dose_min: f64,
    pub peak_captured: bool,
    /// Eventual AUC = oral apparent dose / CLapp; not the integral to the current GUI time.
    pub model_auc_inf_mg_h_l: f64,
    /// CL/V orientation, not a fitted terminal slope of this multi-compartment trace.
    pub model_distribution_half_h: f64,
    pub cmax_error_pct: f64,
    pub tmax_error_min: f64,
    pub auc_inf_error_pct: f64,
}

pub fn compare_purine_pk(trace: &CirculationTrace) -> Result<Vec<PurinePkComparison>, Error> {
    if !trace.scenario.calibration_curve_ids.is_empty() {
        return Ok(vec![]);
    }
    let parameters = trace.scenario.effective_parameters();
    let mut comparisons = Vec::new();
    for r in PurinePkReference::bundled()? {
        let Some(t) = r.matched_dose_time(&trace.scenario) else {
            continue;
        };
        let d = r.substance.index();
        let peak = trace
            .frames
            .iter()
            .max_by(|a, b| {
                a.systemic[d]
                    .central_mg_l
                    .total_cmp(&b.systemic[d].central_mg_l)
            })
            .expect("validated nonempty trace");
        let cmax = peak.systemic[d].central_mg_l;
        let tmax = peak.time_min - t;
        let p = &parameters[d];
        let auc = r.doses_mg[d] / p.apparent_clearance_l_h;
        comparisons.push(PurinePkComparison {
            dose_time_min: t,
            model_cmax_mg_l: cmax,
            model_tmax_after_dose_min: tmax,
            peak_captured: trace.scenario.duration_min > t + r.tmax_min
                && peak.time_min < trace.scenario.duration_min,
            model_auc_inf_mg_h_l: auc,
            model_distribution_half_h: std::f64::consts::LN_2 * p.apparent_volume_l
                / p.apparent_clearance_l_h,
            cmax_error_pct: 100.0 * (cmax / r.cmax_mg_l() - 1.0),
            tmax_error_min: tmax - r.tmax_min,
            auc_inf_error_pct: 100.0 * (auc / r.auc_inf_mg_h_l() - 1.0),
            reference: r,
        });
    }
    Ok(comparisons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulate_circulation;
    use neuro_domain::{BrainAtlas, CirculationScenario, Purine, PurineParameters};

    #[test]
    fn absorption_revision_moves_peaks_toward_published_endpoints() {
        let a = BrainAtlas::bundled().unwrap();
        for mask in [1, 2, 3, 4, 5] {
            let mut s = CirculationScenario::reference(&a, mask);
            s.duration_min = 240.0;
            let current = compare_purine_pk(&simulate_circulation(s.clone(), &a).unwrap()).unwrap();
            assert_eq!(current.len(), 1);
            s.model_revision = 2;
            s.parameters = Purine::ALL.map(|p| PurineParameters::reference_revision(p, 2));
            let old = compare_purine_pk(&simulate_circulation(s, &a).unwrap()).unwrap();
            let c = &current[0];
            assert!(c.peak_captured);
            assert!(
                c.tmax_error_min.abs() < old[0].tmax_error_min.abs(),
                "mask {mask}: {} vs {}",
                c.tmax_error_min,
                old[0].tmax_error_min
            );
            assert!(
                c.tmax_error_min.abs() < 4.0,
                "mask {mask}: {}",
                c.tmax_error_min
            );
            if mask == 1 {
                assert!(c.cmax_error_pct.abs() < old[0].cmax_error_pct.abs());
                assert!(c.auc_inf_error_pct.abs() < old[0].auc_inf_error_pct.abs());
            } else {
                assert_eq!(c.model_auc_inf_mg_h_l, old[0].model_auc_inf_mg_h_l);
            }
        }
    }
    #[test]
    fn shortened_and_repeated_protocols_are_not_complete_reference_curves() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 4);
        s.duration_min = 5.0;
        let tr = simulate_circulation(s.clone(), &a).unwrap();
        assert!(!compare_purine_pk(&tr).unwrap()[0].peak_captured);
        s.doses.push(s.doses[0].clone());
        assert!(
            compare_purine_pk(&simulate_circulation(s, &a).unwrap())
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn absorption_defaults_satisfy_bateman_peak_and_legacy_parameters_survive() {
        for p in Purine::ALL {
            let par = PurineParameters::reference(p);
            let ka = std::f64::consts::LN_2 / par.absorption_half_min;
            let ke = par.apparent_clearance_l_h / par.apparent_volume_l / 60.0;
            let target = [48.0, 46.2, 108.0][p.index()];
            assert!(((ka / ke).ln() / (ka - ke) - target).abs() < 1e-10);
        }
        let a = BrainAtlas::bundled().unwrap();
        let legacy = CirculationScenario::from_json(
            include_str!("../../../scenarios/purine-circulation.json"),
            &a,
        )
        .unwrap();
        assert_eq!(legacy.model_revision, 1);
        assert_eq!(legacy.parameters[2].absorption_half_min, 10.0);
        assert_eq!(legacy.effective_parameters()[2].absorption_half_min, 10.0);
    }
}
