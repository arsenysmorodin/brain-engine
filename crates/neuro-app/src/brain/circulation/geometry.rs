//! Representative capillaries and venous topology, deliberately schematic and enlarged.
use super::*;
use bevy::prelude::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Feed,
    Capillary,
    Vein,
    Sinus,
}
impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Feed => "Артериола",
            Self::Capillary => "Капиллярная сеть",
            Self::Vein => "Региональная вена",
            Self::Sinus => "Венозный синус / яремный отток",
        }
    }
}
pub struct FlowPath {
    pub stage: Stage,
    pub region: Option<usize>,
    pub name: String,
    pub points: Vec<Vec3>,
}
pub struct FlowGeometry {
    pub paths: Vec<FlowPath>,
    pub meshes: Vec<super::super::DisplayMesh>,
}
impl FlowGeometry {
    pub fn new(
        atlas: &BrainAtlas,
        centers: &[Vec3],
        arteries: &[super::super::DisplayMesh],
    ) -> Self {
        let mut paths = vec![];
        let confluence = Vec3::new(0.0, 0.24, 1.13);
        let sagittal = vec![
            Vec3::new(0.0, 0.50, -1.05),
            Vec3::new(0.0, 0.94, -0.8),
            Vec3::new(0.0, 1.13, -0.3),
            Vec3::new(0.0, 1.12, 0.25),
            Vec3::new(0.0, 0.86, 0.8),
            confluence,
        ];
        let deep = Vec3::new(0.0, 0.12, 0.3);
        for (i, r) in atlas
            .regions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind != neuro_domain::TissueKind::Cavity)
        {
            let c = centers[i];
            let a = c - Vec3::X * 0.09;
            let b = c + Vec3::X * 0.09;
            let artery = arteries
                .iter()
                .flat_map(|m| &m.positions)
                .min_by(|x, y| x.distance_squared(a).total_cmp(&y.distance_squared(a)))
                .copied()
                .unwrap_or(a - Vec3::Y * 0.2);
            paths.push(FlowPath {
                stage: Stage::Feed,
                region: Some(i),
                name: format!("Артериола: {}", r.name),
                points: vec![artery, artery.lerp(a, 0.6) - Vec3::Y * 0.025, a],
            });
            // Three parallel representative channels, not a microscopic reconstruction.
            for branch in -1..=1 {
                let d = branch as f32 * 0.035;
                paths.push(FlowPath {
                    stage: Stage::Capillary,
                    region: Some(i),
                    name: format!("Капилляры: {}", r.name),
                    points: vec![
                        a,
                        c + Vec3::new(-0.04, d, 0.025),
                        c + Vec3::new(0.01, d, -0.025),
                        c + Vec3::new(0.05, d, 0.025),
                        b,
                    ],
                });
            }
            let cortical = r.kind == neuro_domain::TissueKind::Cortex;
            let destination = if cortical {
                if c.y < 0.05 {
                    Vec3::new(c.x.signum() * 0.65, 0.14, 1.02)
                } else {
                    *sagittal
                        .iter()
                        .min_by(|x, y| x.distance_squared(c).total_cmp(&y.distance_squared(c)))
                        .unwrap()
                }
            } else {
                deep
            };
            let mid = if cortical {
                c.lerp(destination, 0.5) + Vec3::new(c.x.signum() * 0.09, 0.1, 0.0)
            } else {
                b.lerp(destination, 0.5)
            };
            paths.push(FlowPath {
                stage: Stage::Vein,
                region: Some(i),
                name: format!("Венозный отток: {}", r.name),
                points: vec![b, mid, destination],
            });
        }
        paths.push(FlowPath {
            stage: Stage::Sinus,
            region: None,
            name: "Верхний сагиттальный синус".into(),
            points: sagittal,
        });
        paths.push(FlowPath {
            stage: Stage::Sinus,
            region: None,
            name: "Большая вена мозга и прямой синус".into(),
            points: vec![deep, Vec3::new(0.0, 0.17, 0.55), confluence],
        });
        for side in [-1.0, 1.0] {
            paths.push(FlowPath {
                stage: Stage::Sinus,
                region: None,
                name: format!(
                    "Поперечный → сигмовидный синус → {} яремная вена",
                    if side < 0.0 {
                        "левая"
                    } else {
                        "правая"
                    }
                ),
                points: vec![
                    confluence,
                    Vec3::new(side * 0.65, 0.14, 1.02),
                    Vec3::new(side * 0.94, -0.12, 0.8),
                    Vec3::new(side * 0.84, -0.43, 0.5),
                    Vec3::new(side * 0.61, -0.7, 0.36),
                    Vec3::new(side * 0.55, -1.25, 0.32),
                ],
            });
        }
        for path in &mut paths {
            path.points = smooth(&path.points);
        }
        let meshes = paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                tube(
                    i,
                    &p.points,
                    match p.stage {
                        Stage::Feed => 0.009,
                        Stage::Capillary => 0.006,
                        Stage::Vein => 0.013,
                        Stage::Sinus => 0.027,
                    },
                )
            })
            .collect();
        Self { paths, meshes }
    }
    pub fn pick(&self, lab: &BrainLab, ndc: Vec2, aspect: f32) -> Option<usize> {
        let camera = super::super::camera_transform(lab.scene.camera);
        let origin = camera.translation;
        let tan = (super::super::FIELD_OF_VIEW * 0.5).tan();
        let dir =
            (camera.rotation * Vec3::new(ndc.x * tan * aspect, ndc.y * tan, -1.0)).normalize();
        let mut best = f32::INFINITY;
        let mut selected = None;
        for (i, m) in self.meshes.iter().enumerate() {
            if !lab.circulation.path_visible(lab, i) {
                continue;
            }
            for tri in m.indices.as_chunks::<3>().0 {
                if let Some(t) = super::super::ray_triangle(
                    origin,
                    dir,
                    m.positions[tri[0] as usize],
                    m.positions[tri[1] as usize],
                    m.positions[tri[2] as usize],
                ) && t < best
                {
                    best = t;
                    selected = Some(i);
                }
            }
        }
        selected
    }
}
fn tube(region: usize, points: &[Vec3], radius: f32) -> super::super::DisplayMesh {
    let mut positions = vec![];
    let mut indices = vec![];
    for pair in points.windows(2) {
        let direction = (pair[1] - pair[0]).normalize_or_zero();
        let u = direction.any_orthonormal_vector() * radius;
        let v = direction.cross(u);
        let base = positions.len() as u32;
        for point in pair {
            for j in 0..8 {
                let angle = j as f32 * std::f32::consts::TAU / 8.0;
                positions.push(*point + u * angle.cos() + v * angle.sin());
            }
        }
        for j in 0..8u32 {
            let k = (j + 1) % 8;
            indices.extend_from_slice(&[
                base + j,
                base + k,
                base + j + 8,
                base + k,
                base + k + 8,
                base + j + 8,
            ]);
        }
    }
    let (min, max) = positions.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(*p), b.max(*p)),
    );
    super::super::DisplayMesh {
        region,
        positions,
        indices,
        min,
        max,
    }
}
pub fn along(points: &[Vec3], phase: f32) -> Vec3 {
    let total = points.windows(2).map(|p| p[0].distance(p[1])).sum::<f32>();
    let mut distance = phase.rem_euclid(1.0) * total;
    for p in points.windows(2) {
        let len = p[0].distance(p[1]);
        if distance <= len {
            return p[0].lerp(p[1], distance / len.max(1e-6));
        }
        distance -= len;
    }
    *points.last().unwrap()
}

fn smooth(points: &[Vec3]) -> Vec<Vec3> {
    let mut out = Vec::new();
    for i in 0..points.len() - 1 {
        let a = points[i.saturating_sub(1)];
        let b = points[i];
        let c = points[i + 1];
        let d = points[(i + 2).min(points.len() - 1)];
        for j in 0..6 {
            let t = j as f32 / 6.0;
            out.push(
                0.5 * ((2.0 * b)
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                    + (-a + 3.0 * b - 3.0 * c + d) * t * t * t),
            );
        }
    }
    out.push(*points.last().unwrap());
    out
}
