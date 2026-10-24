//! CPU/f64 binding, operational response and population-network dynamics. No graphics.
use neuro_domain::{Error, Parameters, Scenario, atomic_write};
use serde::Serialize;
use std::path::Path;

mod response;
pub use response::*;
mod perfusion;
pub use perfusion::*;
mod caffeine;
pub use caffeine::*;
mod network;
pub use network::*;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ExperimentEvaluation {
    Binding(Evaluation),
    Response(ResponseEvaluation),
}
pub fn evaluate_experiment(
    scenario: &neuro_domain::Experiment,
) -> Result<ExperimentEvaluation, Error> {
    match scenario {
        neuro_domain::Experiment::Binding(s) => evaluate(s).map(ExperimentEvaluation::Binding),
        neuro_domain::Experiment::Response(s) => {
            evaluate_response(s).map(ExperimentEvaluation::Response)
        }
    }
}
impl ExperimentEvaluation {
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        match self {
            Self::Binding(result) => result.export_csv(path),
            Self::Response(result) => result.export_csv(path),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Occupancy {
    pub p_free: f64,
    pub p_a: f64,
    pub p_b: f64,
}

pub fn calculate(parameters: Parameters) -> Result<Occupancy, Error> {
    parameters.validate()?;
    let Parameters { a, b } = parameters;
    // Scaling bounds every addend by 1, even when a + b would overflow.
    let scale = 1.0_f64.max(a).max(b);
    let free = 1.0 / scale;
    let a = a / scale;
    let b = b / scale;
    let denominator = free + a + b;
    Ok(Occupancy {
        p_free: free / denominator,
        p_a: a / denominator,
        p_b: b / denominator,
    })
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CurvePoint {
    pub a: f64,
    pub b: f64,
    pub occupancy: Occupancy,
    pub p_a_without_b: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Evaluation {
    pub scenario: Scenario,
    pub current: Occupancy,
    pub curve: Vec<CurvePoint>,
}

/// The single calculation entry point used by GUI and CLI.
pub fn evaluate(scenario: &Scenario) -> Result<Evaluation, Error> {
    scenario.validate()?;
    let current = calculate(scenario.parameters)?;
    let end = scenario.plot.max_a.ln_1p();
    let curve = (0..scenario.plot.samples)
        .map(|index| {
            let a = if index == 0 {
                0.0
            } else if index == scenario.plot.samples - 1 {
                scenario.plot.max_a
            } else {
                (end * index as f64 / (scenario.plot.samples - 1) as f64)
                    .exp_m1()
                    .min(scenario.plot.max_a)
            };
            let b = scenario.parameters.b;
            Ok(CurvePoint {
                a,
                b,
                occupancy: calculate(Parameters { a, b })?,
                p_a_without_b: calculate(Parameters { a, b: 0.0 })?.p_a,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(Evaluation {
        scenario: scenario.clone(),
        current,
        curve,
    })
}

impl Evaluation {
    /// The current point and every plotted point, with full round-trip f64 precision.
    pub fn csv(&self) -> String {
        let mut csv = String::from(
            "row_type,format_version,model_id,model_revision,data_kind,a,b,p_free,p_a,p_b,p_a_without_b\n",
        );
        let mut row = |kind: &str, a, b, p: Occupancy, reference| {
            csv.push_str(&format!(
                "{kind},{},{},{},illustrative,{a},{b},{},{},{},{reference}\n",
                self.scenario.format_version,
                self.scenario.model_id,
                self.scenario.model_revision,
                p.p_free,
                p.p_a,
                p.p_b
            ));
        };
        let p = self.scenario.parameters;
        // Already validated when constructing Evaluation.
        let reference = calculate(Parameters { a: p.a, b: 0.0 })
            .expect("validated evaluation")
            .p_a;
        row("current", p.a, p.b, self.current, reference);
        for point in &self.curve {
            row(
                "curve",
                point.a,
                point.b,
                point.occupancy,
                point.p_a_without_b,
            );
        }
        csv
    }
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        atomic_write(path.as_ref(), self.csv().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 5.0 * f64::EPSILON, "{a} != {b}");
    }
    #[test]
    fn analytical_cases() {
        assert_eq!(
            calculate(Parameters { a: 0.0, b: 0.0 }).unwrap(),
            Occupancy {
                p_free: 1.0,
                p_a: 0.0,
                p_b: 0.0
            }
        );
        let p = calculate(Parameters { a: 1.0, b: 1.0 }).unwrap();
        close(p.p_free, 1.0 / 3.0);
        close(p.p_a, 1.0 / 3.0);
        close(p.p_b, 1.0 / 3.0);
        let p = calculate(Parameters { a: 3.0, b: 6.0 }).unwrap();
        close(p.p_free, 0.1);
        close(p.p_a, 0.3);
        close(p.p_b, 0.6);
    }
    #[test]
    fn normalized_symmetric_across_f64_range() {
        let values = [
            0.0,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            1e-100,
            0.01,
            1.0,
            10.0,
            1e100,
            f64::MAX,
        ];
        for a in values {
            for b in values {
                let p = calculate(Parameters { a, b }).unwrap();
                for v in [p.p_free, p.p_a, p.p_b] {
                    assert!(v.is_finite() && (0.0..=1.0).contains(&v));
                }
                close(p.p_free + p.p_a + p.p_b, 1.0);
                let swapped = calculate(Parameters { a: b, b: a }).unwrap();
                close(p.p_free, swapped.p_free);
                close(p.p_a, swapped.p_b);
                close(p.p_b, swapped.p_a);
            }
        }
        let p = calculate(Parameters {
            a: f64::MAX,
            b: f64::MAX,
        })
        .unwrap();
        close(p.p_a, 0.5);
        close(p.p_b, 0.5);
        assert!(p.p_free > 0.0);
    }
    #[test]
    fn competition_is_monotone() {
        for a in [0.001, 1.0, 10.0, 1e100] {
            let mut previous = 1.0;
            for b in [0.0, 0.01, 1.0, 100.0, 1e100, f64::MAX] {
                let current = calculate(Parameters { a, b }).unwrap().p_a;
                assert!(current <= previous);
                previous = current;
            }
        }
        assert!(
            calculate(Parameters { a: 1.0, b: 2.0 }).unwrap().p_a
                < calculate(Parameters { a: 1.0, b: 1.0 }).unwrap().p_a
        );
    }
    #[test]
    fn rejects_invalid_input() {
        for invalid in [
            -1.0,
            -f64::MIN_POSITIVE,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert!(calculate(Parameters { a: invalid, b: 0.0 }).is_err());
            assert!(calculate(Parameters { a: 0.0, b: invalid }).is_err());
        }
    }
    #[test]
    fn extreme_curves_include_zero_and_stay_finite() {
        for max_a in [f64::from_bits(1), 100.0, f64::MAX] {
            let scenario = Scenario {
                plot: neuro_domain::PlotSpec {
                    max_a,
                    samples: 257,
                },
                ..Scenario::default()
            };
            let result = evaluate(&scenario).unwrap();
            assert_eq!(result.curve.first().unwrap().a, 0.0);
            assert_eq!(result.curve.last().unwrap().a, max_a);
            assert!(result.curve.windows(2).all(|w| w[0].a <= w[1].a));
            for p in result.curve {
                assert!(p.a.is_finite() && p.occupancy.p_a.is_finite());
            }
        }
    }
    #[test]
    fn csv_is_the_actual_calculation() {
        let evaluation = evaluate(&Scenario::default()).unwrap();
        let csv = evaluation.csv();
        assert_eq!(csv.lines().count(), 259);
        for (line, point) in csv.lines().skip(2).zip(&evaluation.curve) {
            let columns: Vec<_> = line.split(',').collect();
            assert_eq!(columns[5].parse::<f64>().unwrap(), point.a);
            assert_eq!(columns[8].parse::<f64>().unwrap(), point.occupancy.p_a);
            assert_eq!(columns[10].parse::<f64>().unwrap(), point.p_a_without_b);
        }
    }
}
