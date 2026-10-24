use crate::{
    DataKind, Error, FORMAT_VERSION, Scenario, Source, atomic_write, read_small_file, require,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const RESPONSE_MODEL_ID: &str = "operational_agonism_n1_v1";
pub const RESPONSE_MODEL_REVISION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseParameters {
    /// Free agonist concentration divided by K_A, not an administered dose.
    pub x: f64,
    /// System-dependent transduction ratio R_T / K_E.
    pub tau: f64,
}
impl ResponseParameters {
    pub fn validate(self) -> Result<(), Error> {
        for (name, value) in [("x", self.x), ("τ", self.tau)] {
            require(
                value.is_finite() && value >= 0.0,
                format!("{name}: нужно конечное число от 0 до f64::MAX; получено {value}"),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsePlot {
    pub max_x: f64,
    pub samples: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseScenario {
    pub format_version: u32,
    pub model_id: String,
    pub model_revision: u32,
    pub data_kind: DataKind,
    pub name: String,
    pub parameters: ResponseParameters,
    pub plot: ResponsePlot,
}
impl Default for ResponseScenario {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            model_id: RESPONSE_MODEL_ID.into(),
            model_revision: RESPONSE_MODEL_REVISION,
            data_kind: DataKind::Illustrative,
            name: "Функциональный ответ · Black–Leff, n = 1".into(),
            parameters: ResponseParameters { x: 1.0, tau: 10.0 },
            plot: ResponsePlot {
                max_x: 100.0,
                samples: 257,
            },
        }
    }
}
impl ResponseScenario {
    pub fn validate(&self) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION,
            "Неизвестная версия сценария",
        )?;
        require(
            self.model_id == RESPONSE_MODEL_ID,
            "Неизвестная модель функционального ответа",
        )?;
        require(
            self.model_revision == RESPONSE_MODEL_REVISION,
            "Неизвестная ревизия модели ответа",
        )?;
        require(
            !self.name.trim().is_empty() && self.name.len() <= 512,
            "Название должно содержать от 1 до 512 байт",
        )?;
        self.parameters.validate()?;
        require(
            self.plot.max_x.is_finite() && self.plot.max_x > 0.0,
            "Максимум оси x должен быть положительным конечным числом",
        )?;
        require(
            (2..=4097).contains(&self.plot.samples),
            "Число точек графика должно быть от 2 до 4097",
        )
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.validate()?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}

/// Dispatch by explicit model ID. Existing version-1 binding JSON is unchanged.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Experiment {
    Binding(Scenario),
    Response(ResponseScenario),
}
impl Experiment {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        require(
            value.get("format_version").and_then(|v| v.as_u64()) == Some(u64::from(FORMAT_VERSION)),
            "Неизвестная или отсутствующая версия сценария; поддерживается format_version = 1",
        )?;
        match value.get("model_id").and_then(|v| v.as_str()) {
            Some(crate::MODEL_ID) => {
                let scenario: Scenario = serde_json::from_value(value)?;
                scenario.validate()?;
                Ok(Self::Binding(scenario))
            }
            Some(RESPONSE_MODEL_ID) => {
                let scenario: ResponseScenario = serde_json::from_value(value)?;
                scenario.validate()?;
                Ok(Self::Response(scenario))
            }
            id => Err(Error::Validation(format!(
                "Неизвестная численная модель: {id:?}"
            ))),
        }
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_json(&read_small_file(path.as_ref())?)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseModelInfo {
    pub model_id: String,
    pub model_revision: u32,
    pub title: String,
    pub equations: Vec<String>,
    pub context: String,
    pub species: String,
    pub assumptions: Vec<String>,
    pub limitations: String,
    pub validation_status: String,
    pub source: Source,
}
impl ResponseModelInfo {
    pub fn bundled() -> Result<Self, Error> {
        let info: Self =
            serde_json::from_str(include_str!("../../../data/operational-model.json"))?;
        require(
            info.model_id == RESPONSE_MODEL_ID && info.model_revision == RESPONSE_MODEL_REVISION,
            "Метаданные не соответствуют модели ответа",
        )?;
        require(
            !info.equations.is_empty()
                && !info.assumptions.is_empty()
                && [
                    &info.title,
                    &info.context,
                    &info.species,
                    &info.limitations,
                    &info.validation_status,
                    &info.source.title,
                    &info.source.checked_on,
                    &info.source.rights_note,
                ]
                .iter()
                .all(|s| !s.trim().is_empty())
                && info.source.url.starts_with("https://"),
            "Неполное происхождение модели ответа",
        )?;
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatch_preserves_legacy_and_rejects_mismatched_parameters() {
        for experiment in [
            Experiment::Binding(Scenario::default()),
            Experiment::Response(ResponseScenario::default()),
        ] {
            let json = serde_json::to_string(&experiment).unwrap();
            assert_eq!(Experiment::from_json(&json).unwrap(), experiment);
            assert!(
                Experiment::from_json(
                    &json.replace("\"model_revision\":1", "\"model_revision\":99")
                )
                .is_err()
            );
        }
        let json = serde_json::to_string(&ResponseScenario::default()).unwrap();
        for bad in [
            json.replace(RESPONSE_MODEL_ID, crate::MODEL_ID),
            json.replace("\"tau\":10.0", "\"tau\":-1.0"),
            json.replace("\"max_x\":100.0", "\"max_x\":0.0"),
            json.replace("\"samples\":257", "\"samples\":1"),
        ] {
            assert!(Experiment::from_json(&bad).is_err(), "{bad}");
        }
        assert!(ResponseModelInfo::bundled().is_ok());
    }
}
