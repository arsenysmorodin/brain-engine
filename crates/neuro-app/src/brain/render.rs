use super::{DisplayMesh, FIELD_OF_VIEW, camera_transform};
use crate::{Lab, View};
use bevy::{
    asset::RenderAssetUsages,
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    mesh::Indices,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureFormat},
};
use bevy_egui::{
    EguiGlobalSettings, EguiTextureHandle, EguiUserTextures, PrimaryEguiContext, egui,
};
use neuro_domain::{BrainColorMode, BrainGroup, BrainScene};

#[derive(Resource)]
pub struct BrainViewport {
    pub image: Handle<Image>,
    pub texture: egui::TextureId,
    size: UVec2,
    last_scene: Option<BrainScene>,
    active: bool,
    last_simulation: Option<(bool, u64, bool, u64, bool, u64)>,
}
#[derive(Component)]
pub struct BrainCameraEntity;
#[derive(Component)]
pub struct BrainPart {
    region: usize,
    vessel: bool,
}

pub fn setup(
    mut commands: Commands,
    lab: Res<Lab>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
    mut settings: ResMut<EguiGlobalSettings>,
) {
    settings.auto_create_primary_context = false;
    commands.spawn((Camera2d, PrimaryEguiContext, RenderLayers::none()));
    let size = UVec2::new(1000, 800);
    let image = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));
    let texture = textures.add_image(EguiTextureHandle::Strong(image.clone()));
    commands.spawn((
        BrainCameraEntity,
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb_u8(17, 21, 28)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Projection::Perspective(PerspectiveProjection {
            fov: FIELD_OF_VIEW,
            near: 0.01,
            far: 50.0,
            ..default()
        }),
        Msaa::Off,
        Tonemapping::Reinhard,
        camera_transform(lab.brain.scene.camera),
    ));
    commands.insert_resource(BrainViewport {
        image,
        texture,
        size,
        last_scene: None,
        active: true,
        last_simulation: None,
    });
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.78, 0.86, 1.0),
        brightness: 240.0,
        ..default()
    });
    for (position, color, illuminance) in [
        (
            Vec3::new(-4.0, 6.0, -5.0),
            Color::srgb(1.0, 0.94, 0.89),
            6500.0,
        ),
        (
            Vec3::new(4.0, 1.0, 3.0),
            Color::srgb(0.58, 0.74, 1.0),
            3500.0,
        ),
    ] {
        commands.spawn((
            DirectionalLight {
                color,
                illuminance,
                // Keep the offscreen anatomical view lightweight: directional
                // shading is sufficient without additional shadow-map passes.
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
        ));
    }
    for source in &lab.brain.meshes {
        let mesh = display_mesh(source);
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.65, 0.75, 0.8),
            perceptual_roughness: 0.53,
            reflectance: 0.25,
            ..default()
        });
        commands.spawn((
            BrainPart {
                region: source.region,
                vessel: false,
            },
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
        ));
    }
    setup_flow(&mut commands, &lab.brain, &mut meshes, &mut materials);
    for source in &lab.brain.vascular_meshes {
        commands.spawn((
            BrainPart {
                region: source.region,
                vessel: true,
            },
            Mesh3d(meshes.add(display_mesh(source))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb_u8(217, 104, 103),
                perceptual_roughness: 0.4,
                reflectance: 0.3,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
}

fn display_mesh(source: &DisplayMesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        source
            .positions
            .iter()
            .map(|p| p.to_array())
            .collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(source.indices.clone()));
    // The atlas is normalized to a small display space. Angle-weighted
    // normals treat many fine triangles as zero-sized at that scale.
    // Area weighting preserves their surface normals without changing geometry.
    mesh.compute_area_weighted_normals();
    mesh
}

pub fn sync(
    lab: Res<Lab>,
    mut viewport: ResMut<BrainViewport>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(&mut Camera, &mut Transform), With<BrainCameraEntity>>,
    mut parts: Query<
        (
            &BrainPart,
            &mut Visibility,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<BrainCameraEntity>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((mut camera, mut transform)) = cameras.single_mut() else {
        return;
    };
    let active = lab.view == View::Brain;
    if viewport.active != active {
        camera.is_active = active;
        viewport.active = active;
    }
    if !active {
        return;
    }
    if viewport.size != lab.brain.target_size {
        viewport.size = lab.brain.target_size;
        if let Some(image) = images.get_mut(&viewport.image) {
            image.resize(Extent3d {
                width: viewport.size.x,
                height: viewport.size.y,
                ..default()
            });
        }
    }
    let simulation = &lab.brain.simulation;
    let caffeine = &lab.brain.caffeine;
    let simulation_revision = (
        simulation.mode,
        simulation.revision,
        caffeine.mode,
        caffeine.revision,
        lab.brain.circulation.mode,
        lab.brain.circulation.revision,
    );
    if viewport.last_scene.as_ref() == Some(&lab.brain.scene)
        && viewport.last_simulation == Some(simulation_revision)
    {
        return;
    }
    *transform = camera_transform(lab.brain.scene.camera);
    for (part, mut visibility, mut transform, handle) in &mut parts {
        if part.vessel {
            let v = &lab.brain.vascular_atlas.vessels[part.region];
            let side_visible = !matches!(
                (lab.brain.scene.hemisphere, v.hemisphere),
                (
                    neuro_domain::HemisphereView::Left,
                    neuro_domain::Hemisphere::Right
                ) | (
                    neuro_domain::HemisphereView::Right,
                    neuro_domain::Hemisphere::Left
                )
            );
            *visibility = if ((caffeine.mode && caffeine.show_vessels)
                || (lab.brain.circulation.mode && lab.brain.circulation.arteries))
                && side_visible
            {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            if let Some(material) = materials.get_mut(&handle.0) {
                let rgb = if lab.brain.circulation.mode {
                    lab.brain.circulation.artery_rgb()
                } else {
                    super::vascular::vessel_rgb(&lab.brain, part.region)
                };
                material.base_color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
            }
            continue;
        }
        let region = &lab.brain.atlas.regions[part.region];
        *visibility = if lab.brain.scene.visible(region)
            && (!caffeine.mode || caffeine.tissue_opacity > 0.0)
            && (!lab.brain.circulation.mode || lab.brain.circulation.opacity > 0.0)
        {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        transform.translation = lab.brain.offset(region);
        if let Some(material) = materials.get_mut(&handle.0) {
            let selected = lab.brain.scene.selected_region.as_ref() == Some(&region.id);
            let activity = simulation.state(&region.id);
            let rgb = if lab.brain.circulation.mode {
                lab.brain.circulation.region_rgb(region)
            } else if caffeine.mode {
                caffeine.region_rgb(region)
            } else if simulation.mode {
                activity.map_or([65, 69, 77], |s| super::simulation::heat_rgb(s.e))
            } else if selected {
                [112, 234, 214]
            } else if lab.brain.scene.color_mode == BrainColorMode::Groups {
                group_rgb(region.group)
            } else {
                [184, 194, 207]
            };
            let alpha = if lab.brain.circulation.mode {
                lab.brain.circulation.opacity
            } else if caffeine.mode {
                if selected {
                    caffeine.tissue_opacity.max(0.6)
                } else {
                    caffeine.tissue_opacity
                }
            } else {
                1.0
            };
            material.base_color =
                Color::srgba_u8(rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8);
            material.alpha_mode = if alpha < 0.999 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            };
            material.emissive = if caffeine.mode {
                LinearRgba::BLACK
            } else if simulation.mode {
                let glow = activity.map_or(0.0, |s| s.e as f32 * 0.3);
                LinearRgba::rgb(
                    glow * rgb[0] as f32 / 255.0,
                    glow * rgb[1] as f32 / 255.0,
                    glow * rgb[2] as f32 / 255.0,
                )
            } else if selected {
                LinearRgba::rgb(0.045, 0.15, 0.12)
            } else {
                LinearRgba::BLACK
            };
        }
    }
    viewport.last_scene = Some(lab.brain.scene.clone());
    viewport.last_simulation = Some(simulation_revision);
}
pub fn group_rgb(group: BrainGroup) -> [u8; 3] {
    match group {
        BrainGroup::Frontal => [133, 204, 196],
        BrainGroup::Parietal => [151, 171, 224],
        BrainGroup::Temporal => [188, 157, 211],
        BrainGroup::Occipital => [219, 166, 190],
        BrainGroup::Insula => [205, 169, 135],
        BrainGroup::Medial => [134, 194, 162],
        BrainGroup::Deep => [220, 177, 118],
        BrainGroup::Cerebellum => [129, 175, 207],
        BrainGroup::Brainstem => [159, 186, 173],
        BrainGroup::WhiteMatter => [211, 218, 230],
        BrainGroup::Ventricles => [105, 203, 224],
    }
}
pub fn egui_color(group: BrainGroup) -> egui::Color32 {
    let [r, g, b] = group_rgb(group);
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fine_triangles_have_usable_lighting_normals() {
        let mesh = display_mesh(&DisplayMesh {
            region: 0,
            positions: vec![Vec3::ZERO, Vec3::X * 0.0001, Vec3::Y * 0.0001],
            indices: vec![0, 1, 2],
            min: Vec3::ZERO,
            max: Vec3::splat(0.0001),
        });
        for normal in mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .unwrap()
            .as_float3()
            .unwrap()
        {
            assert!((Vec3::from_array(*normal) - Vec3::Z).length() < 1e-6);
        }
    }
}

#[derive(Component)]
pub struct FlowSegment {
    path: usize,
}
#[derive(Component)]
pub struct FlowParticle {
    path: usize,
}
fn setup_flow(
    commands: &mut Commands,
    brain: &super::BrainLab,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let bead = meshes.add(Sphere::new(0.014).mesh().ico(1).expect("icosphere"));
    for (i, source) in brain.flow_geometry.meshes.iter().enumerate() {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(80, 115, 180),
            perceptual_roughness: 0.5,
            ..default()
        });
        commands.spawn((
            FlowSegment { path: i },
            Mesh3d(meshes.add(display_mesh(source))),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
        ));
        commands.spawn((
            FlowParticle { path: i },
            Mesh3d(bead.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                unlit: true,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
}
pub fn sync_flow(
    lab: Res<Lab>,
    mut segments: Query<
        (
            &FlowSegment,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<FlowParticle>,
    >,
    mut particles: Query<
        (
            &FlowParticle,
            &mut Visibility,
            &mut Transform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<FlowSegment>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let c = &lab.brain.circulation;
    let active = lab.view == View::Brain && c.mode;
    for (part, mut visibility, handle) in &mut segments {
        let visible = active && c.path_visible(&lab.brain, part.path);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if visible && let Some(material) = materials.get_mut(&handle.0) {
            let rgb = c.path_rgb(&lab.brain, part.path);
            material.base_color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
        }
    }
    for (part, mut visibility, mut transform, handle) in &mut particles {
        let path = &lab.brain.flow_geometry.paths[part.path];
        let q_nonzero = path
            .region
            .and_then(|i| c.bed_index(&lab.brain.atlas.regions[i].id))
            .and_then(|i| c.frame().map(|f| f.cbf_ml_100g_min[i] > 0.0))
            .unwrap_or_else(|| {
                c.frame()
                    .is_some_and(|f| f.systemic[c.selected.index()].return_mg_min > 0.0)
            });
        let visible = active
            && c.particles
            && c.path_visible(&lab.brain, part.path)
            && q_nonzero
            && c.path_concentration(&lab.brain, part.path) > 1e-9;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if visible {
            transform.translation = super::circulation::geometry::along(
                &path.points,
                (c.time_min * 12.0 + part.path as f64 * 0.171) as f32,
            );
            if let Some(material) = materials.get_mut(&handle.0) {
                let color = super::circulation::DRUG_COLORS[c.selected.index()].to_array();
                material.base_color = Color::srgb_u8(color[0], color[1], color[2]);
            }
        }
    }
}
