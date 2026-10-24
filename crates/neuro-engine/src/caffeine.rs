//! RK4 caffeine mass balance in minutes; empirical PET fits use total plasma, not Kd.
use crate::{NetworkTrace, simulate_network};
use neuro_domain::{
    A1_PET_C50_MG_L, A2A_PD_PET_C50_MG_L, A2A_PD_PET_MAX, BrainAtlas, CAFFEINE_SAMPLE_MIN,
    CAFFEINE_STEP_MIN, CaffeineScenario, Error, NetworkScenario, PopulationStimulus, atomic_write,
    caffeine_a1_scope, caffeine_a2a_scope, require,
};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct CaffeineSample {
    pub time_min: f64,
    pub administered_mg: f64,
    pub gut_mg: f64,
    pub systemic_mg: f64,
    pub eliminated_parent_mg: f64,
    pub unavailable_mg: f64,
    pub plasma_mg_l: f64,
    pub brain_proxy_mg_l: f64,
    pub a1_occupancy: f64,
    pub a2a_pd_occupancy: f64,
    pub auc_mg_min_l: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaffeineTrace {
    pub solver: String,
    pub step_min: f64,
    pub sample_min: f64,
    pub early_sample_min: Option<f64>,
    pub scenario: CaffeineScenario,
    pub samples: Vec<CaffeineSample>,
    pub perfusion: Option<crate::PerfusionTrace>,
}
pub fn caffeine_pet_estimates(plasma_mg_l: f64) -> (f64, f64) {
    (
        plasma_mg_l / (A1_PET_C50_MG_L + plasma_mg_l),
        A2A_PD_PET_MAX * plasma_mg_l / (A2A_PD_PET_C50_MG_L + plasma_mg_l),
    )
}
impl CaffeineTrace {
    pub fn at(&self, time_min: f64) -> &CaffeineSample {
        let i = self
            .samples
            .partition_point(|s| s.time_min < time_min)
            .min(self.samples.len() - 1);
        if i > 0
            && (time_min - self.samples[i - 1].time_min).abs()
                < (self.samples[i].time_min - time_min).abs()
        {
            &self.samples[i - 1]
        } else {
            &self.samples[i]
        }
    }
    pub fn peak(&self) -> &CaffeineSample {
        self.samples
            .iter()
            .max_by(|a, b| a.plasma_mg_l.total_cmp(&b.plasma_mg_l))
            .expect("trace contains t=0")
    }
    pub fn csv(&self) -> String {
        let mut csv = String::from(
            "model_id,model_revision,provenance_id,solver,step_min,time_min,administered_mg,gut_mg,systemic_mg,eliminated_parent_mg,unavailable_mg,plasma_mg_l,brain_proxy_assumed_mg_l,a1_pet_estimate,a2a_pd_pet_estimate,auc_mg_min_l\n",
        );
        for s in &self.samples {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                self.scenario.model_id,
                self.scenario.model_revision,
                self.scenario.provenance_id,
                self.solver,
                self.step_min,
                s.time_min,
                s.administered_mg,
                s.gut_mg,
                s.systemic_mg,
                s.eliminated_parent_mg,
                s.unavailable_mg,
                s.plasma_mg_l,
                s.brain_proxy_mg_l,
                s.a1_occupancy,
                s.a2a_pd_occupancy,
                s.auc_mg_min_l
            ));
        }
        csv
    }
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        atomic_write(path.as_ref(), self.csv().as_bytes())
    }
}

pub fn simulate_caffeine(
    scenario: CaffeineScenario,
    atlas: &BrainAtlas,
) -> Result<CaffeineTrace, Error> {
    simulate_with_step(scenario, atlas, CAFFEINE_STEP_MIN)
}
fn simulate_with_step(
    scenario: CaffeineScenario,
    atlas: &BrainAtlas,
    dt: f64,
) -> Result<CaffeineTrace, Error> {
    scenario.validate(atlas)?;
    let p = &scenario.parameters;
    let (ka, ke, kb, v, f) = (p.ka(), p.ke(), p.kb(), p.volume_l(), p.bioavailability);
    let derivative = |y: [f64; 6]| -> [f64; 6] {
        [
            -ka * y[0],
            f * ka * y[0] - ke * y[1],
            ke * y[1],
            (1.0 - f) * ka * y[0],
            kb * (y[1] / v - y[4]),
            y[1] / v,
        ]
    };
    let add = |y: [f64; 6], k: [f64; 6], h: f64| std::array::from_fn(|i| y[i] + h * k[i]);
    let steps = (scenario.duration_min / dt).round() as usize;
    let record_steps = (CAFFEINE_SAMPLE_MIN / dt).round() as usize;
    let mut doses: Vec<_> = scenario
        .doses
        .iter()
        .map(|d| ((d.time_min / dt).round() as usize, d.amount_mg))
        .collect();
    doses.sort_by_key(|d| d.0);
    let mut dose_index = 0;
    let mut administered = 0.0;
    let mut y = [0.0; 6];
    let mut samples = Vec::with_capacity(steps / record_steps + 1);
    for step in 0..=steps {
        while doses.get(dose_index).is_some_and(|d| d.0 == step) {
            let d = doses[dose_index].1;
            y[0] += d;
            administered += d;
            dose_index += 1;
        }
        if step % record_steps == 0
            || (scenario.perfusion.is_some()
                && step as f64 * dt <= 5.0 + 1e-9
                && step % (0.05 / dt).round() as usize == 0)
        {
            let cp = y[1] / v;
            let (a1, a2a) = caffeine_pet_estimates(cp);
            require(
                y.iter().all(|x| x.is_finite() && *x >= -1e-10)
                    && (y[0] + y[1] + y[2] + y[3] - administered).abs()
                        < 1e-7 * administered.max(1.0),
                "Ошибка интегратора кофеина: нарушен баланс массы",
            )?;
            samples.push(CaffeineSample {
                time_min: step as f64 * dt,
                administered_mg: administered,
                gut_mg: y[0],
                systemic_mg: y[1],
                eliminated_parent_mg: y[2],
                unavailable_mg: y[3],
                plasma_mg_l: cp,
                brain_proxy_mg_l: y[4],
                a1_occupancy: a1,
                a2a_pd_occupancy: a2a,
                auc_mg_min_l: y[5],
            });
        }
        if step < steps {
            let k1 = derivative(y);
            let k2 = derivative(add(y, k1, dt * 0.5));
            let k3 = derivative(add(y, k2, dt * 0.5));
            let k4 = derivative(add(y, k3, dt));
            y = std::array::from_fn(|i| {
                y[i] + dt * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]) / 6.0
            });
        }
    }
    let perfusion = crate::simulate_perfusion(&scenario, atlas)?;
    Ok(CaffeineTrace {
        perfusion,
        solver: "rk4_mass_balance_v1".into(),
        step_min: dt,
        sample_min: CAFFEINE_SAMPLE_MIN,
        early_sample_min: scenario.perfusion.as_ref().map(|_| 0.05),
        scenario,
        samples,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaffeineNetworkComparison {
    pub pk_time_min: f64,
    pub driver_brain_proxy_mg_l: f64,
    pub hypothesis: String,
    pub regional_drivers_mg_l: Vec<(String, f64)>,
    pub baseline: NetworkTrace,
    pub caffeine_hypothesis: NetworkTrace,
}
/// A one-way, frozen PK snapshot. No conversion of hours into neural milliseconds.
pub fn compare_caffeine_network(
    trace: &CaffeineTrace,
    time_min: f64,
    atlas: &BrainAtlas,
) -> Result<CaffeineNetworkComparison, Error> {
    trace.scenario.validate(atlas)?;
    require(
        time_min.is_finite() && (0.0..=trace.scenario.duration_min).contains(&time_min),
        "Время снимка вне трассы кофеина",
    )?;
    let h = &trace.scenario.neural_hypothesis;
    require(
        h.enabled,
        "Включите гипотезу связи рецепторов с E/I перед расчётом",
    )?;
    let point = trace.at(time_min);
    let mut regional_drivers_mg_l = Vec::new();
    let mut network: NetworkScenario = h.network.clone();
    for p in &h.network.populations {
        let r = atlas.region(&p.region_id).expect("validated population");
        let driver = if let (Some(config), Some(perfusion)) =
            (&trace.scenario.perfusion, &trace.perfusion)
        {
            let i = config
                .regions
                .iter()
                .position(|r| r.region_id == p.region_id)
                .expect("complete perfusion beds");
            perfusion.at(time_min).regions[i].tissue_mg_l
        } else {
            point.brain_proxy_mg_l
        };
        regional_drivers_mg_l.push((p.region_id.clone(), driver));
        let (a1, a2a) = caffeine_pet_estimates(driver);
        let drive = if caffeine_a1_scope(r) {
            h.a1_drive_gain * a1
        } else {
            0.0
        } + if caffeine_a2a_scope(r) {
            h.a2a_striatal_drive_gain * a2a
        } else {
            0.0
        };
        network.stimuli.push(PopulationStimulus {
            region_id: p.region_id.clone(),
            start_ms: 0.0,
            duration_ms: network.duration_ms,
            amplitude: drive,
        });
    }
    Ok(CaffeineNetworkComparison { regional_drivers_mg_l, pk_time_min:point.time_min,driver_brain_proxy_mg_l:point.brain_proxy_mg_l,hypothesis:"Непроверенная гипотеза: P_extra = g_A1*fit_A1(Ce) + g_A2A*fit_A2A(Ce) в стриатуме. PET-функции перенесены с плазмы на региональную Ct при включённой перфузии, иначе на условный Ce. Одинаковый нейронный протокол с/без входа; это не предсказание поведения, BOLD или клинического эффекта.".into(),baseline:simulate_network(h.network.clone(),atlas)?,caffeine_hypothesis:simulate_network(network,atlas)? })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> (BrainAtlas, CaffeineScenario) {
        let a = BrainAtlas::bundled().unwrap();
        let s = CaffeineScenario::reference(&a).unwrap();
        (a, s)
    }
    #[test]
    fn oral_pk_matches_bateman_oracle_and_mass_balance() {
        let (a, mut s) = reference();
        s.parameters.bioavailability = 0.8;
        let p = s.parameters.clone();
        let tr = simulate_caffeine(s, &a).unwrap();
        for t in [0.0, 0.5, 10.0, 30.0, 60.0, 300.0, 1440.0] {
            let q = tr.at(t);
            let ka = p.ka();
            let ke = p.ke();
            let exact =
                0.8 * 100.0 * ka / (ka - ke) * ((-ke * t).exp() - (-ka * t).exp()) / p.volume_l();
            assert!(
                (q.plasma_mg_l - exact).abs() < 2e-10,
                "{t}: {} vs {exact}",
                q.plasma_mg_l
            );
            assert!(
                (q.gut_mg + q.systemic_mg + q.eliminated_parent_mg + q.unavailable_mg - 100.0)
                    .abs()
                    < 1e-9
            );
        }
        let tmax = (p.ka() / p.ke()).ln() / (p.ka() - p.ke());
        assert!((tr.peak().time_min - tmax).abs() <= 0.5);
        let tail = tr.at(1200.0).plasma_mg_l / tr.at(900.0).plasma_mg_l;
        assert!((tail - 0.5).abs() < 1e-10);
    }
    #[test]
    fn pk_handles_zero_future_repeated_and_coincident_rates() {
        let (a, mut s) = reference();
        s.doses.clear();
        let z = simulate_caffeine(s.clone(), &a).unwrap();
        assert!(
            z.samples.iter().all(|x| x.plasma_mg_l == 0.0
                && x.a1_occupancy == 0.0
                && x.brain_proxy_mg_l == 0.0)
        );
        s.doses = vec![neuro_domain::CaffeineDose {
            time_min: 60.0,
            amount_mg: 100.0,
        }];
        let delayed = simulate_caffeine(s.clone(), &a).unwrap();
        assert_eq!(delayed.at(59.5).administered_mg, 0.0);
        assert_eq!(delayed.at(60.0).plasma_mg_l, 0.0);
        s.doses.push(neuro_domain::CaffeineDose {
            time_min: 60.0,
            amount_mg: 100.0,
        });
        let twice = simulate_caffeine(s.clone(), &a).unwrap();
        assert_eq!(
            twice.at(61.0).plasma_mg_l,
            2.0 * delayed.at(61.0).plasma_mg_l
        );
        s.parameters.absorption_half_min = 60.0;
        s.parameters.elimination_half_h = 1.0;
        s.parameters.brain_equilibration_half_min = 60.0;
        let equal = simulate_caffeine(s.clone(), &a).unwrap();
        let q = equal.at(90.0);
        let k = s.parameters.ka();
        let v = s.parameters.volume_l();
        assert!((q.plasma_mg_l - 200.0 / v * k * 30.0 * (-k * 30.0).exp()).abs() < 1e-10);
        assert!(
            (q.brain_proxy_mg_l - 200.0 / v * k * k * 30.0 * 30.0 / 2.0 * (-k * 30.0).exp()).abs()
                < 1e-10
        );
    }
    #[test]
    fn pet_parameters_keep_a1_and_pd_a2a_distinct() {
        let (a1, _) = caffeine_pet_estimates(12.5);
        assert_eq!(a1, 0.5);
        let (_, a2) = caffeine_pet_estimates(2.2);
        assert!((a2 - 0.811 / 2.0).abs() < 1e-15);
        let (_, low) = caffeine_pet_estimates(4.48);
        assert!((low - 0.544).abs() < 0.002);
        let (_, high) = caffeine_pet_estimates(8.92);
        assert!((high - 0.651).abs() < 0.001);
    }
    #[test]
    fn fast_pk_matches_refinement_and_auc_integral() {
        let (a, mut s) = reference();
        s.parameters.absorption_half_min = 0.5;
        s.parameters.brain_equilibration_half_min = 0.5;
        let coarse = simulate_caffeine(s.clone(), &a).unwrap();
        let fine = simulate_with_step(s.clone(), &a, 0.025).unwrap();
        let max = coarse
            .samples
            .iter()
            .zip(&fine.samples)
            .map(|(x, y)| {
                (x.plasma_mg_l - y.plasma_mg_l)
                    .abs()
                    .max((x.brain_proxy_mg_l - y.brain_proxy_mg_l).abs())
            })
            .fold(0.0, f64::max);
        assert!(max < 2e-6, "{max}");
        let p = &s.parameters;
        let t = s.duration_min;
        let exact = 100.0 * p.ka() / p.volume_l() / (p.ka() - p.ke())
            * ((1.0 - (-p.ke() * t).exp()) / p.ke() - (1.0 - (-p.ka() * t).exp()) / p.ka());
        assert!((coarse.at(t).auc_mg_min_l - exact).abs() < 1e-8);
    }
    #[test]
    fn regional_neural_drivers_use_local_delivery_and_preserve_legacy() {
        let (a, mut s) = reference();
        s.duration_min = 60.0;
        s.neural_hypothesis.enabled = true;
        let id = s.neural_hypothesis.network.populations[0].region_id.clone();
        let p = s.perfusion.as_mut().unwrap();
        p.regions
            .iter_mut()
            .find(|r| r.region_id == id)
            .unwrap()
            .flow_fraction = 0.0;
        let t = simulate_caffeine(s.clone(), &a).unwrap();
        let c = compare_caffeine_network(&t, 2.0, &a).unwrap();
        assert_eq!(
            c.regional_drivers_mg_l
                .iter()
                .find(|(r, _)| r == &id)
                .unwrap()
                .1,
            0.0
        );
        assert!(c.regional_drivers_mg_l.iter().any(|(_, c)| *c > 0.0));
        s.perfusion = None;
        let legacy = simulate_caffeine(s, &a).unwrap();
        let c = compare_caffeine_network(&legacy, 2.0, &a).unwrap();
        assert!(
            c.regional_drivers_mg_l
                .iter()
                .all(|(_, c)| *c == legacy.at(2.0).brain_proxy_mg_l)
        );
    }
    #[test]
    fn neural_hypothesis_is_explicit_and_zero_driver_has_no_effect() {
        let (a, mut s) = reference();
        let disabled = simulate_caffeine(s.clone(), &a).unwrap();
        assert!(compare_caffeine_network(&disabled, 60.0, &a).is_err());
        s.neural_hypothesis.enabled = true;
        let tr = simulate_caffeine(s, &a).unwrap();
        let zero = compare_caffeine_network(&tr, 0.0, &a).unwrap();
        assert_eq!(zero.baseline.samples, zero.caffeine_hypothesis.samples);
        let dose = compare_caffeine_network(&tr, 60.0, &a).unwrap();
        assert_ne!(dose.baseline.samples, dose.caffeine_hypothesis.samples);
        assert_eq!(tr.samples[0].brain_proxy_mg_l, 0.0);
    }
}
