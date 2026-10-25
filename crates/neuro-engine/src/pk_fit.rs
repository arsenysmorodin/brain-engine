//! Exploratory fitting of digitized plasma curves. No GUI and no claim of population validation.
use neuro_domain::{
    BrainAtlas, CirculationScenario, DigitizedPkCurve, Error, PkCurveDataset, PkObservation,
    Purine, require,
};
use serde::{Deserialize, Serialize};

pub const PK_FIT_MODEL_ID: &str = "oral_plasma_curve_fit_v1";
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OralCurveParameters {
    pub volume_l: f64,
    pub clearance_l_h: f64,
    pub absorption_rate_h: f64,
    pub peripheral_volume_l: f64,
    pub distribution_clearance_l_h: f64,
    pub absorption_stages: u8,
}
impl OralCurveParameters {
    pub fn concentration(&self, dose_mg: f64, time_min: f64) -> f64 {
        if time_min <= 0.0 {
            return 0.0;
        }
        let t = time_min / 60.0;
        let k10 = self.clearance_l_h / self.volume_l;
        let ka = self.absorption_rate_h;
        let absorb = |lambda| absorption_response(ka, lambda, t, self.absorption_stages);
        let response = if self.peripheral_volume_l > 0.0 && self.distribution_clearance_l_h > 0.0 {
            let k12 = self.distribution_clearance_l_h / self.volume_l;
            let k21 = self.distribution_clearance_l_h / self.peripheral_volume_l;
            let sum = k10 + k12 + k21;
            let disc = (sum * sum - 4.0 * k10 * k21).max(0.0).sqrt();
            let alpha = (sum + disc) * 0.5;
            let beta = 2.0 * k10 * k21 / (sum + disc);
            let fast = (alpha - k21) / (alpha - beta);
            fast * absorb(alpha) + (1.0 - fast) * absorb(beta)
        } else {
            absorb(k10)
        };
        (dose_mg / self.volume_l * response).max(0.0)
    }
}
fn absorption_response(ka: f64, lambda: f64, t: f64, stages: u8) -> f64 {
    let d = ka - lambda;
    let x = d * t;
    let n = stages as usize;
    if x.abs() < 0.5 {
        let mut term = 1.0;
        let mut sum = 1.0 / n as f64;
        for k in 1..32 {
            term *= -x / k as f64;
            sum += term / (n + k) as f64;
        }
        let factorial = (1..n).product::<usize>() as f64;
        (ka * t).powi(n as i32) * (-lambda * t).exp() * sum / factorial
    } else {
        let mut sum = 1.0;
        let mut term = 1.0;
        for k in 1..n {
            term *= x / k as f64;
            sum += term;
        }
        (ka / d).powi(n as i32) * ((-lambda * t).exp() - (-ka * t).exp() * sum)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveFitMetrics {
    pub observations: usize,
    pub log_rmse: f64,
    pub mean_absolute_relative_error_pct: f64,
    pub max_absolute_relative_error_pct: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurvePrediction {
    pub observation: PkObservation,
    pub predicted_mg_l: f64,
    pub relative_error_pct: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveFitCandidate {
    pub compartments: u8,
    pub parameters: OralCurveParameters,
    pub training: CurveFitMetrics,
    pub leave_one_time_out: CurveFitMetrics,
    pub held_out_predictions_mg_l: Vec<f64>,
    /// Jackknife variation of positive parameters, not a confidence interval.
    pub jackknife_min: Vec<f64>,
    pub jackknife_max: Vec<f64>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PkCurveFit {
    pub curve: DigitizedPkCurve,
    pub selected: CurveFitCandidate,
    pub candidates: Vec<CurveFitCandidate>,
    pub predictions: Vec<CurvePrediction>,
    pub sampled_cmax_mg_l: f64,
    pub fitted_cmax_mg_l: f64,
    pub fitted_tmax_min: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PkCurveFitReport {
    pub model_id: String,
    pub model_revision: u32,
    pub dataset_id: String,
    pub data_kind: String,
    pub method: String,
    pub limitations: String,
    pub scope_gaps: Vec<String>,
    pub fits: Vec<PkCurveFit>,
}

fn metrics(pairs: &[(f64, f64)]) -> CurveFitMetrics {
    let n = pairs.len().max(1) as f64;
    CurveFitMetrics {
        observations: pairs.len(),
        log_rmse: (pairs
            .iter()
            .map(|(y, p)| (p.max(1e-15) / y).ln().powi(2))
            .sum::<f64>()
            / n)
            .sqrt(),
        mean_absolute_relative_error_pct: 100.0
            * pairs.iter().map(|(y, p)| (p / y - 1.0).abs()).sum::<f64>()
            / n,
        max_absolute_relative_error_pct: 100.0
            * pairs
                .iter()
                .map(|(y, p)| (p / y - 1.0).abs())
                .fold(0.0_f64, f64::max),
    }
}
fn bounds(compartments: u8) -> Vec<(f64, f64)> {
    let mut b = vec![
        (10.0_f64.ln(), 1000.0_f64.ln()),
        (0.1_f64.ln(), 500.0_f64.ln()),
        (0.7_f64.ln(), 80.0_f64.ln()),
    ];
    if compartments == 2 {
        b.extend([
            (1.0_f64.ln(), 5000.0_f64.ln()),
            (0.01_f64.ln(), 5000.0_f64.ln()),
        ]);
    }
    b
}
fn decode(x: &[f64], stages: u8) -> OralCurveParameters {
    OralCurveParameters {
        volume_l: x[0].exp(),
        clearance_l_h: x[1].exp(),
        absorption_rate_h: x[2].exp(),
        peripheral_volume_l: x.get(3).map_or(0.0, |v| v.exp()),
        distribution_clearance_l_h: x.get(4).map_or(0.0, |v| v.exp()),
        absorption_stages: stages,
    }
}
fn objective(x: &[f64], stages: u8, dose: f64, obs: &[PkObservation], omit: Option<usize>) -> f64 {
    let p = decode(x, stages);
    let mut loss = 0.0;
    for (i, o) in obs.iter().enumerate() {
        if o.exclude.is_some() || Some(i) == omit {
            continue;
        }
        let mut lo = f64::INFINITY;
        let mut hi = 0.0_f64;
        for j in 0..=8 {
            let t = (o.time_min + o.time_error_min * (j as f64 / 4.0 - 1.0)).max(0.0);
            let v = p.concentration(dose, t).max(1e-15);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let residual = if let Some(limit) = o.upper_limit_mg_l {
            (lo / limit).ln().max(0.0)
        } else {
            let y = o.concentration_mg_l;
            if y < lo {
                (lo / y).ln()
            } else if y > hi {
                (hi / y).ln()
            } else {
                0.0
            }
        };
        // A 10% log residual floor is a modelling weight, not measured SD/SEM.
        let sigma = o.log_concentration_error.max(0.10);
        loss += (residual / sigma).powi(2);
    }
    if loss.is_finite() { loss } else { 1e100 }
}
fn optimize(
    dose: f64,
    obs: &[PkObservation],
    compartments: u8,
    stages: u8,
    omit: Option<usize>,
) -> Vec<f64> {
    let b = bounds(compartments);
    let n = b.len();
    let peak = obs
        .iter()
        .enumerate()
        .filter(|(i, o)| Some(*i) != omit && o.exclude.is_none() && o.upper_limit_mg_l.is_none())
        .map(|(_, o)| o.concentration_mg_l)
        .fold(1e-9_f64, f64::max);
    let volume = (dose / peak * 0.8).clamp(12.0, 900.0);
    let usable: Vec<_> = obs
        .iter()
        .enumerate()
        .filter(|(i, o)| Some(*i) != omit && o.exclude.is_none() && o.upper_limit_mg_l.is_none())
        .map(|(_, o)| o)
        .collect();
    let a = usable[usable.len() - 2];
    let z = usable[usable.len() - 1];
    let rate = ((a.concentration_mg_l / z.concentration_mg_l).ln()
        / ((z.time_min - a.time_min) / 60.0))
        .clamp(0.01, 2.0);
    let project = |v: &mut Vec<f64>| {
        for (x, (l, h)) in v.iter_mut().zip(&b) {
            *x = x.clamp(*l, *h);
        }
    };
    let eval = |v: &[f64]| objective(v, stages, dose, obs, omit);
    let mut best = (Vec::new(), f64::INFINITY);
    for start in 0..3 {
        let mut x = vec![
            volume.ln(),
            (volume * rate).ln(),
            [2.0_f64, 5.0, 12.0][start].ln(),
        ];
        if compartments == 2 {
            x.extend([
                (volume * [0.5_f64, 2.0, 5.0][start]).ln(),
                (volume * rate * [0.3_f64, 1.0, 5.0][start]).ln(),
            ]);
        }
        project(&mut x);
        let mut simplex = vec![(x.clone(), eval(&x))];
        for j in 0..n {
            let mut v = x.clone();
            v[j] += 0.35;
            project(&mut v);
            simplex.push((v.clone(), eval(&v)));
        }
        for _ in 0..1000 {
            simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
            let spread = simplex
                .iter()
                .flat_map(|(v, _)| v.iter().zip(&simplex[0].0).map(|(x, y)| (x - y).abs()))
                .fold(0.0_f64, f64::max);
            if spread < 1e-6 {
                break;
            }
            let center: Vec<_> = (0..n)
                .map(|j| simplex[..n].iter().map(|(v, _)| v[j]).sum::<f64>() / n as f64)
                .collect();
            let make = |factor: f64| {
                let mut v: Vec<_> = center
                    .iter()
                    .zip(&simplex[n].0)
                    .map(|(c, w)| c + factor * (c - w))
                    .collect();
                project(&mut v);
                let f = eval(&v);
                (v, f)
            };
            let reflected = make(1.0);
            if reflected.1 < simplex[0].1 {
                let expanded = make(2.0);
                simplex[n] = if expanded.1 < reflected.1 {
                    expanded
                } else {
                    reflected
                };
            } else if reflected.1 < simplex[n - 1].1 {
                simplex[n] = reflected;
            } else {
                let contracted = make(if reflected.1 < simplex[n].1 {
                    0.5
                } else {
                    -0.5
                });
                if contracted.1 < simplex[n].1.min(reflected.1) {
                    simplex[n] = contracted;
                } else {
                    let first = simplex[0].0.clone();
                    for (v, f) in &mut simplex[1..] {
                        for (x, a) in v.iter_mut().zip(&first) {
                            *x = (*x + a) * 0.5;
                        }
                        *f = eval(v);
                    }
                }
            }
        }
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        if simplex[0].1 < best.1 {
            best = simplex.remove(0);
        }
    }
    best.0
}
fn candidate(curve: &DigitizedPkCurve, compartments: u8, stages: u8) -> CurveFitCandidate {
    let obs = curve.observations();
    let dose = curve.doses_mg[curve.substance.index()];
    let x = optimize(dose, &obs, compartments, stages, None);
    let parameters = decode(&x, stages);
    let indices: Vec<_> = obs
        .iter()
        .enumerate()
        .filter(|(_, o)| o.exclude.is_none() && o.upper_limit_mg_l.is_none())
        .map(|(i, _)| i)
        .collect();
    let training = metrics(
        &indices
            .iter()
            .map(|i| {
                (
                    obs[*i].concentration_mg_l,
                    parameters.concentration(dose, obs[*i].time_min),
                )
            })
            .collect::<Vec<_>>(),
    );
    let mut held_out_predictions_mg_l = Vec::new();
    let mut jackknife_min = vec![f64::INFINITY; x.len()];
    let mut jackknife_max = vec![0.0_f64; x.len()];
    for &i in &indices {
        let v = optimize(dose, &obs, compartments, stages, Some(i));
        let p = decode(&v, stages);
        held_out_predictions_mg_l.push(p.concentration(dose, obs[i].time_min));
        for (j, value) in v.iter().map(|v| v.exp()).enumerate() {
            jackknife_min[j] = jackknife_min[j].min(value);
            jackknife_max[j] = jackknife_max[j].max(value);
        }
    }
    let leave_one_time_out = metrics(
        &indices
            .iter()
            .zip(&held_out_predictions_mg_l)
            .map(|(i, p)| (obs[*i].concentration_mg_l, *p))
            .collect::<Vec<_>>(),
    );
    let mut warnings = Vec::new();
    if x.iter()
        .zip(bounds(compartments))
        .any(|(v, (l, h))| (v - l).abs() < 0.01 || (v - h).abs() < 0.01)
    {
        warnings.push("Параметр у технической границы: идентифицируемость ограничена".into());
    }
    if jackknife_min
        .iter()
        .zip(&jackknife_max)
        .any(|(l, h)| h / l > 10.0)
    {
        warnings.push("Параметры меняются более чем в 10 раз при исключении точки; это не надёжная физиологическая оценка".into());
    }
    if leave_one_time_out.mean_absolute_relative_error_pct > 30.0 {
        warnings.push(
            "Ошибка на исключаемых точках >30%; перенос кривой требует дополнительных измерений"
                .into(),
        );
    }
    CurveFitCandidate {
        compartments,
        parameters,
        training,
        leave_one_time_out,
        held_out_predictions_mg_l,
        jackknife_min,
        jackknife_max,
        warnings,
    }
}
pub fn fit_pk_curves(dataset: &PkCurveDataset) -> Result<PkCurveFitReport, Error> {
    dataset.validate()?;
    let mut fits = Vec::new();
    for curve in &dataset.curves {
        let obs = curve.observations();
        require(
            obs.iter()
                .filter(|o| o.exclude.is_none() && o.upper_limit_mg_l.is_none())
                .count()
                >= 6,
            "Недостаточно количественных точек для подбора",
        )?;
        let mut candidates = Vec::new();
        for compartments in [1, 2] {
            if compartments == 2
                && obs
                    .iter()
                    .filter(|o| o.exclude.is_none() && o.upper_limit_mg_l.is_none())
                    .count()
                    < 8
            {
                continue;
            }
            for stages in [1, 2, 3, 4] {
                candidates.push(candidate(curve, compartments, stages));
            }
        }
        let selected = candidates
            .iter()
            .min_by(|a, b| {
                a.leave_one_time_out
                    .log_rmse
                    .total_cmp(&b.leave_one_time_out.log_rmse)
                    .then_with(|| {
                        (a.compartments, a.parameters.absorption_stages)
                            .cmp(&(b.compartments, b.parameters.absorption_stages))
                    })
            })
            .expect("nonempty candidate set")
            .clone();
        let dose = curve.doses_mg[curve.substance.index()];
        let predictions = obs
            .iter()
            .map(|o| {
                let predicted = selected.parameters.concentration(dose, o.time_min);
                CurvePrediction {
                    observation: o.clone(),
                    predicted_mg_l: predicted,
                    relative_error_pct: 100.0 * (predicted / o.concentration_mg_l - 1.0),
                }
            })
            .collect();
        let horizon = obs.last().unwrap().time_min;
        let (fitted_tmax_min, fitted_cmax_mg_l) = (0..=(horizon / 0.5).ceil() as usize)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, selected.parameters.concentration(dose, t))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let sampled_cmax_mg_l = obs
            .iter()
            .filter(|o| o.exclude.is_none() && o.upper_limit_mg_l.is_none())
            .map(|o| o.concentration_mg_l)
            .fold(0.0_f64, f64::max);
        fits.push(PkCurveFit {
            curve: curve.clone(),
            selected,
            candidates,
            predictions,
            sampled_cmax_mg_l,
            fitted_cmax_mg_l,
            fitted_tmax_min,
        });
    }
    Ok(PkCurveFitReport{model_id:PK_FIT_MODEL_ID.into(),model_revision:1,dataset_id:dataset.id.clone(),data_kind:"digitized_published_graphs_not_original_records".into(),method:"Deterministic bounded multistart Nelder-Mead; log interval residuals; pixel uncertainty and a 0.10 log modelling floor; minimum leave-one-time-out log RMSE among 1/2 compartments and 1..4 absorption stages; complexity breaks exact ties".into(),limitations:dataset.limitations.clone(),scope_gaps:dataset.gaps.clone(),fits})
}
impl PkCurveFitReport {
    pub fn csv(&self) -> String {
        let mut out="curve_id,substance,time_min,observed_mg_l,predicted_mg_l,relative_error_pct,time_error_min,log_digitization_error,excluded,upper_limit_mg_l\n".to_owned();
        for fit in &self.fits {
            for p in &fit.predictions {
                let o = &p.observation;
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}\n",
                    fit.curve.id,
                    fit.curve.substance.key(),
                    o.time_min,
                    o.concentration_mg_l,
                    p.predicted_mg_l,
                    p.relative_error_pct,
                    o.time_error_min,
                    o.log_concentration_error,
                    o.exclude.is_some(),
                    o.upper_limit_mg_l.map_or(String::new(), |x| x.to_string())
                ));
            }
        }
        out
    }
}

/// A study condition is transferred as a whole, never as independently mixed coefficients.
pub fn fitted_pk_protocols(
    report: &PkCurveFitReport,
    atlas: &BrainAtlas,
) -> Result<Vec<CirculationScenario>, Error> {
    require(
        report.model_id == PK_FIT_MODEL_ID && report.model_revision == 1,
        "Неизвестная версия подбора PK",
    )?;
    PkCurveDataset {
        format_version: 1,
        id: report.dataset_id.clone(),
        species: "Homo sapiens".into(),
        matrix: "plasma".into(),
        limitations: report.limitations.clone(),
        curves: report.fits.iter().map(|f| f.curve.clone()).collect(),
        gaps: report.scope_gaps.clone(),
    }
    .validate()?;
    let mut out = Vec::new();
    for first in &report.fits {
        if out
            .iter()
            .any(|s: &CirculationScenario| s.calibration_curve_ids.contains(&first.curve.id))
        {
            continue;
        }
        let group: Vec<_> = report
            .fits
            .iter()
            .filter(|f| {
                f.curve.source_url == first.curve.source_url
                    && f.curve.doses_mg == first.curve.doses_mg
            })
            .collect();
        if Purine::ALL.iter().any(|p| {
            first.curve.doses_mg[p.index()] > 0.0
                && group.iter().filter(|f| f.curve.substance == *p).count() != 1
        }) {
            continue;
        }
        let mask = Purine::ALL.iter().fold(0, |mask, p| {
            mask | if first.curve.doses_mg[p.index()] > 0.0 {
                1 << p.index()
            } else {
                0
            }
        });
        let mut s = CirculationScenario::reference(atlas, mask);
        s.name = format!(
            "{}: C/M/T {:.0}/{:.0}/{:.0} мг — оцифрованная кривая",
            first.curve.study,
            first.curve.doses_mg[0],
            first.curve.doses_mg[1],
            first.curve.doses_mg[2]
        );
        s.interactions = false;
        s.duration_min = 5.0;
        for dose in &mut s.doses {
            dose.amount_mg = first.curve.doses_mg[dose.substance.index()];
        }
        for fit in group {
            let p = &mut s.parameters[fit.curve.substance.index()];
            let f = &fit.selected.parameters;
            p.apparent_volume_l = f.volume_l;
            p.apparent_clearance_l_h = f.clearance_l_h;
            p.absorption_half_min = std::f64::consts::LN_2 / f.absorption_rate_h * 60.0;
            p.absorption_stages = f.absorption_stages;
            p.peripheral_volume_l = f.peripheral_volume_l;
            p.distribution_clearance_l_h = f.distribution_clearance_l_h;
            s.calibration_curve_ids.push(fit.curve.id.clone());
            s.duration_min = s
                .duration_min
                .max((fit.curve.observations().last().unwrap().time_min * 2.0).ceil() / 2.0);
        }
        s.validate(atlas)?;
        out.push(s);
    }
    Ok(out)
}

/// Only the exact complete composition, dose and common dose time can use study markers.
pub fn matching_fitted_curves<'a>(
    trace: &crate::CirculationTrace,
    report: &'a PkCurveFitReport,
) -> Vec<(&'a PkCurveFit, f64)> {
    let s = &trace.scenario;
    let doses: Vec<_> = s.doses.iter().filter(|d| d.amount_mg > 0.0).collect();
    let Some(first) = doses.first() else {
        return vec![];
    };
    if doses.iter().any(|d| d.time_min != first.time_min) {
        return vec![];
    }
    let mut amounts = [0.0; 3];
    for d in &doses {
        if amounts[d.substance.index()] != 0.0 {
            return vec![];
        }
        amounts[d.substance.index()] = d.amount_mg;
    }
    report
        .fits
        .iter()
        .filter(|f| s.calibration_curve_ids.contains(&f.curve.id) && f.curve.doses_mg == amounts)
        .map(|f| (f, first.time_min))
        .collect()
}

/// Actual closed-loop concentrations, including brain exchange, evaluated at published times.
pub fn coupled_curve_metrics(
    trace: &crate::CirculationTrace,
    fit: &PkCurveFit,
    dose_time: f64,
) -> CurveFitMetrics {
    metrics(
        &fit.curve
            .observations()
            .iter()
            .filter(|o| {
                o.exclude.is_none()
                    && o.upper_limit_mg_l.is_none()
                    && o.time_min + dose_time <= trace.scenario.duration_min
            })
            .map(|o| {
                (
                    o.concentration_mg_l,
                    trace.at(o.time_min + dose_time).systemic[fit.curve.substance.index()]
                        .central_mg_l,
                )
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitted_curve_recovers_a_known_oral_response_on_excluded_times() {
        let p = OralCurveParameters {
            volume_l: 45.0,
            clearance_l_h: 5.0,
            absorption_rate_h: 4.0,
            peripheral_volume_l: 0.0,
            distribution_clearance_l_h: 0.0,
            absorption_stages: 1,
        };
        let mut data = PkCurveDataset::bundled().unwrap();
        data.curves.truncate(1);
        let c = &mut data.curves[0];
        c.doses_mg = [0.0, 100.0, 0.0];
        c.axes = neuro_domain::DigitizationAxes {
            x0_px: 0.0,
            x1_px: 1_440_000.0,
            t0_min: 0.0,
            t1_min: 1440.0,
            y0_px: 0.0,
            y1_px: 100_000.0,
            c0_ng_ml: 10000.0,
            c1_ng_ml: 0.001,
        };
        c.points = [
            3.0, 6.0, 12.0, 20.0, 30.0, 45.0, 60.0, 120.0, 240.0, 360.0, 480.0, 720.0, 1440.0,
        ]
        .into_iter()
        .map(|t| {
            let ng = p.concentration(100.0, t) * 1000.0;
            let y = (ng / c.axes.c0_ng_ml).ln() / (c.axes.c1_ng_ml / c.axes.c0_ng_ml).ln()
                * c.axes.y1_px;
            neuro_domain::DigitizedPkPoint {
                pixel: [t * 1000.0, y],
                error_px: 0.1,
                time_min: Some(t),
                exclude: None,
                upper_limit_ng_ml: None,
            }
        })
        .collect();
        let report = fit_pk_curves(&data).unwrap();
        let selected = &report.fits[0].selected;
        assert_eq!(selected.training.observations, 13);
        assert_eq!(selected.held_out_predictions_mg_l.len(), 13);
        assert!(
            selected.leave_one_time_out.mean_absolute_relative_error_pct < 2.0,
            "{:?}",
            selected.leave_one_time_out
        );
        assert!(
            (selected.parameters.concentration(100.0, 75.0) / p.concentration(100.0, 75.0) - 1.0)
                .abs()
                < 0.01
        );
    }
    #[test]
    fn analytical_absorption_has_correct_limits_and_coincident_rates() {
        let p = OralCurveParameters {
            volume_l: 50.0,
            clearance_l_h: 5.0,
            absorption_rate_h: 0.1,
            peripheral_volume_l: 0.0,
            distribution_clearance_l_h: 0.0,
            absorption_stages: 1,
        };
        assert_eq!(p.concentration(100.0, 0.0), 0.0);
        assert!((p.concentration(100.0, 60.0) - 2.0 * 0.1 * (-0.1_f64).exp()).abs() < 1e-12);
        let p2 = OralCurveParameters {
            absorption_stages: 2,
            ..p
        };
        assert!(
            (p2.concentration(100.0, 60.0) - 2.0 * 0.1 * 0.1 * 0.5 * (-0.1_f64).exp()).abs()
                < 1e-12
        );
        for stages in [1, 2, 3, 4] {
            let equal = OralCurveParameters {
                absorption_stages: stages,
                ..p
            };
            let factorial = (1..=stages as usize).product::<usize>() as f64;
            let expected = 2.0 * 0.1_f64.powi(stages as i32) * (-0.1_f64).exp() / factorial;
            assert!((equal.concentration(100.0, 60.0) - expected).abs() < 1e-12);
            for rate in [0.1 - 1e-10, 0.1 + 1e-10] {
                let p = OralCurveParameters {
                    absorption_rate_h: rate,
                    absorption_stages: stages,
                    ..p
                };
                assert!(p.concentration(100.0, 60.0).is_finite());
            }
        }
    }
    #[test]
    fn two_compartment_formula_matches_an_independent_rk4_oracle() {
        for stages in [1, 2, 3, 4] {
            let p = OralCurveParameters {
                volume_l: 40.0,
                clearance_l_h: 6.0,
                absorption_rate_h: 3.0,
                peripheral_volume_l: 80.0,
                distribution_clearance_l_h: 12.0,
                absorption_stages: stages,
            };
            let derivative = |s: [f64; 6]| {
                let ka = p.absorption_rate_h;
                let mut ds = [0.0; 6];
                ds[0] = -ka * s[0];
                for j in 1..stages as usize {
                    ds[j] = ka * (s[j - 1] - s[j]);
                }
                let distribution = p.distribution_clearance_l_h
                    * (s[4] / p.volume_l - s[5] / p.peripheral_volume_l);
                ds[4] = ka * s[stages as usize - 1]
                    - p.clearance_l_h * s[4] / p.volume_l
                    - distribution;
                ds[5] = distribution;
                ds
            };
            let mut s = [100.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let dt = 0.0001;
            for i in 0..80000 {
                let k1 = derivative(s);
                let k2 = derivative(std::array::from_fn(|j| s[j] + dt * 0.5 * k1[j]));
                let k3 = derivative(std::array::from_fn(|j| s[j] + dt * 0.5 * k2[j]));
                let k4 = derivative(std::array::from_fn(|j| s[j] + dt * k3[j]));
                for j in 0..6 {
                    s[j] += dt / 6.0 * (k1[j] + 2.0 * k2[j] + 2.0 * k3[j] + k4[j]);
                }
                if [499, 9999, 79999].contains(&i) {
                    let t = (i + 1) as f64 * dt * 60.0;
                    assert!((s[4] / p.volume_l - p.concentration(100.0, t)).abs() < 1e-9);
                }
            }
        }
    }
}
