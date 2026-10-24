pub mod caffeine;
mod controls;
mod render;
mod simulation;
pub use controls::{inspector, sidebar, viewport};
pub use render::{BrainViewport, setup, sync};
pub use simulation::SimulationLab;

use bevy::prelude::*;
use neuro_domain::{
    BrainAtlas, BrainCamera, BrainGeometry, BrainRegion, BrainScene, Error, Hemisphere,
    HemisphereView, TissueKind,
};
use std::path::PathBuf;

pub const FIELD_OF_VIEW: f32 = 40.0_f32.to_radians();
pub struct DisplayMesh {
    pub region: usize,
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub min: Vec3,
    pub max: Vec3,
}
pub struct BrainLab {
    pub atlas: BrainAtlas,
    pub scene: BrainScene,
    pub meshes: Vec<DisplayMesh>,
    pub centers: Vec<Vec3>,
    pub search: String,
    pub group: Option<neuro_domain::BrainGroup>,
    pub dirty: bool,
    pub last_scene_path: Option<PathBuf>,
    pub target_size: UVec2,
    pub simulation: SimulationLab,
    pub caffeine: caffeine::CaffeineLab,
}
impl BrainLab {
    pub fn new() -> Result<Self, Error> {
        let atlas = BrainAtlas::bundled()?;
        let geometry = BrainGeometry::bundled(&atlas)?;
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in geometry.meshes.iter().flat_map(|m| &m.positions) {
            let p = Vec3::from_array(*p);
            min = min.min(p);
            max = max.max(p);
        }
        let center = (min + max) * 0.5;
        let scale = 2.8 / (max - min).max_element();
        let meshes: Vec<DisplayMesh> = geometry
            .meshes
            .into_iter()
            .map(|m| {
                let region = atlas
                    .regions
                    .iter()
                    .position(|r| r.mesh_ids.contains(&m.id))
                    .expect("validated mesh ownership");
                let positions: Vec<_> = m
                    .positions
                    .into_iter()
                    .map(|p| {
                        let p = (Vec3::from_array(p) - center) * scale;
                        // Source positive X is left, Y is posterior, Z is superior.
                        // Preserve handedness: display X right, Y superior, Z posterior.
                        Vec3::new(-p.x, p.z, p.y)
                    })
                    .collect();
                let (min, max) = positions.iter().fold(
                    (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                    |(a, b), p| (a.min(*p), b.max(*p)),
                );
                DisplayMesh {
                    region,
                    positions,
                    indices: m.indices,
                    min,
                    max,
                }
            })
            .collect();
        let centers = (0..atlas.regions.len())
            .map(|n| {
                let mut sum = Vec3::ZERO;
                let mut count = 0;
                for m in meshes.iter().filter(|m| m.region == n) {
                    for p in &m.positions {
                        sum += *p;
                        count += 1;
                    }
                }
                sum / count as f32
            })
            .collect();
        let simulation = SimulationLab::new(&atlas)?;
        let caffeine = caffeine::CaffeineLab::new(&atlas)?;
        Ok(Self {
            atlas,
            scene: BrainScene::default(),
            meshes,
            centers,
            search: String::new(),
            group: None,
            dirty: false,
            last_scene_path: None,
            target_size: UVec2::new(1000, 800),
            simulation,
            caffeine,
        })
    }
    pub fn offset(&self, r: &BrainRegion) -> Vec3 {
        Vec3::X
            * self.scene.separation
            * 0.75
            * match r.hemisphere {
                Hemisphere::Left => -1.0,
                Hemisphere::Right => 1.0,
                Hemisphere::Bilateral => 0.0,
            }
    }
    pub fn selected(&self) -> Option<&BrainRegion> {
        self.scene
            .selected_region
            .as_deref()
            .and_then(|id| self.atlas.region(id))
    }
    pub fn reveal_selected(&mut self) {
        if let Some(r) = self.selected().cloned() {
            self.scene.hidden_regions.retain(|id| id != &r.id);
            if matches!(
                (self.scene.hemisphere, r.hemisphere),
                (HemisphereView::Left, Hemisphere::Right)
                    | (HemisphereView::Right, Hemisphere::Left)
            ) {
                self.scene.hemisphere = HemisphereView::Both;
            }
            match r.kind {
                TissueKind::Cortex => self.scene.layers.cortex = true,
                TissueKind::GrayMatter => self.scene.layers.deep = true,
                TissueKind::Cerebellum => self.scene.layers.cerebellum = true,
                TissueKind::Brainstem => self.scene.layers.brainstem = true,
                TissueKind::WhiteMatter => self.scene.layers.white_matter = true,
                TissueKind::Cavity => self.scene.layers.cavities = true,
            }
        }
    }
    pub fn focus_selected(&mut self) {
        if let Some(r) = self.selected().cloned() {
            self.reveal_selected();
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for mesh in self
                .meshes
                .iter()
                .filter(|m| self.atlas.regions[m.region].id == r.id)
            {
                min = min.min(mesh.min);
                max = max.max(mesh.max);
            }
            self.scene.camera.target = ((min + max) * 0.5 + self.offset(&r)).to_array();
            let rotation = camera_transform(self.scene.camera).rotation.inverse();
            let half = (max - min) * 0.5;
            let aspect = self.target_size.x as f32 / self.target_size.y as f32;
            let tan_y = (FIELD_OF_VIEW * 0.5).tan();
            let tan_x = tan_y * aspect;
            let mut distance = 0.2_f32;
            // Fit the projected box with a margin, including small deep structures.
            for x in [-half.x, half.x] {
                for y in [-half.y, half.y] {
                    for z in [-half.z, half.z] {
                        let p = rotation * Vec3::new(x, y, z);
                        distance =
                            distance.max(p.z + 1.18 * (p.x.abs() / tan_x).max(p.y.abs() / tan_y));
                    }
                }
            }
            self.scene.camera.distance = distance.clamp(0.2, 15.0);
        }
    }
    pub fn pick(&self, ndc: Vec2, aspect: f32) -> Option<String> {
        let camera = camera_transform(self.scene.camera);
        let origin = camera.translation;
        let tan = (FIELD_OF_VIEW * 0.5).tan();
        let direction =
            (camera.rotation * Vec3::new(ndc.x * tan * aspect, ndc.y * tan, -1.0)).normalize();
        let mut candidates: Vec<_> = self
            .meshes
            .iter()
            .filter_map(|m| {
                let r = &self.atlas.regions[m.region];
                if !self.scene.visible(r) {
                    return None;
                }
                let offset = self.offset(r);
                ray_box(origin - offset, direction, m.min, m.max)
                    .map(|distance| (distance, m, offset))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut nearest = f32::INFINITY;
        let mut selected = None;
        for (near, mesh, offset) in candidates {
            if near > nearest {
                break;
            }
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let [a, b, c] = triangle.map(|i| mesh.positions[i as usize]);
                if let Some(distance) = ray_triangle(origin - offset, direction, a, b, c)
                    && distance < nearest
                {
                    nearest = distance;
                    selected = Some(self.atlas.regions[mesh.region].id.clone());
                }
            }
        }
        selected
    }
}
pub fn advance_network(
    time: Res<Time<Real>>,
    mut lab: ResMut<crate::Lab>,
    windows: Query<&Window>,
) {
    if lab.view == crate::View::Brain && windows.iter().any(|w| w.focused) {
        lab.brain.simulation.tick(time.delta_secs_f64());
        lab.brain.caffeine.tick(time.delta_secs_f64());
    }
}
pub fn camera_transform(camera: BrainCamera) -> Transform {
    let target = Vec3::from_array(camera.target);
    let direction = Vec3::new(
        camera.pitch.cos() * camera.yaw.sin(),
        camera.pitch.sin(),
        camera.pitch.cos() * camera.yaw.cos(),
    );
    Transform::from_translation(target + direction * camera.distance).looking_at(target, Vec3::Y)
}
fn ray_box(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-8 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - origin[axis]) / direction[axis];
            let b = (max[axis] - origin[axis]) / direction[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}
fn ray_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let cross = direction.cross(e2);
    let det = e1.dot(cross);
    if det.abs() < 1e-10 {
        return None;
    }
    let inv = 1.0 / det;
    let from = origin - a;
    let u = from.dot(cross) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = from.cross(e1);
    let v = direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = e2.dot(q) * inv;
    (distance >= 0.0).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ray_selection_obeys_visibility_and_uses_actual_geometry() {
        let mut lab = BrainLab::new().unwrap();
        let id = "FMA72654";
        lab.scene.selected_region = Some(id.into());
        lab.scene.isolate = true;
        lab.focus_selected();
        // A folded gyrus need not intersect the ray through its bounding-box center.
        // Project a real, non-degenerate surface triangle instead.
        let mesh = lab
            .meshes
            .iter()
            .find(|m| lab.atlas.regions[m.region].id == id)
            .unwrap();
        let triangle = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .find(|t| {
                let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
                (b - a).cross(c - a).length_squared() > 1e-12
            })
            .unwrap();
        let point = triangle
            .iter()
            .map(|&i| mesh.positions[i as usize])
            .sum::<Vec3>()
            / 3.0;
        let transform = camera_transform(lab.scene.camera);
        let local = transform.rotation.inverse() * (point - transform.translation);
        let tan = (FIELD_OF_VIEW * 0.5).tan();
        let ndc = Vec2::new(local.x / (-local.z * tan * 1.2), local.y / (-local.z * tan));
        assert_eq!(lab.pick(ndc, 1.2).as_deref(), Some(id));
        lab.scene.hidden_regions.push(id.into());
        assert_eq!(lab.pick(ndc, 1.2), None);
        let left = lab
            .meshes
            .iter()
            .find(|m| lab.atlas.regions[m.region].id == id)
            .unwrap();
        assert!(
            (left.min.x + left.max.x) * 0.5 < 0.0,
            "left must display on anatomical left"
        );
    }
    #[test]
    fn orbit_pan_and_zoom_keep_scenes_valid() {
        let atlas = BrainAtlas::bundled().unwrap();
        let mut scene = BrainScene::default();
        controls::orbit(&mut scene, Vec2::new(5000.0, 5000.0));
        controls::zoom(&mut scene, 1e6);
        assert!(scene.validate(&atlas).is_ok());
        controls::zoom(&mut scene, -1e6);
        assert!(scene.validate(&atlas).is_ok());
    }

    #[test]
    fn focus_fits_all_regions_in_portrait_and_landscape_viewports() {
        let mut lab = BrainLab::new().unwrap();
        for size in [UVec2::new(600, 800), UVec2::new(1200, 750)] {
            lab.target_size = size;
            lab.scene.separation = 0.7;
            for region in lab.atlas.regions.clone() {
                lab.scene.selected_region = Some(region.id.clone());
                lab.focus_selected();
                lab.scene.validate(&lab.atlas).unwrap();
                let camera = camera_transform(lab.scene.camera);
                let tan_y = (FIELD_OF_VIEW * 0.5).tan();
                let tan_x = tan_y * size.x as f32 / size.y as f32;
                for mesh in lab
                    .meshes
                    .iter()
                    .filter(|m| lab.atlas.regions[m.region].id == region.id)
                {
                    for position in &mesh.positions {
                        let local = camera.rotation.inverse()
                            * (*position + lab.offset(&region) - camera.translation);
                        assert!(local.z < -0.01);
                        assert!(local.x.abs() / (-local.z * tan_x) < 1.0, "{}", region.id);
                        assert!(local.y.abs() / (-local.z * tan_y) < 1.0, "{}", region.id);
                    }
                }
            }
        }
    }
}
