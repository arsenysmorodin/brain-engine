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
}
#[derive(Component)]
pub struct BrainCameraEntity;
#[derive(Component)]
pub struct BrainPart {
    region: usize,
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
                // Detailed, adjoining atlas surfaces produce shadow-map acne.
                // Soft directional shading keeps anatomical boundaries readable.
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
            },
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
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
    if viewport.last_scene.as_ref() == Some(&lab.brain.scene) {
        return;
    }
    *transform = camera_transform(lab.brain.scene.camera);
    for (part, mut visibility, mut transform, handle) in &mut parts {
        let region = &lab.brain.atlas.regions[part.region];
        *visibility = if lab.brain.scene.visible(region) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        transform.translation = lab.brain.offset(region);
        if let Some(material) = materials.get_mut(&handle.0) {
            let selected = lab.brain.scene.selected_region.as_ref() == Some(&region.id);
            let rgb = if selected {
                [112, 234, 214]
            } else if lab.brain.scene.color_mode == BrainColorMode::Groups {
                group_rgb(region.group)
            } else {
                [184, 194, 207]
            };
            material.base_color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]);
            material.emissive = if selected {
                LinearRgba::rgb(0.045, 0.15, 0.12)
            } else {
                LinearRgba::BLACK
            };
        }
    }
    viewport.last_scene = Some(lab.brain.scene.clone());
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
