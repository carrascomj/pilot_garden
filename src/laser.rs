//! Laser component and materials.

use bevy::{
    camera::visibility::NoFrustumCulling,
    mesh::{primitives::{CylinderAnchor, CylinderMeshBuilder}, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

pub struct LaserPlugin;

impl Plugin for LaserPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<LaserMaterial>::default());
    }
}

/// Marker struct for the Laser beams of the killer arms.
#[derive(Component)]
pub struct LaserBeam;

#[derive(Component, Copy, Clone)]
pub struct LaserBeamSource {
    pub head: Entity,
}

/// Material attached to cylinder meshes, children entities of killer beams.
///
/// They have an associated WGSL shader to make them look funny, colors are
/// hardcoded inside the shader.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LaserMaterial {
    /// Timer fraction In [0,1] to visually build the anticipation for firing
    /// the laser. This is bound at material binding `0`.
    #[uniform(0)]
    pub fraction: f32,
}

impl Material for LaserMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/laser_beam.wgsl".into()
    }
    fn vertex_shader() -> ShaderRef {
        "shaders/laser_beam.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let _ = _key;
        descriptor.primitive.cull_mode = None; // disable backface culling
        Ok(())
    }
}

pub fn setup_laser_mesh(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<LaserMaterial>>,
    head: Entity,
) {
    // Create a single cylinder mesh (radius ~0.03, height 1.0 unit) anchored at bottom
    let beam_radius = 0.04;
    let beam_mesh = CylinderMeshBuilder::new(beam_radius, 1.0, 48)
        .anchor(CylinderAnchor::Bottom)
        .build();
    let beam_mesh_handle = meshes.add(beam_mesh);

    // Prepare a laser material (custom shader defined later) with initial color
    let beam_material_handle = materials.add(LaserMaterial {
        fraction: 0.0, // progress 0 at start
    });

    commands.spawn((
        Mesh3d(beam_mesh_handle.clone()),
        MeshMaterial3d(beam_material_handle.clone()),
        LaserBeam, // marker component
        LaserBeamSource { head },
        Transform::default(),       // will be updated each frame
        GlobalTransform::default(), // to ensure correct transform propagation
        Visibility::Hidden,         // start hidden until active
        NoFrustumCulling,
    ));
}
