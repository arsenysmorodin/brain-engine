//! Conservative positive pairwise Strang splitting of a closed circulation, f64 CPU only.
use neuro_domain::{BrainAtlas, CirculationScenario, Error, Purine, require};
use serde::Serialize;
pub const CIRCULATION_STEP_MIN: f64 = 0.005;
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct DrugBedSample {
    pub capillary_mg_l: f64,
    pub tissue_mg_l: f64,
    pub venous_mg_l: f64,
    pub capillary_mg: f64,
    pub tissue_mg: f64,
    pub venous_mg: f64,
    pub inflow_mg_min: f64,
    pub return_mg_min: f64,
    pub cumulative_in_mg: f64,
    pub cumulative_return_mg: f64,
}
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct SystemicSample {
    pub administered_mg: f64,
    pub gut_mg: f64,
    pub rest_mg: f64,
    pub central_mg_l: f64,
    pub brain_mg: f64,
    pub eliminated_mg: f64,
    pub return_mg_min: f64,
    pub auc_mg_min_l: f64,
    pub balance_error_mg: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct CirculationFrame {
    pub time_min: f64,
    pub systemic: [SystemicSample; 3],
    pub cbf_ml_100g_min: Vec<f64>,
    pub regions: Vec<[DrugBedSample; 3]>,
}
#[derive(Clone, Debug, Serialize)]
pub struct CirculationTrace {
    pub model_id: String,
    pub solver: String,
    pub step_min: f64,
    pub scenario: CirculationScenario,
    pub frames: Vec<CirculationFrame>,
}
#[derive(Clone, Copy, Default)]
struct Bed {
    cap: f64,
    tissue: f64,
    vein: f64,
    incoming: f64,
    returned: f64,
}
#[derive(Default)]
struct State {
    gut: f64,
    rest: f64,
    eliminated: f64,
    administered: f64,
    auc: f64,
    beds: Vec<Bed>,
}
fn transfer(from: &mut f64, to: &mut f64, rate: f64, dt: f64) -> f64 {
    let amount = *from * (-(-rate * dt).exp_m1());
    *from -= amount;
    *to += amount;
    amount
}
/// Exact reversible capillary/tissue exchange dMt=PS(Ccap-Ct/Kp).
fn exchange(cap: &mut f64, tissue: &mut f64, vc: f64, kt: f64, ps: f64, dt: f64) {
    let total = *cap + *tissue;
    let target = total * kt / (vc + kt);
    let delta = (target - *tissue) * (-(-ps * (1.0 / vc + 1.0 / kt) * dt).exp_m1());
    *cap -= delta;
    *tissue += delta;
}
fn rest_volumes(s: &CirculationScenario) -> [f64; 3] {
    let vt = s.beds.regions.iter().map(|r| r.volume_l()).sum::<f64>();
    s.effective_parameters().map(|p| {
        p.apparent_volume_l
            - vt * (p.tissue_blood_partition
                + s.capillary_volume_fraction
                + s.venous_volume_fraction)
    })
}
fn q(s: &CirculationScenario, caffeine: f64, index: usize) -> f64 {
    let r = &s.beds.regions[index];
    let reduction = if s.beds.vasoconstriction_enabled {
        s.beds.max_flow_reduction * caffeine / (s.beds.flow_half_effect_mg_l + caffeine)
    } else {
        0.0
    };
    r.baseline_flow_ml_min() * r.flow_fraction * (1.0 - reduction) / 1000.0
}
pub fn simulate_circulation(
    s: CirculationScenario,
    atlas: &BrainAtlas,
) -> Result<CirculationTrace, Error> {
    simulate_with_step(s, atlas, CIRCULATION_STEP_MIN)
}
fn simulate_with_step(
    s: CirculationScenario,
    atlas: &BrainAtlas,
    dt: f64,
) -> Result<CirculationTrace, Error> {
    s.validate(atlas)?;
    let n = s.beds.regions.len();
    let params = s.effective_parameters();
    let vr = rest_volumes(&s);
    let mut states: [State; 3] = std::array::from_fn(|_| State {
        beds: vec![Bed::default(); n],
        ..Default::default()
    });
    let mut frames = vec![];
    let steps = (s.duration_min / dt).round() as usize;
    for step in 0..=steps {
        let t = step as f64 * dt;
        for d in &s.doses {
            if (d.time_min / dt).round() as usize == step {
                let st = &mut states[d.substance.index()];
                st.gut += d.amount_mg;
                st.administered += d.amount_mg;
            }
        }
        if (t <= 5.0 + 1e-9 && step % ((0.05 / dt).round() as usize) == 0)
            || step % ((0.5 / dt).round() as usize) == 0
        {
            let flow: Vec<_> = (0..n).map(|i| q(&s, states[0].rest / vr[0], i)).collect();
            let regions = (0..n)
                .map(|i| {
                    let vt = s.beds.regions[i].volume_l();
                    std::array::from_fn(|d| {
                        let b = states[d].beds[i];
                        DrugBedSample {
                            capillary_mg_l: b.cap / (vt * s.capillary_volume_fraction),
                            tissue_mg_l: b.tissue / vt,
                            venous_mg_l: b.vein / (vt * s.venous_volume_fraction),
                            capillary_mg: b.cap,
                            tissue_mg: b.tissue,
                            venous_mg: b.vein,
                            inflow_mg_min: flow[i] * states[d].rest / vr[d],
                            return_mg_min: flow[i] * b.vein / (vt * s.venous_volume_fraction),
                            cumulative_in_mg: b.incoming,
                            cumulative_return_mg: b.returned,
                        }
                    })
                })
                .collect::<Vec<_>>();
            let systemic = std::array::from_fn(|d| {
                let st = &states[d];
                let brain = st
                    .beds
                    .iter()
                    .map(|b| b.cap + b.tissue + b.vein)
                    .sum::<f64>();
                SystemicSample {
                    administered_mg: st.administered,
                    gut_mg: st.gut,
                    rest_mg: st.rest,
                    central_mg_l: st.rest / vr[d],
                    brain_mg: brain,
                    eliminated_mg: st.eliminated,
                    return_mg_min: regions.iter().map(|r| r[d].return_mg_min).sum(),
                    auc_mg_min_l: st.auc,
                    balance_error_mg: st.gut + st.rest + brain + st.eliminated - st.administered,
                }
            });
            require(
                systemic.iter().all(|v| {
                    v.balance_error_mg.abs() < 1e-6
                        && [
                            v.gut_mg,
                            v.rest_mg,
                            v.brain_mg,
                            v.eliminated_mg,
                            v.central_mg_l,
                        ]
                        .iter()
                        .all(|x| x.is_finite() && *x >= -1e-10)
                }) && regions.iter().flatten().all(|r| {
                    (r.capillary_mg + r.tissue_mg + r.venous_mg - r.cumulative_in_mg
                        + r.cumulative_return_mg)
                        .abs()
                        < 1e-7
                }),
                "Замкнутая циркуляция: нарушен баланс массы или положительность",
            )?;
            frames.push(CirculationFrame {
                time_min: t,
                systemic,
                cbf_ml_100g_min: flow
                    .iter()
                    .zip(&s.beds.regions)
                    .map(|(q, r)| q * 100_000.0 / r.tissue_mass_g)
                    .collect(),
                regions,
            });
        }
        if step == steps {
            break;
        }
        // Midpoint predictor for the common caffeine-dependent blood flow.
        let caf = &states[0];
        let cp = caf.rest / vr[0];
        let mut derivative = caf.gut * std::f64::consts::LN_2 / params[0].absorption_half_min
            - params[0].apparent_clearance_l_h / 60.0 * cp;
        for (i, b) in caf.beds.iter().enumerate() {
            derivative += q(&s, cp, i)
                * (b.vein / (s.beds.regions[i].volume_l() * s.venous_volume_fraction) - cp);
        }
        let cp_mid = (cp + dt * 0.5 * derivative / vr[0]).max(0.0);
        let flows: Vec<_> = (0..n).map(|i| q(&s, cp_mid, i)).collect();
        for d in 0..3 {
            let st = &mut states[d];
            let p = &params[d];
            let before = st.rest / vr[d];
            let half = dt * 0.5;
            transfer(
                &mut st.gut,
                &mut st.rest,
                std::f64::consts::LN_2 / p.absorption_half_min,
                half,
            );
            transfer(
                &mut st.rest,
                &mut st.eliminated,
                p.apparent_clearance_l_h / 60.0 / vr[d],
                half,
            );
            for reverse in [false, true] {
                for k in 0..n {
                    let i = if reverse { n - 1 - k } else { k };
                    let vt = s.beds.regions[i].volume_l();
                    let vc = vt * s.capillary_volume_fraction;
                    let vv = vt * s.venous_volume_fraction;
                    let b = &mut st.beds[i];
                    let flow = flows[i];
                    if reverse {
                        exchange(
                            &mut b.cap,
                            &mut b.tissue,
                            vc,
                            vt * p.tissue_blood_partition,
                            p.exchange_min_inv * vt,
                            half,
                        );
                        b.returned += transfer(&mut b.vein, &mut st.rest, flow / vv, half);
                        transfer(&mut b.cap, &mut b.vein, flow / vc, half);
                        b.incoming += transfer(&mut st.rest, &mut b.cap, flow / vr[d], half);
                    } else {
                        b.incoming += transfer(&mut st.rest, &mut b.cap, flow / vr[d], half);
                        transfer(&mut b.cap, &mut b.vein, flow / vc, half);
                        b.returned += transfer(&mut b.vein, &mut st.rest, flow / vv, half);
                        exchange(
                            &mut b.cap,
                            &mut b.tissue,
                            vc,
                            vt * p.tissue_blood_partition,
                            p.exchange_min_inv * vt,
                            half,
                        );
                    }
                }
            }
            transfer(
                &mut st.rest,
                &mut st.eliminated,
                p.apparent_clearance_l_h / 60.0 / vr[d],
                half,
            );
            transfer(
                &mut st.gut,
                &mut st.rest,
                std::f64::consts::LN_2 / p.absorption_half_min,
                half,
            );
            st.auc += dt * 0.5 * (before + st.rest / vr[d]);
        }
    }
    Ok(CirculationTrace {
        model_id: s.model_id.clone(),
        solver: "conservative_pairwise_strang_v1".into(),
        step_min: dt,
        scenario: s,
        frames,
    })
}
impl CirculationTrace {
    pub fn at(&self, t: f64) -> &CirculationFrame {
        let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
        let i = self
            .frames
            .partition_point(|f| f.time_min < t)
            .min(self.frames.len() - 1);
        if i > 0 && (self.frames[i - 1].time_min - t).abs() < (self.frames[i].time_min - t).abs() {
            &self.frames[i - 1]
        } else {
            &self.frames[i]
        }
    }
    pub fn csv(&self) -> String {
        let mut out = String::from(
            "model_id,model_revision,time_min,substance,administered_mg,gut_mg,rest_mg,central_mg_l,brain_mg,eliminated_mg,return_mg_min,auc_mg_min_l,balance_error_mg\n",
        );
        for f in &self.frames {
            for p in Purine::ALL {
                let r = f.systemic[p.index()];
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    self.model_id,
                    self.scenario.model_revision,
                    f.time_min,
                    p.key(),
                    r.administered_mg,
                    r.gut_mg,
                    r.rest_mg,
                    r.central_mg_l,
                    r.brain_mg,
                    r.eliminated_mg,
                    r.return_mg_min,
                    r.auc_mg_min_l,
                    r.balance_error_mg
                ));
            }
        }
        out
    }
    pub fn regional_csv(&self) -> String {
        let mut out = String::from(
            "model_id,model_revision,time_min,region_id,substance,cbf_ml_100g_min,capillary_mg_l,tissue_mg_l,venous_mg_l,capillary_mg,tissue_mg,venous_mg,inflow_mg_min,return_mg_min,cumulative_in_mg,cumulative_return_mg\n",
        );
        for f in &self.frames {
            for (i, region) in self.scenario.beds.regions.iter().enumerate() {
                for p in Purine::ALL {
                    let r = f.regions[i][p.index()];
                    out.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                        self.model_id,
                        self.scenario.model_revision,
                        f.time_min,
                        region.region_id,
                        p.key(),
                        f.cbf_ml_100g_min[i],
                        r.capillary_mg_l,
                        r.tissue_mg_l,
                        r.venous_mg_l,
                        r.capillary_mg,
                        r.tissue_mg,
                        r.venous_mg,
                        r.inflow_mg_min,
                        r.return_mg_min,
                        r.cumulative_in_mg,
                        r.cumulative_return_mg
                    ));
                }
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_exchange_conserves_mass_and_equilibrates() {
        let (mut cap, mut tissue) = (1.0, 0.0);
        exchange(&mut cap, &mut tissue, 0.1, 2.0, 0.5, 100.0);
        assert!((cap + tissue - 1.0).abs() < 1e-14);
        assert!((cap / 0.1 - tissue / 2.0).abs() < 1e-12);
        let before = (cap, tissue);
        exchange(&mut cap, &mut tissue, 0.1, 2.0, 0.0, 100.0);
        assert_eq!((cap, tissue), before);
    }
    #[test]
    fn closed_loop_balances_each_drug_and_bed_and_returns_to_systemic() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 7);
        s.duration_min = 10.0;
        s.beds.vasoconstriction_enabled = false;
        s.beds.regions[0].flow_fraction = 0.0;
        let tr = simulate_circulation(s.clone(), &a).unwrap();
        assert_eq!(tr.at(0.0).systemic[0].rest_mg, 0.0);
        for f in &tr.frames {
            for d in 0..3 {
                assert!(f.systemic[d].balance_error_mg.abs() < 1e-8);
                assert_eq!(f.regions[0][d].tissue_mg, 0.0);
            }
        }
        for d in 0..3 {
            assert!(tr.at(1.0).systemic[d].return_mg_min > 0.0);
            assert!(tr.at(10.0).systemic[d].eliminated_mg > 0.0);
        }
        let fine = simulate_with_step(s, &a, 0.0025).unwrap();
        let max = tr
            .frames
            .iter()
            .zip(&fine.frames)
            .flat_map(|(a, b)| {
                a.regions
                    .iter()
                    .zip(&b.regions)
                    .map(|(a, b)| (a[0].tissue_mg_l - b[0].tissue_mg_l).abs())
            })
            .fold(0.0_f64, f64::max);
        assert!(max < 0.002, "refinement {max}");
    }
    #[test]
    fn delayed_dose_no_interaction_and_zero_exchange_are_causal() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 2);
        s.duration_min = 5.0;
        s.doses[0].time_min = 2.5;
        s.parameters[1].exchange_min_inv = 0.0;
        let tr = simulate_circulation(s, &a).unwrap();
        assert!(
            tr.frames
                .iter()
                .filter(|f| f.time_min < 2.5)
                .all(|f| f.systemic[1].administered_mg == 0.0)
        );
        assert!(
            tr.frames
                .iter()
                .all(|f| f.regions.iter().all(|r| r[1].tissue_mg == 0.0))
        );
        assert!(tr.at(5.0).regions[1][1].venous_mg > 0.0);
        assert!(
            tr.frames
                .iter()
                .all(|f| f.systemic[0].rest_mg == 0.0 && f.systemic[2].rest_mg == 0.0)
        );
    }
    #[test]
    fn with_zero_flow_rest_is_analytical_bateman_and_elimination() {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CirculationScenario::reference(&a, 1);
        s.duration_min = 5.0;
        for r in &mut s.beds.regions {
            r.flow_fraction = 0.0;
        }
        let vr = rest_volumes(&s)[0];
        let ka = std::f64::consts::LN_2 / s.parameters[0].absorption_half_min;
        let ke = s.parameters[0].apparent_clearance_l_h / 60.0 / vr;
        let tr = simulate_circulation(s, &a).unwrap();
        let expected = 150.0 * ka * ((-ke * 5.0).exp() - (-ka * 5.0).exp()) / (ka - ke);
        assert!((tr.at(5.0).systemic[0].rest_mg - expected).abs() < 2e-6);
    }
}

#[cfg(test)]
mod long_horizon_tests {
    use super::*;
    #[test]
    fn triple_24h_preserves_mass_and_concentration_dependent_flow_refines() {
        let a = BrainAtlas::bundled().unwrap();
        let s = CirculationScenario::reference(&a, 7);
        let tr = simulate_circulation(s.clone(), &a).unwrap();
        assert_eq!(tr.frames.last().unwrap().time_min, 1440.0);
        for f in &tr.frames {
            for x in &f.systemic {
                assert!(x.balance_error_mg.abs() < 1e-7);
            }
        }
        let last = tr.frames.last().unwrap();
        assert!(last.systemic[1].eliminated_mg > 99.0);
        assert!(last.systemic[2].brain_mg > last.systemic[1].brain_mg);
        let mut short = s;
        short.duration_min = 10.0;
        let normal = simulate_circulation(short.clone(), &a).unwrap();
        let fine = simulate_with_step(short, &a, 0.0025).unwrap();
        for (x, y) in normal.frames.iter().zip(&fine.frames) {
            assert!((x.systemic[0].central_mg_l - y.systemic[0].central_mg_l).abs() < 0.0001);
            for (x, y) in x.regions.iter().zip(&y.regions) {
                assert!((x[0].tissue_mg_l - y[0].tissue_mg_l).abs() < 0.002);
            }
        }
    }
}
