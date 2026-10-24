//! Versioned scientific claims and reproducible, strictly validated scenarios.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, io::Write, path::Path};
use thiserror::Error;

mod response;
pub use response::*;
mod brain;
pub use brain::*;
mod network;
pub use network::*;
mod vascular;
pub use vascular::*;
mod perfusion;
pub use perfusion::*;
mod learning;
pub use learning::*;
mod purine_neural;
pub use purine_neural::*;
mod circulation;
pub use circulation::*;
mod caffeine;
pub use caffeine::*;

pub const FORMAT_VERSION: u32 = 1;
pub const MODEL_ID: &str = "competitive_binding_equilibrium_v1";
pub const MODEL_REVISION: u32 = 1;
pub const ASSUMPTIONS: [&str; 6] = [
    "Один тип одинаковых независимых рецепторов.",
    "Один сайт связывания на рецептор; A и B связываются взаимоисключающе.",
    "Связывание обратимо; достигнуто равновесие.",
    "Свободные концентрации заданы; истощение лигандов не учитывается.",
    "Однородная среда; нет кооперативности и аллостерии.",
    "Нет интернализации и метаболизма.",
];

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),
    #[error("Ошибка файла: {0}")]
    Io(#[from] std::io::Error),
    #[error("Повреждённый JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn require(condition: bool, message: impl Into<String>) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Validation(message.into()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub a: f64,
    pub b: f64,
}

impl Parameters {
    pub fn validate(self) -> Result<(), Error> {
        for (name, value) in [("a", self.a), ("b", self.b)] {
            require(
                value.is_finite() && value >= 0.0,
                format!("{name}: нужно конечное число от 0 до f64::MAX; получено {value}"),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataKind {
    Illustrative,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotSpec {
    pub max_a: f64,
    pub samples: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub format_version: u32,
    pub model_id: String,
    pub model_revision: u32,
    pub data_kind: DataKind,
    pub name: String,
    pub parameters: Parameters,
    pub plot: PlotSpec,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            model_id: MODEL_ID.into(),
            model_revision: MODEL_REVISION,
            data_kind: DataKind::Illustrative,
            name: "Конкурентное связывание A и B".into(),
            parameters: Parameters { a: 1.0, b: 1.0 },
            plot: PlotSpec {
                max_a: 100.0,
                samples: 257,
            },
        }
    }
}

impl Scenario {
    pub fn validate(&self) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION,
            format!(
                "Неизвестная версия сценария: {} (поддерживается {FORMAT_VERSION})",
                self.format_version
            ),
        )?;
        require(
            self.model_id == MODEL_ID,
            format!("Неизвестная численная модель: {}", self.model_id),
        )?;
        require(
            self.model_revision == MODEL_REVISION,
            format!("Неизвестная ревизия модели: {}", self.model_revision),
        )?;
        require(
            !self.name.trim().is_empty() && self.name.len() <= 512,
            "Название должно содержать от 1 до 512 байт",
        )?;
        self.parameters.validate()?;
        require(
            self.plot.max_a.is_finite() && self.plot.max_a > 0.0,
            "Максимум оси a должен быть положительным конечным числом",
        )?;
        require(
            (2..=4097).contains(&self.plot.samples),
            "Число точек графика должно быть от 2 до 4097",
        )
    }
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        require(
            value.get("format_version").and_then(|v| v.as_u64()) == Some(u64::from(FORMAT_VERSION)),
            "Неизвестная или отсутствующая версия сценария; поддерживается format_version = 1",
        )?;
        let scenario: Self = serde_json::from_value(value)?;
        scenario.validate()?;
        Ok(scenario)
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_json(&read_small_file(path.as_ref())?)
    }
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.validate()?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
}

fn read_small_file(path: &Path) -> Result<String, Error> {
    use std::io::Read;
    let mut text = String::new();
    fs::File::open(path)?
        .take(1_048_577)
        .read_to_string(&mut text)?;
    require(text.len() <= 1_048_576, "Файл больше допустимого 1 МиБ")?;
    Ok(text)
}

/// Write beside the target, then atomically rename. A failed write preserves the old file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Validation(e.to_string()))?
        .as_nanos();
    let name = path
        .file_name()
        .ok_or_else(|| Error::Validation("Укажите имя файла".into()))?;
    let temp = path.with_file_name(format!(
        ".{}.{}.{stamp}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| -> Result<(), Error> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Antagonist,
    Agonist,
}
impl Relation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Antagonist => "антагонист",
            Self::Agonist => "агонист",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    AbstractVerified,
    FullTextVerified,
    Unverified,
}
impl Verification {
    pub fn label(self) -> &'static str {
        match self {
            Self::AbstractVerified => "Проверена аннотация",
            Self::FullTextVerified => "Проверен полный текст",
            Self::Unverified => "Источник не проверен",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub url: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub verification: Verification,
    pub checked_on: String,
    pub rights_note: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub subject: String,
    pub relation: Relation,
    pub object: String,
    pub explanation: String,
    pub source_ids: Vec<String>,
    pub evidence: String,
    pub context: String,
    pub species: String,
    pub limitations: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub format_version: u32,
    pub entities: Vec<Entity>,
    pub sources: Vec<Source>,
    pub claims: Vec<Claim>,
}

impl Catalog {
    pub fn bundled() -> Result<Self, Error> {
        Self::from_json(include_str!("../../../data/catalog.json"))
    }
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let catalog: Self = serde_json::from_str(text)?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn entity(&self, id: &str) -> &Entity {
        self.entities
            .iter()
            .find(|e| e.id == id)
            .expect("validated catalog reference")
    }
    pub fn validate(&self) -> Result<(), Error> {
        require(self.format_version == 1, "Неизвестная версия каталога")?;
        let entities: HashSet<_> = self.entities.iter().map(|e| e.id.as_str()).collect();
        let sources: HashSet<_> = self.sources.iter().map(|e| e.id.as_str()).collect();
        let claims: HashSet<_> = self.claims.iter().map(|e| e.id.as_str()).collect();
        require(
            !entities.is_empty() && entities.len() == self.entities.len(),
            "Пустой каталог или повторные ID сущностей",
        )?;
        require(
            sources.len() == self.sources.len() && claims.len() == self.claims.len(),
            "Повторные ID источников или утверждений",
        )?;
        for entity in &self.entities {
            require(
                !entity.id.is_empty() && !entity.name.is_empty() && !entity.kind.is_empty(),
                "Неполная сущность",
            )?;
        }
        for source in &self.sources {
            require(
                !source.id.is_empty()
                    && source.url.starts_with("https://")
                    && !source.title.is_empty()
                    && !source.checked_on.is_empty()
                    && !source.rights_note.is_empty(),
                "Неполный источник",
            )?;
        }
        for claim in &self.claims {
            require(
                entities.contains(claim.subject.as_str())
                    && entities.contains(claim.object.as_str()),
                "Неизвестный субъект или объект связи",
            )?;
            require(
                !claim.source_ids.is_empty()
                    && claim
                        .source_ids
                        .iter()
                        .all(|id| sources.contains(id.as_str())),
                "У связи отсутствует известный источник",
            )?;
            require(
                [
                    &claim.id,
                    &claim.explanation,
                    &claim.evidence,
                    &claim.context,
                    &claim.species,
                    &claim.limitations,
                ]
                .iter()
                .all(|s| !s.trim().is_empty()),
                "Неполное происхождение утверждения",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scenario_roundtrip_and_rejections() {
        let scenario = Scenario::default();
        let json = serde_json::to_string(&scenario).unwrap();
        assert_eq!(Scenario::from_json(&json).unwrap(), scenario);
        for bad in [
            "{broken",
            "{}",
            &json.replace("\"format_version\":1", "\"format_version\":99"),
            &json.replace(MODEL_ID, "unknown"),
        ] {
            assert!(Scenario::from_json(bad).is_err(), "{bad}");
        }
        let mut invalid = scenario;
        invalid.parameters.a = -1.0;
        assert!(invalid.validate().is_err());
        invalid.parameters.a = 1.0;
        invalid.plot.samples = usize::MAX;
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn catalog_references_and_provenance_are_validated() {
        let mut catalog = Catalog::bundled().unwrap();
        assert_eq!(catalog.entities.len(), 4);
        assert_eq!(catalog.claims.len(), 4);
        catalog.claims[0].source_ids.push("missing".into());
        assert!(catalog.validate().is_err());
    }
}
