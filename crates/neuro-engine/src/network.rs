//! CPU/f64 method-of-steps Heun integration on an explicit delay grid.
use neuro_domain::{BrainAtlas, Error, NetworkScenario, PopulationState, atomic_write, require};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkSample {
    pub time_ms: f64,
    pub states: Vec<PopulationState>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NetworkTrace {
    pub solver: String,
    pub scenario: NetworkScenario,
    pub elapsed_ms: f64,
    pub samples: Vec<NetworkSample>,
}
impl NetworkTrace {
    pub fn csv(&self) -> String {
        let mut csv = String::from(
            "model_id,model_revision,data_kind,atlas_id,atlas_revision,dt_ms,time_ms,region_id,e,i\n",
        );
        for sample in &self.samples {
            for (population, state) in self.scenario.populations.iter().zip(&sample.states) {
                csv.push_str(&format!(
                    "{},{},illustrative,{},{},{},{},{},{},{}\n",
                    self.scenario.model_id,
                    self.scenario.model_revision,
                    self.scenario.atlas_id,
                    self.scenario.atlas_revision,
                    self.scenario.dt_ms,
                    sample.time_ms,
                    population.region_id,
                    state.e,
                    state.i
                ));
            }
        }
        csv
    }
    pub fn export_csv(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        atomic_write(path.as_ref(), self.csv().as_bytes())
    }
}
#[derive(Clone)]
struct Edge {
    from: usize,
    to: usize,
    delay: usize,
    weight: f64,
}
#[derive(Clone)]
struct Stimulus {
    node: usize,
    start: usize,
    end: usize,
    amplitude: f64,
}
pub struct NetworkSimulation {
    scenario: NetworkScenario,
    edges: Vec<Edge>,
    stimuli: Vec<Stimulus>,
    history: Vec<Vec<PopulationState>>,
    states: Vec<PopulationState>,
    step: usize,
    samples: Vec<NetworkSample>,
}
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
impl NetworkSimulation {
    pub fn new(scenario: NetworkScenario, atlas: &BrainAtlas) -> Result<Self, Error> {
        scenario.validate(atlas)?;
        let index = |id: &str| {
            scenario
                .populations
                .iter()
                .position(|p| p.region_id == id)
                .expect("validated population")
        };
        let edges: Vec<_> = scenario
            .connections
            .iter()
            .map(|c| Edge {
                from: index(&c.from),
                to: index(&c.to),
                delay: (c.delay_ms / scenario.dt_ms).round() as usize,
                weight: if c.enabled { c.weight } else { 0.0 },
            })
            .collect();
        let stimuli = scenario
            .stimuli
            .iter()
            .map(|s| Stimulus {
                node: index(&s.region_id),
                start: (s.start_ms / scenario.dt_ms).round() as usize,
                end: ((s.start_ms + s.duration_ms) / scenario.dt_ms).round() as usize,
                amplitude: s.amplitude,
            })
            .collect();
        let states: Vec<_> = scenario.populations.iter().map(|p| p.initial).collect();
        let capacity = edges.iter().map(|e| e.delay).max().unwrap_or(1) + 2;
        let samples = vec![NetworkSample {
            time_ms: 0.0,
            states: states.clone(),
        }];
        Ok(Self {
            scenario,
            edges,
            stimuli,
            history: vec![states.clone(); capacity],
            states,
            step: 0,
            samples,
        })
    }
    pub fn scenario(&self) -> &NetworkScenario {
        &self.scenario
    }
    pub fn states(&self) -> &[PopulationState] {
        &self.states
    }
    pub fn samples(&self) -> &[NetworkSample] {
        &self.samples
    }
    pub fn time_ms(&self) -> f64 {
        self.step as f64 * self.scenario.dt_ms
    }
    pub fn finished(&self) -> bool {
        self.step >= self.total_steps()
    }
    pub fn total_steps(&self) -> usize {
        (self.scenario.duration_ms / self.scenario.dt_ms).round() as usize
    }
    fn delayed(&self, edge: &Edge, step: usize) -> f64 {
        if step < edge.delay {
            self.scenario.populations[edge.from].initial.e
        } else {
            self.history[(step - edge.delay) % self.history.len()][edge.from].e
        }
    }
    /// Signed, delayed E→E inputs in the exact order of the saved connections.
    pub fn edge_inputs(&self) -> Vec<f64> {
        self.edges
            .iter()
            .map(|e| e.weight * self.delayed(e, self.step))
            .collect()
    }
    fn coupling(&self, step: usize) -> Vec<f64> {
        let mut result = vec![0.0; self.states.len()];
        for edge in &self.edges {
            result[edge.to] += edge.weight * self.delayed(edge, step);
        }
        result
    }
    fn derivatives(
        &self,
        states: &[PopulationState],
        coupling: &[f64],
        stimulus: &[f64],
    ) -> Vec<PopulationState> {
        self.scenario
            .populations
            .iter()
            .zip(states)
            .enumerate()
            .map(|(n, (p, s))| PopulationState {
                e: (-s.e
                    + sigmoid(
                        p.w_ee * s.e - p.w_ei * s.i - p.threshold_e + coupling[n] + stimulus[n],
                    ))
                    / p.tau_e_ms,
                i: (-s.i + sigmoid(p.w_ie * s.e - p.w_ii * s.i - p.threshold_i)) / p.tau_i_ms,
            })
            .collect()
    }
    pub fn advance_steps(&mut self, count: usize) -> Result<(), Error> {
        let dt = self.scenario.dt_ms;
        let stride = (self.scenario.sample_ms / dt).round() as usize;
        for _ in 0..count.min(self.total_steps() - self.step) {
            let mut stimulus = vec![0.0; self.states.len()];
            for s in &self.stimuli {
                if (s.start..s.end).contains(&self.step) {
                    stimulus[s.node] += s.amplitude;
                }
            }
            let k1 = self.derivatives(&self.states, &self.coupling(self.step), &stimulus);
            let predicted: Vec<_> = self
                .states
                .iter()
                .zip(&k1)
                .map(|(s, k)| PopulationState {
                    e: s.e + dt * k.e,
                    i: s.i + dt * k.i,
                })
                .collect();
            // Delays >= dt: end-of-step delayed values already exist in history.
            // Grid-aligned rectangular input is constant over this interval,
            // including the left limit at its endpoint (no half-step pulse leak).
            let k2 = self.derivatives(&predicted, &self.coupling(self.step + 1), &stimulus);
            let next: Vec<_> = self
                .states
                .iter()
                .zip(k1.iter().zip(&k2))
                .map(|(s, (a, b))| PopulationState {
                    e: s.e + dt * 0.5 * (a.e + b.e),
                    i: s.i + dt * 0.5 * (a.i + b.i),
                })
                .collect();
            require(
                next.iter().all(|s| {
                    [s.e, s.i]
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                }),
                "Интегратор сети вышел из диапазона E/I; расчёт остановлен",
            )?;
            self.step += 1;
            self.states = next;
            let slot = self.step % self.history.len();
            self.history[slot].clone_from(&self.states);
            if self.step.is_multiple_of(stride) {
                self.samples.push(NetworkSample {
                    time_ms: self.time_ms(),
                    states: self.states.clone(),
                });
            }
        }
        Ok(())
    }
    pub fn trace(&self) -> NetworkTrace {
        let mut samples = self.samples.clone();
        if samples.last().is_none_or(|s| s.time_ms != self.time_ms()) {
            samples.push(NetworkSample {
                time_ms: self.time_ms(),
                states: self.states.clone(),
            });
        }
        NetworkTrace {
            solver: "heun_delay_grid_v1".into(),
            scenario: self.scenario.clone(),
            elapsed_ms: self.time_ms(),
            samples,
        }
    }
}
pub fn simulate_network(
    scenario: NetworkScenario,
    atlas: &BrainAtlas,
) -> Result<NetworkTrace, Error> {
    let mut simulation = NetworkSimulation::new(scenario, atlas)?;
    simulation.advance_steps(simulation.total_steps())?;
    Ok(simulation.trace())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn single() -> (BrainAtlas, NetworkScenario) {
        let a = BrainAtlas::bundled().unwrap();
        let mut s = NetworkScenario::bundled(&a).unwrap();
        s.populations.truncate(1);
        s.connections.clear();
        s.stimuli.clear();
        s.duration_ms = 200.0;
        s.sample_ms = 2.0;
        (a, s)
    }
    #[test]
    fn uncoupled_population_matches_exponential_oracle_and_converges() {
        let (a, mut s) = single();
        let p = &mut s.populations[0];
        p.w_ee = 0.0;
        p.w_ei = 0.0;
        p.w_ie = 0.0;
        p.w_ii = 0.0;
        let p = p.clone();
        let mut errors = Vec::new();
        for dt in [1.0, 0.5, 0.25] {
            s.dt_ms = dt;
            let trace = simulate_network(s.clone(), &a).unwrap();
            let end = trace.samples.last().unwrap().states[0];
            let exact =
                |threshold: f64, tau: f64| sigmoid(-threshold) * (1.0 - (-200.0 / tau).exp());
            errors.push((end.i - exact(p.threshold_i, p.tau_i_ms)).abs());
            assert!((end.e - exact(p.threshold_e, p.tau_e_ms)).abs() < 1e-8);
        }
        assert!(
            errors[0] / errors[1] > 3.8 && errors[1] / errors[2] > 3.8,
            "{errors:?}"
        );
        assert!(errors[2] < 2e-8);
    }
    #[test]
    fn directed_delay_is_causal_and_disconnection_removes_transfer() {
        let (a, mut s) = single();
        let mut target = s.populations[0].clone();
        target.region_id = "FMA72975".into();
        s.populations.push(target);
        s.connections.push(neuro_domain::PopulationConnection {
            from: s.populations[0].region_id.clone(),
            to: s.populations[1].region_id.clone(),
            weight: 8.0,
            delay_ms: 24.0,
            enabled: true,
        });
        s.stimuli.push(neuro_domain::PopulationStimulus {
            region_id: s.populations[0].region_id.clone(),
            start_ms: 20.0,
            duration_ms: 40.0,
            amplitude: 6.0,
        });
        let coupled = simulate_network(s.clone(), &a).unwrap();
        let mut control = s.clone();
        control.stimuli.clear();
        let control = simulate_network(control, &a).unwrap();
        for (x, y) in coupled.samples.iter().zip(&control.samples) {
            if x.time_ms <= 44.0 {
                assert_eq!(x.states[1], y.states[1]);
            }
        }
        s.connections[0].enabled = false;
        let cut = simulate_network(s.clone(), &a).unwrap();
        for (x, y) in coupled.samples.iter().zip(&cut.samples) {
            assert_eq!(x.states[0], y.states[0]);
        }
        assert!(
            coupled
                .samples
                .iter()
                .zip(&cut.samples)
                .any(|(x, y)| x.states[1].e - y.states[1].e > 0.1)
        );
        s.connections[0].enabled = true;
        s.connections[0].weight = -8.0;
        let suppressed = simulate_network(s.clone(), &a).unwrap();
        assert!(
            suppressed
                .samples
                .iter()
                .zip(&cut.samples)
                .any(|(x, y)| x.time_ms > 44.0 && x.states[1].e < y.states[1].e - 0.005)
        );
        s.connections[0].enabled = false;
        s.stimuli[0].region_id = s.populations[1].region_id.clone();
        let reversed = simulate_network(s, &a).unwrap();
        assert!(reversed.samples.iter().all(|x| x.states[0].e < 0.03));
    }
    #[test]
    fn full_network_is_bounded_and_chunk_independent() {
        let a = BrainAtlas::bundled().unwrap();
        let s = NetworkScenario::bundled(&a).unwrap();
        assert_eq!(s.data_kind, neuro_domain::DataKind::Illustrative);
        let full = simulate_network(s.clone(), &a).unwrap();
        let mut chunked = NetworkSimulation::new(s, &a).unwrap();
        while !chunked.finished() {
            chunked.advance_steps(37).unwrap();
        }
        assert_eq!(full, chunked.trace());
        assert!(
            full.samples
                .iter()
                .flat_map(|s| &s.states)
                .all(|s| s.e.is_finite()
                    && s.i.is_finite()
                    && (0.0..=1.0).contains(&s.e)
                    && (0.0..=1.0).contains(&s.i))
        );
        assert!(
            full.samples
                .iter()
                .any(|s| s.states.last().unwrap().e > 0.3),
            "pulse must reach the far branch"
        );
        assert!(full.csv().contains("illustrative"));
    }
    #[test]
    fn delayed_network_agrees_with_a_refined_time_grid() {
        let atlas = BrainAtlas::bundled().unwrap();
        let mut scenario = NetworkScenario::bundled(&atlas).unwrap();
        let coarse = simulate_network(scenario.clone(), &atlas).unwrap();
        scenario.dt_ms = 0.125;
        let fine = simulate_network(scenario, &atlas).unwrap();
        let max = coarse
            .samples
            .iter()
            .zip(&fine.samples)
            .flat_map(|(a, b)| a.states.iter().zip(&b.states))
            .flat_map(|(a, b)| [(a.e - b.e).abs(), (a.i - b.i).abs()])
            .fold(0.0_f64, f64::max);
        assert!(
            max < 0.002,
            "maximum discrepancy on common 2 ms samples: {max}"
        );
        eprintln!("network dt 0.25 vs 0.125 ms, maximum E/I discrepancy: {max}");
    }
}
