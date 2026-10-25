use crate::{Error, Purine, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PkCurveDataset {
    pub format_version: u32,
    pub id: String,
    pub species: String,
    pub matrix: String,
    pub limitations: String,
    pub curves: Vec<DigitizedPkCurve>,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DigitizedPkCurve {
    pub id: String,
    pub study: String,
    pub substance: Purine,
    pub doses_mg: [f64; 3],
    pub n: usize,
    pub source_url: String,
    pub figure_url: String,
    pub sha256: String,
    pub verification: String,
    pub axes: DigitizationAxes,
    pub points: Vec<DigitizedPkPoint>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DigitizationAxes {
    pub x0_px: f64,
    pub x1_px: f64,
    pub t0_min: f64,
    pub t1_min: f64,
    pub y0_px: f64,
    pub y1_px: f64,
    pub c0_ng_ml: f64,
    pub c1_ng_ml: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DigitizedPkPoint {
    pub pixel: [f64; 2],
    pub error_px: f64,
    #[serde(default)]
    pub time_min: Option<f64>,
    #[serde(default)]
    pub exclude: Option<String>,
    #[serde(default)]
    pub upper_limit_ng_ml: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PkObservation {
    pub time_min: f64,
    pub concentration_mg_l: f64,
    pub time_error_min: f64,
    pub log_concentration_error: f64,
    pub exclude: Option<String>,
    pub upper_limit_mg_l: Option<f64>,
}
impl DigitizedPkCurve {
    pub fn observations(&self) -> Vec<PkObservation> {
        let a = &self.axes;
        self.points
            .iter()
            .map(|p| PkObservation {
                time_min: p.time_min.unwrap_or(
                    a.t0_min + (p.pixel[0] - a.x0_px) / (a.x1_px - a.x0_px) * (a.t1_min - a.t0_min),
                ),
                concentration_mg_l: a.c0_ng_ml
                    * ((p.pixel[1] - a.y0_px) / (a.y1_px - a.y0_px)
                        * (a.c1_ng_ml / a.c0_ng_ml).ln())
                    .exp()
                    / 1000.0,
                time_error_min: if p.time_min.is_some() {
                    0.0
                } else {
                    2.0 * (a.t1_min - a.t0_min) / (a.x1_px - a.x0_px)
                },
                log_concentration_error: p.error_px
                    * ((a.c1_ng_ml / a.c0_ng_ml).ln() / (a.y1_px - a.y0_px)).abs(),
                exclude: p.exclude.clone(),
                upper_limit_mg_l: p.upper_limit_ng_ml.map(|v| v / 1000.0),
            })
            .collect()
    }
}
impl PkCurveDataset {
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        Self::from_json(&crate::read_small_file(path.as_ref())?)
    }
    pub fn bundled() -> Result<Self, Error> {
        Self::from_json(include_str!("../../../data/purine-pk-curves.json"))
    }
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let d: Self = serde_json::from_str(text)?;
        d.validate()?;
        Ok(d)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let d = self;
        require(
            d.format_version == 1
                && d.species == "Homo sapiens"
                && d.matrix == "plasma"
                && !d.id.is_empty()
                && !d.limitations.is_empty()
                && !d.curves.is_empty()
                && d.curves.len() <= 64,
            "Неизвестный или пустой набор PK-кривых",
        )?;
        let mut ids = std::collections::HashSet::new();
        for c in &d.curves {
            require(
                ids.insert(&c.id)
                    && c.id.len() <= 128
                    && !c.id.is_empty()
                    && c.id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                    && c.n > 0
                    && !c.study.is_empty()
                    && !c.verification.is_empty()
                    && c.source_url.starts_with("https://")
                    && c.figure_url.starts_with("https://")
                    && c.sha256.len() == 64
                    && c.sha256.bytes().all(|v| v.is_ascii_hexdigit()),
                "Неполное происхождение PK-кривой",
            )?;
            require(
                c.doses_mg
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= 500.0)
                    && c.doses_mg[c.substance.index()] > 0.0
                    && (6..=512).contains(&c.points.len()),
                "Дозы или размер кривой вне пределов",
            )?;
            let a = &c.axes;
            require(
                [
                    a.x0_px, a.x1_px, a.y0_px, a.y1_px, a.t0_min, a.t1_min, a.c0_ng_ml, a.c1_ng_ml,
                ]
                .iter()
                .all(|v| v.is_finite())
                    && a.x1_px > a.x0_px
                    && a.y1_px != a.y0_px
                    && a.t1_min > a.t0_min
                    && a.t0_min >= 0.0
                    && a.c0_ng_ml > 0.0
                    && a.c1_ng_ml > 0.0
                    && a.c0_ng_ml != a.c1_ng_ml,
                "Недопустимые оси оцифровки",
            )?;
            for p in &c.points {
                require(
                    p.pixel.iter().all(|v| v.is_finite())
                        && p.error_px.is_finite()
                        && (0.1..=30.0).contains(&p.error_px)
                        && p.time_min
                            .is_none_or(|t| t.is_finite() && (0.0..=2880.0).contains(&t))
                        && p.upper_limit_ng_ml.is_none_or(|v| v.is_finite() && v > 0.0),
                    "Недопустимая точка оцифровки",
                )?;
            }
            let observations = c.observations();
            require(
                observations.iter().all(|o| {
                    o.time_min >= 0.0
                        && o.time_min <= 2880.0
                        && o.concentration_mg_l.is_finite()
                        && o.concentration_mg_l > 0.0
                }) && observations
                    .windows(2)
                    .all(|v| v[0].time_min < v[1].time_min)
                    && observations
                        .iter()
                        .filter(|o| o.exclude.is_none() && o.upper_limit_mg_l.is_none())
                        .count()
                        >= 6,
                "Временной ряд PK должен быть упорядочен и содержать достаточно точек",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn digitization_preserves_units_censoring_and_source_gaps() {
        let d = PkCurveDataset::bundled().unwrap();
        let m = &d.curves[0];
        let o = m.observations();
        assert_eq!(o[0].time_min, 15.0);
        assert_eq!(o[0].time_error_min, 0.0);
        assert_eq!(o.last().unwrap().upper_limit_mg_l, Some(0.00067));
        assert!(o.last().unwrap().concentration_mg_l < 0.00067);
        assert!(!d.gaps.is_empty());
        assert!(d.curves[2].observations()[0].exclude.is_some());
        let mut c = m.clone();
        c.points[0].pixel = [c.axes.x0_px, c.axes.y0_px];
        c.points[0].time_min = None;
        assert_eq!(c.observations()[0].concentration_mg_l, 1.0); // 1000 ng/mL = 1 mg/L
        let mut broken = serde_json::to_value(d).unwrap();
        broken["curves"][0]["axes"]["x1_px"] = broken["curves"][0]["axes"]["x0_px"].clone();
        assert!(PkCurveDataset::from_json(&broken.to_string()).is_err());
    }
}
