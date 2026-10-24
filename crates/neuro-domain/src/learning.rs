//! Offline educational definitions of the equations and scientific labels in NeuroLab.
use crate::{Error, require};
use serde::Deserialize;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningTerm {
    pub term: String,
    pub aliases: Vec<String>,
    pub explanation: String,
    pub context: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningCatalog {
    pub format_version: u32,
    pub language: String,
    pub terms: Vec<LearningTerm>,
}
impl LearningCatalog {
    pub fn bundled() -> Result<Self, Error> {
        let c: Self = serde_json::from_str(include_str!("../../../data/glossary.json"))?;
        require(
            c.format_version == 1 && c.language == "ru",
            "Неизвестная версия словаря",
        )?;
        let mut names = std::collections::HashSet::new();
        for t in &c.terms {
            require(
                names.insert(&t.term)
                    && !t.explanation.is_empty()
                    && !t.context.is_empty()
                    && !t.aliases.is_empty()
                    && t.aliases.iter().all(|a| !a.is_empty()),
                "Неполный/повторный термин",
            )?;
        }
        Ok(c)
    }
}
