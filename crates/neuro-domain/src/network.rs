//! Explicit population networks. Anatomical region identities do not imply connectivity.
use crate::{
    BrainAtlas, BrainScene, DataKind, Error, FORMAT_VERSION, TissueKind, atomic_write,
    read_small_file, require,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub const NETWORK_MODEL_ID: &str = "wilson_cowan_delayed_rate_v1";
pub const NETWORK_MODEL_REVISION: u32 = 1;
pub const NETWORK_SOURCE_URL: &str =
    "https://journals.plos.org/ploscompbiol/article?id=10.1371/journal.pcbi.1008164";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationState {
    pub e: f64,
    pub i: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeuralPopulation {
    pub region_id: String,
    pub tau_e_ms: f64,
    pub tau_i_ms: f64,
    pub w_ee: f64,
    pub w_ei: f64,
    pub w_ie: f64,
    pub w_ii: f64,
    pub threshold_e: f64,
    pub threshold_i: f64,
    pub initial: PopulationState,
}
impl NeuralPopulation {
    pub fn illustrative(region_id: String) -> Self {
        Self {
            region_id,
            tau_e_ms: 12.0,
            tau_i_ms: 30.0,
            w_ee: 4.0,
            w_ei: 8.0,
            w_ie: 6.0,
            w_ii: 1.0,
            threshold_e: 4.0,
            threshold_i: 3.0,
            initial: PopulationState { e: 0.0, i: 0.0 },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationConnection {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub delay_ms: f64,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationStimulus {
    pub region_id: String,
    pub start_ms: f64,
    pub duration_ms: f64,
    pub amplitude: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkScenario {
    pub format_version: u32,
    pub document_kind: String,
    pub model_id: String,
    pub model_revision: u32,
    pub data_kind: DataKind,
    pub atlas_id: String,
    pub atlas_revision: u32,
    pub name: String,
    pub assumptions: String,
    pub dt_ms: f64,
    pub duration_ms: f64,
    pub sample_ms: f64,
    pub populations: Vec<NeuralPopulation>,
    pub connections: Vec<PopulationConnection>,
    pub stimuli: Vec<PopulationStimulus>,
}
pub fn on_time_grid(value: f64, dt: f64) -> bool {
    value.is_finite()
        && dt.is_finite()
        && dt > 0.0
        && (value / dt - (value / dt).round()).abs() < 1e-7
}
impl NetworkScenario {
    pub fn bundled(atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::from_json(include_str!("../../../scenarios/brain-network.json"), atlas)
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION
                && self.document_kind == "brain_network"
                && self.model_id == NETWORK_MODEL_ID
                && self.model_revision == NETWORK_MODEL_REVISION
                && self.atlas_id == atlas.atlas_id
                && self.atlas_revision == atlas.atlas_revision,
            "Неизвестная версия сети, модели или атласа",
        )?;
        require(
            !self.name.trim().is_empty() && !self.assumptions.trim().is_empty(),
            "Укажите название и допущения сети",
        )?;
        require(
            self.dt_ms.is_finite() && (0.1..=1.0).contains(&self.dt_ms),
            "Шаг сети: 0.1–1 мс",
        )?;
        require(
            (1.0..=5000.0).contains(&self.duration_ms)
                && on_time_grid(self.duration_ms, self.dt_ms)
                && (self.dt_ms..=50.0).contains(&self.sample_ms)
                && on_time_grid(self.sample_ms, self.dt_ms)
                && on_time_grid(self.duration_ms, self.sample_ms),
            "Время и запись должны лежать на сетке шага; длительность до 5000 мс",
        )?;
        require(
            (1..=128).contains(&self.populations.len())
                && self.connections.len() <= 1024
                && self.stimuli.len() <= 128,
            "Лимиты сети: 1–128 областей, до 1024 связей и 128 стимулов",
        )?;
        let steps = (self.duration_ms / self.dt_ms).round() as usize;
        require(
            steps.saturating_mul(self.connections.len().max(self.populations.len())) <= 20_000_000
                && (self.duration_ms / self.sample_ms + 1.0) * self.populations.len() as f64
                    <= 500_000.0,
            "Сеть превышает лимит вычислений или записи; увеличьте шаг/интервал записи",
        )?;
        let mut ids = HashSet::new();
        for p in &self.populations {
            require(
                ids.insert(p.region_id.as_str()),
                "Повторная популяция области",
            )?;
            require(
                atlas
                    .region(&p.region_id)
                    .is_some_and(|r| matches!(r.kind, TissueKind::Cortex | TissueKind::GrayMatter)),
                "Популяция должна ссылаться на кору или внутреннее серое вещество; для прочих тканей модель не задана",
            )?;
            require(
                [p.tau_e_ms, p.tau_i_ms]
                    .iter()
                    .all(|v| v.is_finite() && (2.0..=200.0).contains(v))
                    && self.dt_ms <= p.tau_e_ms.min(p.tau_i_ms) / 4.0,
                "Постоянные времени: 2–200 мс; шаг не больше tau/4",
            )?;
            require(
                [p.w_ee, p.w_ei, p.w_ie, p.w_ii, p.threshold_e, p.threshold_i]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=20.0).contains(v)),
                "Локальные веса и пороги: конечные числа 0–20",
            )?;
            require(
                [p.initial.e, p.initial.i]
                    .iter()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "Начальные E/I: 0–1",
            )?;
        }
        let mut edges = HashSet::new();
        for c in &self.connections {
            require(
                ids.contains(c.from.as_str()) && ids.contains(c.to.as_str()) && c.from != c.to,
                "Связь: нужны две разные настроенные области",
            )?;
            require(
                edges.insert((&c.from, &c.to)),
                "Повторная направленная связь",
            )?;
            require(
                c.weight.is_finite() && (-20.0..=20.0).contains(&c.weight),
                "Вес связи: −20…20",
            )?;
            require(
                (self.dt_ms..=200.0).contains(&c.delay_ms) && on_time_grid(c.delay_ms, self.dt_ms),
                "Задержка: от одного шага до 200 мс, кратна шагу",
            )?;
        }
        for s in &self.stimuli {
            require(
                ids.contains(s.region_id.as_str()),
                "Стимул: неизвестная популяция",
            )?;
            require(
                s.start_ms >= 0.0
                    && s.duration_ms >= self.dt_ms
                    && s.start_ms + s.duration_ms <= self.duration_ms
                    && on_time_grid(s.start_ms, self.dt_ms)
                    && on_time_grid(s.duration_ms, self.dt_ms)
                    && s.amplitude.is_finite()
                    && (-20.0..=20.0).contains(&s.amplitude),
                "Стимул: проверьте амплитуду, длительность и сетку времени",
            )?;
        }
        Ok(())
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let s: Self = serde_json::from_str(text)?;
        s.validate(atlas)?;
        Ok(s)
    }
    pub fn load(path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<Self, Error> {
        let text = read_small_file(path.as_ref())?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value.get("document_kind").and_then(|v| v.as_str()) == Some("brain_project") {
            Ok(BrainProject::from_json(&text, atlas)?.network)
        } else {
            Self::from_json(&text, atlas)
        }
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        self.validate(atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainProject {
    pub format_version: u32,
    pub document_kind: String,
    pub scene: BrainScene,
    pub network: NetworkScenario,
}
impl BrainProject {
    pub fn new(scene: BrainScene, network: NetworkScenario) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            document_kind: "brain_project".into(),
            scene,
            network,
        }
    }
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION && self.document_kind == "brain_project",
            "Неизвестная версия проекта мозга",
        )?;
        self.scene.validate(atlas)?;
        self.network.validate(atlas)
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let p: Self = serde_json::from_str(text)?;
        p.validate(atlas)?;
        Ok(p)
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        self.validate(atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_validation_and_legacy_project_roundtrip() {
        let atlas = BrainAtlas::bundled().unwrap();
        let s = NetworkScenario::bundled(&atlas).unwrap();
        assert_eq!(s.populations.len(), 47);
        let project = BrainProject::new(BrainScene::default(), s.clone());
        assert_eq!(
            BrainProject::from_json(&serde_json::to_string(&project).unwrap(), &atlas).unwrap(),
            project
        );
        for mut bad in [s.clone(), s.clone(), s.clone(), s.clone()]
            .into_iter()
            .enumerate()
        {
            match bad.0 {
                0 => bad.1.connections[0].to = "FMA78454".into(),
                1 => bad.1.connections[0].delay_ms = 0.1,
                2 => bad.1.populations[0].tau_e_ms = f64::NAN,
                _ => bad.1.populations[0].region_id = "FMA67944".into(),
            }
            assert!(bad.1.validate(&atlas).is_err());
        }
        let mut bad = s;
        bad.connections.push(bad.connections[0].clone());
        assert!(bad.validate(&atlas).is_err());
    }
}
