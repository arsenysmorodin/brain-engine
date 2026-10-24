//! BodyParts3D arterial anatomy, kept independent of perfusion parameters.
use crate::{
    BrainGeometry, BrainMeshInfo, BrainSource, Error, FORMAT_VERSION, Hemisphere, require,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainVessel {
    pub id: String,
    pub name: String,
    pub name_en: String,
    pub hemisphere: Hemisphere,
    pub supply: String,
    pub mesh_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VascularAtlas {
    pub format_version: u32,
    pub atlas_id: String,
    pub atlas_revision: u32,
    pub source: BrainSource,
    pub vessels: Vec<BrainVessel>,
    pub meshes: Vec<BrainMeshInfo>,
    pub total_vertices: usize,
    pub total_triangles: usize,
}
impl VascularAtlas {
    pub fn bundled() -> Result<Self, Error> {
        let atlas: Self = serde_json::from_str(include_str!("../../../data/vascular-atlas.json"))?;
        atlas.validate()?;
        Ok(atlas)
    }
    pub fn validate(&self) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION
                && self.atlas_id == "bodyparts3d_cerebral_arteries_4.0"
                && self.atlas_revision == 1,
            "Неизвестный сосудистый атлас",
        )?;
        require(
            self.source.id == "bodyparts3d-4.0"
                && self.source.license == "CC BY 4.0"
                && self.source.url.starts_with("https://")
                && !self.source.verification.is_empty(),
            "Неполное происхождение сосудов",
        )?;
        let ids: HashSet<_> = self.vessels.iter().map(|r| &r.id).collect();
        let meshes: HashSet<_> = self.meshes.iter().map(|m| &m.id).collect();
        require(
            !ids.is_empty() && ids.len() == self.vessels.len() && meshes.len() == self.meshes.len(),
            "Повторные сосуды или меши",
        )?;
        let mut assigned = HashSet::new();
        for r in &self.vessels {
            require(
                r.id.starts_with("FMA")
                    && !r.name.is_empty()
                    && !r.name_en.is_empty()
                    && !r.mesh_ids.is_empty()
                    && [
                        "ica_left",
                        "ica_right",
                        "aca_left",
                        "aca_right",
                        "mca_left",
                        "mca_right",
                        "pca_left",
                        "pca_right",
                        "vertebrobasilar",
                        "communicating",
                    ]
                    .contains(&r.supply.as_str()),
                "Неполная артериальная структура",
            )?;
            for id in &r.mesh_ids {
                require(
                    meshes.contains(id) && assigned.insert(id),
                    "Повторно назначенная или отсутствующая геометрия сосуда",
                )?;
            }
        }
        require(
            assigned == meshes
                && self.total_vertices == self.meshes.iter().map(|m| m.vertices).sum::<usize>()
                && self.total_triangles == self.meshes.iter().map(|m| m.triangles).sum::<usize>(),
            "Некорректный состав сосудистого атласа",
        )
    }
    pub fn geometry(&self) -> Result<BrainGeometry, Error> {
        self.validate()?;
        BrainGeometry::decode_meshes(
            include_bytes!("../../../assets/brain/arteries.bin"),
            &self.meshes,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arterial_inventory_matches_official_geometry_and_rejects_corruption() {
        let a = VascularAtlas::bundled().unwrap();
        let g = a.geometry().unwrap();
        assert_eq!(g.meshes.len(), a.meshes.len());
        assert_eq!(a.vessels.len(), 23);
        let mut bytes = include_bytes!("../../../assets/brain/arteries.bin").to_vec();
        bytes.pop();
        assert!(BrainGeometry::decode_meshes(&bytes, &a.meshes).is_err());
        let mut bad = a.clone();
        bad.vessels[0].mesh_ids = bad.vessels[1].mesh_ids.clone();
        assert!(bad.validate().is_err());
    }
}
