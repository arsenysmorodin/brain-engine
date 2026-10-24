//! Anatomical identities and geometry. No physiological state is inferred from a mesh.
use crate::{Error, FORMAT_VERSION, atomic_write, read_small_file, require};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub const BRAIN_ATLAS_ID: &str = "bodyparts3d_brain_4.0";
pub const BRAIN_ATLAS_REVISION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hemisphere {
    Left,
    Right,
    Bilateral,
}
impl Hemisphere {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Левая сторона",
            Self::Right => "Правая сторона",
            Self::Bilateral => "Общая / двусторонняя структура",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrainGroup {
    Frontal,
    Parietal,
    Temporal,
    Occipital,
    Insula,
    Medial,
    Deep,
    Cerebellum,
    Brainstem,
    WhiteMatter,
    Ventricles,
}
impl BrainGroup {
    pub const ALL: [Self; 11] = [
        Self::Frontal,
        Self::Parietal,
        Self::Temporal,
        Self::Occipital,
        Self::Insula,
        Self::Medial,
        Self::Deep,
        Self::Cerebellum,
        Self::Brainstem,
        Self::WhiteMatter,
        Self::Ventricles,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Frontal => "Лобная кора",
            Self::Parietal => "Теменная кора",
            Self::Temporal => "Височная кора",
            Self::Occipital => "Затылочная кора",
            Self::Insula => "Островок",
            Self::Medial => "Медиальные структуры",
            Self::Deep => "Глубокие структуры",
            Self::Cerebellum => "Мозжечок",
            Self::Brainstem => "Ствол мозга",
            Self::WhiteMatter => "Белое вещество",
            Self::Ventricles => "Желудочки",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TissueKind {
    Cortex,
    GrayMatter,
    Brainstem,
    Cerebellum,
    WhiteMatter,
    Cavity,
}
impl TissueKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cortex => "Кора",
            Self::GrayMatter => "Внутреннее серое вещество",
            Self::Brainstem => "Отдел ствола мозга",
            Self::Cerebellum => "Мозжечок",
            Self::WhiteMatter => "Белое вещество",
            Self::Cavity => "Полость",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainRegion {
    pub id: String,
    pub name: String,
    pub name_en: String,
    pub group: BrainGroup,
    pub kind: TissueKind,
    pub hemisphere: Hemisphere,
    pub mesh_ids: Vec<String>,
    pub source_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainSource {
    pub id: String,
    pub title: String,
    pub url: String,
    pub archive_url: String,
    pub license_url: String,
    pub license: String,
    pub attribution: String,
    pub publication_doi: String,
    pub checked_on: String,
    pub verification: String,
    pub changes: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainMeshInfo {
    pub id: String,
    pub archive_member: String,
    pub source_crc32: String,
    pub vertices: usize,
    pub triangles: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainAtlas {
    pub format_version: u32,
    pub atlas_id: String,
    pub atlas_revision: u32,
    pub title: String,
    pub species: String,
    pub coordinate_space: String,
    pub limitations: String,
    pub source: BrainSource,
    pub regions: Vec<BrainRegion>,
    pub meshes: Vec<BrainMeshInfo>,
    pub total_vertices: usize,
    pub total_triangles: usize,
}
impl BrainAtlas {
    pub fn bundled() -> Result<Self, Error> {
        Self::from_json(include_str!("../../../data/brain-atlas.json"))
    }
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let atlas: Self = serde_json::from_str(text)?;
        atlas.validate()?;
        Ok(atlas)
    }
    pub fn region(&self, id: &str) -> Option<&BrainRegion> {
        self.regions.iter().find(|r| r.id == id)
    }
    pub fn validate(&self) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION
                && self.atlas_id == BRAIN_ATLAS_ID
                && self.atlas_revision == BRAIN_ATLAS_REVISION,
            "Неизвестная версия анатомического атласа",
        )?;
        require(
            [
                &self.title,
                &self.species,
                &self.coordinate_space,
                &self.limitations,
                &self.source.title,
                &self.source.attribution,
                &self.source.license,
                &self.source.checked_on,
                &self.source.changes,
            ]
            .iter()
            .all(|s| !s.trim().is_empty())
                && [
                    &self.source.url,
                    &self.source.archive_url,
                    &self.source.license_url,
                ]
                .iter()
                .all(|s| s.starts_with("https://")),
            "Неполное происхождение анатомических данных",
        )?;
        let ids: HashSet<_> = self.regions.iter().map(|r| r.id.as_str()).collect();
        let meshes: HashSet<_> = self.meshes.iter().map(|m| m.id.as_str()).collect();
        require(
            !ids.is_empty() && ids.len() == self.regions.len() && meshes.len() == self.meshes.len(),
            "Повторные ID анатомических структур",
        )?;
        let mut assigned = HashSet::new();
        for region in &self.regions {
            require(
                region.id.starts_with("FMA")
                    && !region.name.trim().is_empty()
                    && !region.name_en.trim().is_empty()
                    && region.source_id == self.source.id
                    && !region.mesh_ids.is_empty(),
                "Неполная анатомическая структура",
            )?;
            for mesh in &region.mesh_ids {
                require(
                    meshes.contains(mesh.as_str()) && assigned.insert(mesh.as_str()),
                    "Отсутствующая или повторно назначенная геометрия",
                )?;
            }
        }
        require(assigned == meshes, "Геометрия без анатомической структуры")?;
        for mesh in &self.meshes {
            require(
                mesh.vertices > 2
                    && mesh.triangles > 0
                    && mesh.source_crc32.len() == 8
                    && mesh.source_crc32.chars().all(|c| c.is_ascii_hexdigit()),
                "Неполные метаданные геометрии",
            )?;
        }
        require(
            self.total_vertices == self.meshes.iter().map(|m| m.vertices).sum::<usize>()
                && self.total_triangles == self.meshes.iter().map(|m| m.triangles).sum::<usize>(),
            "Неверные итоговые размеры геометрии",
        )
    }
    /// An inventory, not simulated activity. Unknown activity is explicitly marked.
    pub fn csv(&self) -> String {
        let mut text = String::from(
            "region_id,name_ru,name_en,group,hemisphere,kind,meshes,vertices,triangles,activity_status\n",
        );
        for r in &self.regions {
            let infos: Vec<_> = self
                .meshes
                .iter()
                .filter(|m| r.mesh_ids.contains(&m.id))
                .collect();
            let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            text.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},not_configured\n",
                quote(&r.id),
                quote(&r.name),
                quote(&r.name_en),
                quote(r.group.label()),
                quote(r.hemisphere.label()),
                quote(r.kind.label()),
                r.mesh_ids.len(),
                infos.iter().map(|m| m.vertices).sum::<usize>(),
                infos.iter().map(|m| m.triangles).sum::<usize>()
            ));
        }
        text
    }
}

#[derive(Clone, Debug)]
pub struct AnatomicalMesh {
    pub id: String,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}
#[derive(Clone, Debug)]
pub struct BrainGeometry {
    pub meshes: Vec<AnatomicalMesh>,
}
impl BrainGeometry {
    pub fn bundled(atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::decode(
            include_bytes!("../../../assets/brain/bodyparts3d.bin"),
            atlas,
        )
    }
    pub fn decode(bytes: &[u8], atlas: &BrainAtlas) -> Result<Self, Error> {
        fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
            require(bytes.len() >= n, "Повреждённая геометрия: файл обрезан")?;
            let (head, tail) = bytes.split_at(n);
            *bytes = tail;
            Ok(head)
        }
        fn integer(bytes: &mut &[u8]) -> Result<u32, Error> {
            Ok(u32::from_le_bytes(
                take(bytes, 4)?.try_into().expect("four bytes"),
            ))
        }
        let mut bytes = bytes;
        require(
            take(&mut bytes, 8)? == b"NBRN0001",
            "Неизвестный формат геометрии",
        )?;
        let count = integer(&mut bytes)? as usize;
        require(
            count == atlas.meshes.len(),
            "Геометрия не соответствует атласу",
        )?;
        let mut meshes = Vec::with_capacity(count);
        let mut ids = HashSet::new();
        for _ in 0..count {
            let len = integer(&mut bytes)? as usize;
            require(len <= 64, "Некорректный ID геометрии")?;
            let id = std::str::from_utf8(take(&mut bytes, len)?)
                .map_err(|_| Error::Validation("Некорректный ID геометрии".into()))?
                .to_string();
            require(ids.insert(id.clone()), "Повторный ID геометрии")?;
            let info = atlas
                .meshes
                .iter()
                .find(|m| m.id == id)
                .ok_or_else(|| Error::Validation("Неизвестный ID геометрии".into()))?;
            let vertices = integer(&mut bytes)? as usize;
            let indices = integer(&mut bytes)? as usize;
            require(
                vertices == info.vertices && indices == info.triangles * 3,
                "Размер геометрии не соответствует атласу",
            )?;
            let positions = take(&mut bytes, vertices * 12)?
                .as_chunks::<12>()
                .0
                .iter()
                .map(|chunk| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().expect("four bytes"))
                    })
                })
                .collect::<Vec<[f32; 3]>>();
            let indices = take(&mut bytes, indices * 4)?
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| u32::from_le_bytes(*chunk))
                .collect::<Vec<_>>();
            require(
                positions.iter().flatten().all(|p| p.is_finite())
                    && indices.iter().all(|&i| (i as usize) < vertices),
                "Недопустимые вершины или индексы геометрии",
            )?;
            meshes.push(AnatomicalMesh {
                id,
                positions,
                indices,
            });
        }
        require(bytes.is_empty(), "Лишние данные в файле геометрии")?;
        Ok(Self { meshes })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HemisphereView {
    Both,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrainColorMode {
    Groups,
    Anatomy,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainLayers {
    pub cortex: bool,
    pub deep: bool,
    pub cerebellum: bool,
    pub brainstem: bool,
    pub white_matter: bool,
    pub cavities: bool,
}
impl Default for BrainLayers {
    fn default() -> Self {
        Self {
            cortex: true,
            deep: true,
            cerebellum: true,
            brainstem: true,
            white_matter: false,
            cavities: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: [f32; 3],
}
impl Default for BrainCamera {
    fn default() -> Self {
        Self {
            yaw: -2.2,
            pitch: 0.24,
            distance: 4.6,
            target: [0.0; 3],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrainScene {
    pub format_version: u32,
    pub document_kind: String,
    pub atlas_id: String,
    pub atlas_revision: u32,
    pub selected_region: Option<String>,
    pub camera: BrainCamera,
    pub layers: BrainLayers,
    pub hemisphere: HemisphereView,
    pub color_mode: BrainColorMode,
    pub separation: f32,
    pub isolate: bool,
    pub hidden_regions: Vec<String>,
}
impl Default for BrainScene {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            document_kind: "brain_scene".into(),
            atlas_id: BRAIN_ATLAS_ID.into(),
            atlas_revision: BRAIN_ATLAS_REVISION,
            selected_region: None,
            camera: BrainCamera::default(),
            layers: BrainLayers::default(),
            hemisphere: HemisphereView::Both,
            color_mode: BrainColorMode::Groups,
            separation: 0.0,
            isolate: false,
            hidden_regions: Vec::new(),
        }
    }
}
impl BrainScene {
    pub fn validate(&self, atlas: &BrainAtlas) -> Result<(), Error> {
        require(
            self.format_version == FORMAT_VERSION
                && self.document_kind == "brain_scene"
                && self.atlas_id == atlas.atlas_id
                && self.atlas_revision == atlas.atlas_revision,
            "Неизвестная версия сцены мозга или атласа",
        )?;
        require(
            self.selected_region
                .as_ref()
                .is_none_or(|id| atlas.region(id).is_some())
                && self
                    .hidden_regions
                    .iter()
                    .all(|id| atlas.region(id).is_some()),
            "Неизвестная область мозга в сцене",
        )?;
        let hidden: HashSet<_> = self.hidden_regions.iter().collect();
        require(
            hidden.len() == self.hidden_regions.len(),
            "Повторные скрытые области",
        )?;
        require(
            !self.isolate || self.selected_region.is_some(),
            "Для изоляции выберите область",
        )?;
        let c = self.camera;
        require(
            c.yaw.is_finite()
                && c.yaw.abs() <= 10000.0
                && c.pitch.is_finite()
                && c.pitch.abs() <= 1.5
                && c.distance.is_finite()
                && (0.2..=15.0).contains(&c.distance)
                && c.target.iter().all(|v| v.is_finite() && v.abs() <= 10.0)
                && self.separation.is_finite()
                && (0.0..=1.0).contains(&self.separation),
            "Недопустимые параметры камеры или раздвижения полушарий",
        )
    }
    pub fn from_json(text: &str, atlas: &BrainAtlas) -> Result<Self, Error> {
        let scene: Self = serde_json::from_str(text)?;
        scene.validate(atlas)?;
        Ok(scene)
    }
    pub fn load(path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<Self, Error> {
        Self::from_json(&read_small_file(path.as_ref())?, atlas)
    }
    pub fn save(&self, path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<(), Error> {
        self.validate(atlas)?;
        atomic_write(
            path.as_ref(),
            format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes(),
        )
    }
    pub fn visible(&self, r: &BrainRegion) -> bool {
        let layer = match r.kind {
            TissueKind::Cortex => self.layers.cortex,
            TissueKind::GrayMatter => self.layers.deep,
            TissueKind::Brainstem => self.layers.brainstem,
            TissueKind::Cerebellum => self.layers.cerebellum,
            TissueKind::WhiteMatter => self.layers.white_matter,
            TissueKind::Cavity => self.layers.cavities,
        };
        layer
            && !self.hidden_regions.contains(&r.id)
            && (!self.isolate || self.selected_region.as_ref() == Some(&r.id))
            && match self.hemisphere {
                HemisphereView::Both => true,
                HemisphereView::Left => r.hemisphere != Hemisphere::Right,
                HemisphereView::Right => r.hemisphere != Hemisphere::Left,
            }
    }
}

/// The workspace can contain either anatomical view state or a numerical scenario.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceDocument {
    Brain(BrainScene),
    Experiment(crate::Experiment),
}
impl WorkspaceDocument {
    pub fn load(path: impl AsRef<Path>, atlas: &BrainAtlas) -> Result<Self, Error> {
        let text = read_small_file(path.as_ref())?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value.get("document_kind").is_some() {
            BrainScene::from_json(&text, atlas).map(Self::Brain)
        } else {
            crate::Experiment::from_json(&text).map(Self::Experiment)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anatomical_manifest_matches_geometry_and_rejects_corruption() {
        let atlas = BrainAtlas::bundled().unwrap();
        let geometry = BrainGeometry::bundled(&atlas).unwrap();
        assert_eq!(atlas.regions.len(), 58);
        assert_eq!(geometry.meshes.len(), 63);
        assert_eq!(
            geometry
                .meshes
                .iter()
                .map(|m| m.indices.len() / 3)
                .sum::<usize>(),
            370316
        );
        let mut atlas = atlas;
        atlas.regions[0].mesh_ids.push("missing".into());
        assert!(atlas.validate().is_err());
        assert!(BrainGeometry::decode(b"NBRN0001", &atlas).is_err());
    }
    #[test]
    fn scene_roundtrip_visibility_and_unknown_state_are_explicit() {
        let atlas = BrainAtlas::bundled().unwrap();
        let mut scene = BrainScene::default();
        assert_eq!(
            BrainScene::from_json(&serde_json::to_string(&scene).unwrap(), &atlas).unwrap(),
            scene
        );
        let left = atlas.region("FMA72654").unwrap();
        let right = atlas.region("FMA72653").unwrap();
        scene.hemisphere = HemisphereView::Left;
        assert!(scene.visible(left));
        assert!(!scene.visible(right));
        scene.layers.cortex = false;
        assert!(!scene.visible(left));
        scene.camera.distance = f32::NAN;
        assert!(scene.validate(&atlas).is_err());
        scene.camera = BrainCamera::default();
        scene.selected_region = Some("unknown".into());
        assert!(scene.validate(&atlas).is_err());
        assert!(
            atlas
                .csv()
                .lines()
                .skip(1)
                .all(|line| line.ends_with(",not_configured"))
        );
    }
}
