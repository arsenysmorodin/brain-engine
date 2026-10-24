use crate::calculate;
use neuro_domain::{Error, Parameters, ResponseParameters, ResponseScenario, atomic_write};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct FunctionalResponse {
    pub occupancy: f64,
    pub fractional_response: f64,
    pub response_ceiling: f64,
    /// Half of this agonist's asymptote, not half of the system maximum.
    /// Undefined for the identically zero response at tau = 0.
    pub ec50_over_ka: Option<f64>,
}

pub fn calculate_response(p: ResponseParameters) -> Result<FunctionalResponse, Error> {
    p.validate()?;
    let occupancy = calculate(Parameters { a: p.x, b: 0.0 })?.p_a;
    // E/E_m = tau * theta / (1 + tau * theta).
    // Avoid overflow in tau*x and retain tiny x at very large tau.
    let fractional_response = if p.tau <= 1.0 {
        let stimulus = p.tau * occupancy;
        stimulus / (1.0 + stimulus)
    } else {
        occupancy / (1.0 / p.tau + occupancy)
    };
    let response_ceiling = if p.tau <= 1.0 {
        p.tau / (1.0 + p.tau)
    } else {
        1.0 / (1.0 + 1.0 / p.tau)
    };
    Ok(FunctionalResponse {
        occupancy,
        fractional_response,
        response_ceiling,
        ec50_over_ka: (p.tau > 0.0).then(|| 1.0 / (1.0 + p.tau)),
    })
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ResponsePoint {
    pub x: f64,
    pub result: FunctionalResponse,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResponseEvaluation {
    pub scenario: ResponseScenario,
    pub current: FunctionalResponse,
    pub curve: Vec<ResponsePoint>,
}
pub fn evaluate_response(scenario: &ResponseScenario) -> Result<ResponseEvaluation, Error> {
    scenario.validate()?;
    let end = scenario.plot.max_x.ln_1p();
    let curve = (0..scenario.plot.samples)
        .map(|i| {
            let x = if i == 0 {
                0.0
            } else if i == scenario.plot.samples - 1 {
                scenario.plot.max_x
            } else {
                (end * i as f64 / (scenario.plot.samples - 1) as f64)
                    .exp_m1()
                    .min(scenario.plot.max_x)
            };
            Ok(ResponsePoint {
                x,
                result: calculate_response(ResponseParameters {
                    x,
                    tau: scenario.parameters.tau,
                })?,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(ResponseEvaluation {
        scenario: scenario.clone(),
        current: calculate_response(scenario.parameters)?,
        curve,
    })
}
impl ResponseEvaluation {
    pub fn csv(&self) -> String {
        let mut text = String::from(
            "row_type,format_version,model_id,model_revision,data_kind,x,tau,occupancy,fractional_response,response_ceiling,ec50_over_ka\n",
        );
        let mut row = |kind, x, r: FunctionalResponse| {
            text.push_str(&format!(
                "{kind},{},{},{},illustrative,{x},{},{},{},{},{}\n",
                self.scenario.format_version,
                self.scenario.model_id,
                self.scenario.model_revision,
                self.scenario.parameters.tau,
                r.occupancy,
                r.fractional_response,
                r.response_ceiling,
                r.ec50_over_ka.map(|v| v.to_string()).unwrap_or_default()
            ));
        };
        row("current", self.scenario.parameters.x, self.current);
        for point in &self.curve {
            row("curve", point.x, point.result);
        }
        text
    }
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        atomic_write(path.as_ref(), self.csv().as_bytes())
    }
}

#[derive(Debug, Serialize)]
pub struct ResponseBenchmark {
    pub model_id: &'static str,
    pub model_revision: u32,
    pub reference: &'static str,
    pub reference_kind: &'static str,
    pub points: usize,
    pub max_absolute_error: f64,
    pub rmse: f64,
    pub absolute_tolerance: f64,
    pub passed: bool,
    pub independent_experimental_validation: bool,
}
/// Reconstruct noise-free theoretical curves from Jakubik et al. 2019, Eq. 1 / Fig. 1.
/// Seven decade anchors within the published range; not the paper's noisy measurements/fits.
pub fn response_benchmark() -> Result<ResponseBenchmark, Error> {
    let mut errors = Vec::new();
    for tau in [0.1, 1.0, 10.0, 100.0, 1000.0] {
        for x in [0.0001, 0.001, 0.01, 0.1, 1.0, 10.0, 100.0] {
            // Direct published equation is safe on this restricted reference range.
            let reference = tau * x / (1.0 + (1.0 + tau) * x);
            let actual = calculate_response(ResponseParameters { x, tau })?.fractional_response;
            errors.push((actual - reference).abs());
        }
    }
    let max_absolute_error = errors.iter().copied().fold(0.0_f64, f64::max);
    let rmse = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    let absolute_tolerance = 5e-16;
    Ok(ResponseBenchmark {
        model_id: neuro_domain::RESPONSE_MODEL_ID,
        model_revision: neuro_domain::RESPONSE_MODEL_REVISION,
        reference: "https://pmc.ncbi.nlm.nih.gov/articles/PMC6420642/",
        reference_kind: "theoretical_reconstruction_without_noise",
        points: errors.len(),
        max_absolute_error,
        rmse,
        absolute_tolerance,
        passed: max_absolute_error <= absolute_tolerance,
        independent_experimental_validation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(x: f64, tau: f64) -> FunctionalResponse {
        calculate_response(ResponseParameters { x, tau }).unwrap()
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 5e-16, "{a} != {b}");
    }
    #[test]
    fn rational_oracle_in_published_theoretical_range() {
        // Exact rational evaluations of Eq. 1, independently reduced by hand.
        let cases = [
            (0.0001, 0.1, 1.0, 100011.0),
            (0.0001, 1.0, 1.0, 10002.0),
            (0.0001, 10.0, 10.0, 10011.0),
            (0.0001, 100.0, 100.0, 10101.0),
            (0.0001, 1000.0, 1000.0, 11001.0),
            (1.0, 0.1, 1.0, 21.0),
            (1.0, 1.0, 1.0, 3.0),
            (1.0, 10.0, 5.0, 6.0),
            (1.0, 100.0, 50.0, 51.0),
            (1.0, 1000.0, 500.0, 501.0),
            (100.0, 0.1, 10.0, 111.0),
            (100.0, 1.0, 100.0, 201.0),
            (100.0, 10.0, 1000.0, 1101.0),
            (100.0, 100.0, 10000.0, 10101.0),
            (100.0, 1000.0, 100000.0, 100101.0),
        ];
        for (x, tau, n, d) in cases {
            close(at(x, tau).fractional_response, n / d);
        }
        let benchmark = response_benchmark().unwrap();
        assert!(
            benchmark.passed
                && benchmark.points == 35
                && !benchmark.independent_experimental_validation
        );
    }
    #[test]
    fn zero_half_maximum_and_transduction_are_distinct_from_binding() {
        assert_eq!(at(1.0, 0.0).ec50_over_ka, None);
        for tau in [0.0, 0.1, 1.0, 10.0, 1e100, f64::MAX] {
            assert_eq!(at(0.0, tau).fractional_response, 0.0);
            let r = at(1.0, tau);
            close(r.occupancy, 0.5);
            if let Some(ec50) = r.ec50_over_ka {
                close(at(ec50, tau).fractional_response, r.response_ceiling / 2.0);
            }
        }
        close(at(1.0, 10.0).fractional_response, 5.0 / 6.0);
        close(at(1.0, 0.1).fractional_response, 1.0 / 21.0);
    }
    #[test]
    fn extreme_inputs_remain_finite_bounded_and_monotone() {
        let values = [
            0.0,
            f64::from_bits(1),
            1e-300,
            1e-100,
            0.1,
            1.0,
            10.0,
            1e100,
            1e300,
            f64::MAX,
        ];
        for tau in values {
            let mut previous = 0.0;
            for x in values {
                let r = at(x, tau);
                for v in [r.occupancy, r.fractional_response, r.response_ceiling] {
                    assert!(v.is_finite() && (0.0..=1.0).contains(&v));
                }
                assert!(r.fractional_response + 5e-16 >= previous);
                assert!(r.fractional_response <= r.response_ceiling + 5e-16);
                previous = r.fractional_response;
            }
        }
        // A naive tau*x overflows for MAX, but underflow of theta/tau is also avoided.
        let tiny = at(f64::from_bits(1), f64::MAX).fractional_response;
        assert!(tiny > 8e-16 && tiny < 1e-15);
        for bad in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(calculate_response(ResponseParameters { x: bad, tau: 1.0 }).is_err());
            assert!(calculate_response(ResponseParameters { x: 1.0, tau: bad }).is_err());
        }
    }
    #[test]
    fn curve_endpoints_and_csv_preserve_the_shared_results() {
        for max_x in [f64::from_bits(1), 100.0, f64::MAX] {
            let s = ResponseScenario {
                plot: neuro_domain::ResponsePlot {
                    max_x,
                    samples: 257,
                },
                ..ResponseScenario::default()
            };
            let r = evaluate_response(&s).unwrap();
            assert_eq!(r.curve.first().unwrap().x, 0.0);
            assert_eq!(r.curve.last().unwrap().x, max_x);
            let csv = r.csv();
            assert_eq!(csv.lines().count(), 259);
            for (line, p) in csv.lines().skip(2).zip(&r.curve) {
                let columns: Vec<_> = line.split(',').collect();
                assert_eq!(columns[5].parse::<f64>().unwrap(), p.x);
                assert_eq!(
                    columns[8].parse::<f64>().unwrap(),
                    p.result.fractional_response
                );
            }
        }
        let s = ResponseScenario {
            parameters: ResponseParameters { x: 1.0, tau: 0.0 },
            ..ResponseScenario::default()
        };
        assert!(
            evaluate_response(&s)
                .unwrap()
                .csv()
                .lines()
                .skip(1)
                .all(|line| line.ends_with(','))
        );
    }
}
