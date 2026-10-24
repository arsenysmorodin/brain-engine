//! One-way regional perfusion experiment driven by the analytical oral plasma boundary.
//! Local amounts balance inflow/outflow. They must NOT be added to the systemic PK amount.
use neuro_domain::{
    ArterialTerritory, BrainAtlas, CaffeineScenario, Error, PERFUSION_MODEL_ID, RegionalPerfusion,
    require,
};
use serde::Serialize;

pub const PERFUSION_STEP_MIN: f64 = 0.025;
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RegionalPerfusionSample {
    pub arterial_mg_l: f64,
    pub tissue_mg_l: f64,
    pub cbf_ml_100g_min: f64,
    pub inflow_mg_min: f64,
    pub outflow_mg_min: f64,
    pub tissue_mg: f64,
    pub cumulative_in_mg: f64,
    pub cumulative_out_mg: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PerfusionFrame {
    pub time_min: f64,
    /// Same order as scenario.perfusion.regions.
    pub regions: Vec<RegionalPerfusionSample>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PerfusionTrace {
    pub model_id: String,
    pub solver: String,
    pub step_min: f64,
    pub boundary: String,
    pub frames: Vec<PerfusionFrame>,
}
impl PerfusionTrace {
    pub fn at(&self, time_min: f64) -> &PerfusionFrame {
        let i = self.frames.partition_point(|s| s.time_min < time_min);
        let i = i.min(self.frames.len() - 1);
        if i > 0
            && (time_min - self.frames[i - 1].time_min).abs()
                < (self.frames[i].time_min - time_min).abs()
        {
            &self.frames[i - 1]
        } else {
            &self.frames[i]
        }
    }
    pub fn regional_csv(&self, scenario: &CaffeineScenario) -> String {
        let mut out = String::from(
            "model_id,provenance_id,solver,step_min,time_min,region_id,territory,arterial_mg_l,tissue_mg_l,cbf_ml_100g_min,inflow_mg_min,outflow_mg_min,tissue_mg,cumulative_in_mg,cumulative_out_mg\n",
        );
        let p = scenario
            .perfusion
            .as_ref()
            .expect("perfusion trace has configuration");
        for frame in &self.frames {
            for (r, s) in p.regions.iter().zip(&frame.regions) {
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    self.model_id,
                    p.provenance_id,
                    self.solver,
                    self.step_min,
                    frame.time_min,
                    r.region_id,
                    r.territory.key(),
                    s.arterial_mg_l,
                    s.tissue_mg_l,
                    s.cbf_ml_100g_min,
                    s.inflow_mg_min,
                    s.outflow_mg_min,
                    s.tissue_mg,
                    s.cumulative_in_mg,
                    s.cumulative_out_mg
                ));
            }
        }
        out
    }
    pub fn territory_flow(
        &self,
        scenario: &CaffeineScenario,
        time: f64,
        territory: ArterialTerritory,
    ) -> (f64, f64) {
        let p = scenario
            .perfusion
            .as_ref()
            .expect("perfusion trace has configuration");
        p.regions
            .iter()
            .zip(&self.at(time).regions)
            .filter(|(r, _)| r.territory == territory)
            .fold((0.0, 0.0), |(q, j), (r, s)| {
                (
                    q + s.cbf_ml_100g_min * r.tissue_mass_g / 100.0,
                    j + s.inflow_mg_min,
                )
            })
    }
}
/// Stable Bateman solution, including coincident ka=ke and doses at/after t.
/// Blood/plasma ratio is assumed 1 for this one-way experiment.
pub fn caffeine_arterial_boundary(s: &CaffeineScenario, time_min: f64) -> f64 {
    let p = &s.parameters;
    let ka = p.ka();
    let ke = p.ke();
    s.doses
        .iter()
        .filter_map(|d| {
            let t = time_min - d.time_min;
            if t <= 0.0 {
                return None;
            }
            let gap = ka - ke;
            let factor = if gap.abs() < 1e-12 {
                t * (-ke * t).exp()
            } else if gap > 0.0 {
                (-ke * t).exp() * (-(-gap * t).exp_m1()) / gap
            } else {
                (-ka * t).exp() * (-((gap * t).exp_m1())) / (-gap)
            };
            Some(p.bioavailability * d.amount_mg * ka * factor / p.volume_l())
        })
        .fold(0.0, |total, amount| total + amount)
}
fn flow(s: &CaffeineScenario, r: &RegionalPerfusion, ca: f64) -> f64 {
    let p = s.perfusion.as_ref().expect("configured");
    let reduction = if p.vasoconstriction_enabled {
        p.max_flow_reduction * ca / (p.flow_half_effect_mg_l + ca)
    } else {
        0.0
    };
    r.baseline_cbf_ml_100g_min * r.flow_fraction * (1.0 - reduction)
}
/// Exact update for fixed midpoint input/flow; positive for all permitted parameters.
fn bed_step(mass: f64, q_l_min: f64, ca: f64, kp_v: f64, dt: f64) -> (f64, f64, f64) {
    let relax = -(-q_l_min * dt / kp_v).exp_m1();
    let next = mass + (kp_v * ca - mass) * relax;
    let incoming = q_l_min * ca * dt;
    (next, incoming, incoming - (next - mass))
}
pub fn simulate_perfusion(
    s: &CaffeineScenario,
    atlas: &BrainAtlas,
) -> Result<Option<PerfusionTrace>, Error> {
    simulate_with_step(s, atlas, PERFUSION_STEP_MIN)
}
fn simulate_with_step(
    s: &CaffeineScenario,
    atlas: &BrainAtlas,
    dt: f64,
) -> Result<Option<PerfusionTrace>, Error> {
    s.validate(atlas)?;
    let Some(p) = &s.perfusion else {
        return Ok(None);
    };
    let n = p.regions.len();
    let mut mass = vec![0.0; n];
    let mut cin = vec![0.0; n];
    let mut cout = vec![0.0; n];
    let steps = (s.duration_min / dt).round() as usize;
    let early = (0.05 / dt).round() as usize;
    let regular = (0.5 / dt).round() as usize;
    let mut frames = Vec::new();
    for step in 0..=steps {
        let t = step as f64 * dt;
        if (t <= 5.0 + 1e-9 && step % early == 0) || step % regular == 0 {
            let regions = p
                .regions
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let ca = caffeine_arterial_boundary(s, t - r.transit_sec / 60.0);
                    let cbf = flow(s, r, ca);
                    let q = cbf * r.tissue_mass_g / 100_000.0;
                    RegionalPerfusionSample {
                        arterial_mg_l: ca,
                        tissue_mg_l: mass[i] / r.volume_l(),
                        cbf_ml_100g_min: cbf,
                        inflow_mg_min: q * ca,
                        outflow_mg_min: q * mass[i] / (r.partition_tissue_blood * r.volume_l()),
                        tissue_mg: mass[i],
                        cumulative_in_mg: cin[i],
                        cumulative_out_mg: cout[i],
                    }
                })
                .collect::<Vec<_>>();
            require(
                regions.iter().all(|r| {
                    [
                        r.arterial_mg_l,
                        r.tissue_mg_l,
                        r.cbf_ml_100g_min,
                        r.inflow_mg_min,
                        r.outflow_mg_min,
                        r.tissue_mg,
                        r.cumulative_in_mg,
                        r.cumulative_out_mg,
                    ]
                    .iter()
                    .all(|x| x.is_finite() && *x >= -1e-10)
                        && (r.tissue_mg - r.cumulative_in_mg + r.cumulative_out_mg).abs() < 1e-8
                }),
                "Перфузия: нарушена конечность, положительность или локальный баланс массы",
            )?;
            frames.push(PerfusionFrame {
                time_min: t,
                regions,
            });
        }
        if step == steps {
            break;
        }
        for (i, r) in p.regions.iter().enumerate() {
            let ca = caffeine_arterial_boundary(s, t + dt * 0.5 - r.transit_sec / 60.0);
            let q = flow(s, r, ca) * r.tissue_mass_g / 100_000.0;
            let (next, incoming, outgoing) =
                bed_step(mass[i], q, ca, r.partition_tissue_blood * r.volume_l(), dt);
            mass[i] = next;
            cin[i] += incoming;
            cout[i] += outgoing;
        }
    }
    Ok(Some(PerfusionTrace {model_id:PERFUSION_MODEL_ID.into(),solver:"exponential_midpoint_beds_v1".into(),step_min:dt,boundary:"Prescribed analytical oral Cp(t-transit); blood/plasma=1; no return coupling to systemic PK. Regional mass is a diagnostic subset and is not added to systemic_mg.".into(),frames}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> (BrainAtlas, CaffeineScenario) {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = CaffeineScenario::reference(&a).unwrap();
        s.duration_min = 60.0;
        (a, s)
    }
    #[test]
    fn fixed_bed_has_exact_uptake_equilibrium_washout_and_zero_flow() {
        let mut m = 0.0;
        let mut incoming = 0.0;
        let mut outgoing = 0.0;
        for _ in 0..100 {
            let (n, i, o) = bed_step(m, 0.02, 4.0, 0.1, 0.1);
            m = n;
            incoming += i;
            outgoing += o;
        }
        assert!((m - 0.4 * (1.0 - (-2.0_f64).exp())).abs() < 1e-14);
        assert!((m - incoming + outgoing).abs() < 1e-14);
        let (wash, _, _) = bed_step(m, 0.02, 0.0, 0.1, 10.0);
        assert!((wash - m * (-2.0_f64).exp()).abs() < 1e-14);
        assert_eq!(bed_step(m, 0.0, 4.0, 0.1, 10.0), (m, 0.0, 0.0));
    }
    #[test]
    fn delivery_is_causal_flow_sensitive_balanced_and_convergent() {
        let (a, mut s) = reference();
        assert_eq!(
            caffeine_arterial_boundary(&s, 0.0).to_bits(),
            0.0_f64.to_bits()
        );
        let p = s.perfusion.as_mut().unwrap();
        p.vasoconstriction_enabled = false;
        for r in &mut p.regions {
            r.baseline_cbf_ml_100g_min = 50.0;
            r.transit_sec = 0.0;
        }
        p.regions[0].flow_fraction = 0.0;
        p.regions[1].flow_fraction = 0.25;
        p.regions[2].transit_sec = 30.0;
        let tr = simulate_perfusion(&s, &a).unwrap().unwrap();
        assert!(tr.frames.iter().all(|f| f.regions[0].tissue_mg == 0.0));
        assert_eq!(tr.at(0.5).regions[2].tissue_mg, 0.0);
        assert!(tr.at(2.0).regions[1].tissue_mg_l < tr.at(2.0).regions[3].tissue_mg_l);
        let fine = simulate_with_step(&s, &a, 0.0125).unwrap().unwrap();
        let max = tr
            .frames
            .iter()
            .zip(&fine.frames)
            .flat_map(|(x, y)| {
                x.regions
                    .iter()
                    .zip(&y.regions)
                    .map(|(x, y)| (x.tissue_mg_l - y.tissue_mg_l).abs())
            })
            .fold(0.0_f64, f64::max);
        assert!(max < 1e-5, "refined-step error {max}");
        for f in &tr.frames {
            for r in &f.regions {
                assert!((r.tissue_mg - r.cumulative_in_mg + r.cumulative_out_mg).abs() < 1e-10);
            }
        }
        s.doses.clear();
        let z = simulate_perfusion(&s, &a).unwrap().unwrap();
        assert!(z.frames.iter().all(|f| {
            f.regions
                .iter()
                .all(|r| r.tissue_mg == 0.0 && r.inflow_mg_min == 0.0)
        }));
    }
    #[test]
    fn vasoconstriction_is_separate_from_pet_and_can_be_disabled() {
        let (a, mut s) = reference();
        let on = simulate_perfusion(&s, &a).unwrap().unwrap();
        let p = s.perfusion.as_mut().unwrap();
        p.vasoconstriction_enabled = false;
        let off = simulate_perfusion(&s, &a).unwrap().unwrap();
        assert!(on.at(30.0).regions[0].cbf_ml_100g_min < off.at(30.0).regions[0].cbf_ml_100g_min);
        assert!(on.at(2.0).regions[0].tissue_mg_l < off.at(2.0).regions[0].tissue_mg_l);
        s.parameters.elimination_half_h = s.parameters.absorption_half_min / 60.0;
        // Analytic boundary supports the equal-rate limit independent of UI ranges.
        let k = s.parameters.ka();
        let exact = 100.0 * k * 2.0 * (-k * 2.0).exp() / s.parameters.volume_l();
        assert!((caffeine_arterial_boundary(&s, 2.0) - exact).abs() < 1e-12);
    }
}
