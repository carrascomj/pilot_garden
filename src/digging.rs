//! Systems for digable terrain.

use crate::player_movement::Collider;
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

/// Initializes colliders for dirt, attaches markers for digging
/// and implements digging capabilities for [`Diggable`] entities.
pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_dirt_colliders)
            .init_gizmo_group::<MyRoundGizmos>()
            // TODO: remove this for custom interaction system
            .add_plugins(MeshPickingPlugin)
            .add_systems(
                PostUpdate,
                add_colliders_to_diggables.after(TransformSystem::TransformPropagate),
            )
            .add_systems(Update, draw_mesh_intersections);
        if cfg!(debug_assertions) {
            app.add_systems(Update, (activate_gizmos, draw_collider_gizmos));
        }
    }
}
// We can create our own gizmo config group!
#[derive(Default, Reflect, GizmoConfigGroup)]
struct MyRoundGizmos;

#[derive(Component)]
struct Diggable;

/// Attaches [`Diggable`] markers to gltf objects named as `crop_ground`.
fn add_dirt_colliders(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    meshes: Query<(Entity, &Name), (Without<Diggable>, Added<Name>)>,
) {
    let _e = trigger.target();
    for (ent, name) in meshes.iter() {
        if name.as_str().starts_with("crop_ground") {
            commands.entity(ent).insert(Diggable {});
        }
    }
}

/// Calculate and attach colliders to [`Diggable`] entities.
fn add_colliders_to_diggables(
    mut commands: Commands,
    diggables: Populated<(Entity, &Transform), (With<Diggable>, Without<Collider>)>,
) {
    // TODO: check these bounds
    let size = Vec3::new(1.0, 0.5, 1.0);

    for (ent, trans) in diggables.iter() {
        commands
            .entity(ent)
            .insert(Collider::from_translation(
                trans.translation + Vec3::Y * 0.5,
                size,
            ))
            .observe(remove_on_click);
    }
}

fn draw_collider_gizmos(mut my_gizmos: Gizmos<MyRoundGizmos>, dig_colliders: Query<&Collider>) {
    const CORAL: Color = Color::linear_rgb(1.0, 0.2, 0.2);
    for collider in dig_colliders.iter() {
        let centre = (collider.min + collider.max) * 0.5;
        let extent = collider.max - collider.min; // (width, height, depth)

        let xf: Mat4 = Mat4::from_scale_rotation_translation(
            extent,         // scale
            Quat::IDENTITY, // no rotation – AABB is axis-aligned
            centre,         // translation
        );

        my_gizmos.cuboid(xf, CORAL);
    }
}

fn activate_gizmos(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyU) {
        config_store.config_mut::<AabbGizmoConfigGroup>().1.draw_all ^= true;
    }
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        config_store.config_mut::<MyRoundGizmos>().0.enabled ^= true;
    }
}

/// TODO: remove this for custom interaction system. It should be in the middle
/// of the screen instead of at the pointer.
fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
    for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
    {
        gizmos.sphere(point, 0.05, RED_500);
        gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
    }
}

fn remove_on_click(
    trigger: Trigger<Pointer<Pressed>>,
    mut commands: Commands,
    diggables: Query<Entity, With<Diggable>>,
) {
    if let Ok(digged) = diggables.get(trigger.target()) {
        commands.entity(digged).despawn();
    }
}
