//! Systems for digable terrain.

use std::f32::consts::PI;
use std::time::Duration;

use crate::player_movement::{Collider, Player};
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

/// resting position when picked

/// Only one item can be held at a time.
#[derive(Resource)]
pub enum Inventory {
    Shovel(usize),
    MiningPick,
    Food,
    None,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::None
    }
}

/// Initializes colliders for dirt, attaches markers for digging
/// and implements digging capabilities for [`Diggable`] entities.
pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_dirt_colliders)
            .add_observer(add_collectibles)
            .init_gizmo_group::<MyRoundGizmos>()
            .init_resource::<Inventory>()
            // TODO: remove this for custom interaction system
            .add_plugins(MeshPickingPlugin)
            .add_systems(
                PostUpdate,
                add_colliders_to_diggables.after(TransformSystem::TransformPropagate),
            )
            .add_systems(
                Update,
                (
                    draw_mesh_intersections,
                    manage_inventory,
                    animate_interaction,
                    tick_on_hand_active,
                ),
            );
        if cfg!(debug_assertions) {
            app.add_systems(Update, (activate_gizmos, draw_collider_gizmos));
        }
    }
}
// We can create our own gizmo config group!
#[derive(Default, Reflect, GizmoConfigGroup)]
struct MyRoundGizmos;

// Markers for entities that can be interacted with.
/// Can be removed with the shovel.
#[derive(Component)]
struct Diggable;
/// Can be removed with the Mining Pick.
#[derive(Component)]
struct Minable;
/// Can be taken (shovel, mining pick, food)
#[derive(Component)]
enum Collectible {
    Shovel,
    MiningPick,
    Food,
}

impl Collectible {
    fn on_hand_poses(&self) -> (Vec3, Quat) {
        match self {
            Collectible::MiningPick => {
                const PICK_OFFSET: Vec3 = Vec3::new(1.8, 1.5, -2.4);
                (
                    PICK_OFFSET,
                    Quat::from_euler(
                        EulerRot::YXZ,
                        -std::f32::consts::FRAC_PI_2, // yaw  -90°  (tip forward)
                        -0.35,                        // pitch ~-20° (look slightly down along it)
                        0.25,                         // roll  +14°  (handle tilt)
                    ),
                )
            }
            _ => {
                const FPS_OFFSET: Vec3 = Vec3::new(1.4, -0.25, -1.8); // X right, Y up, Z forward
                (
                    FPS_OFFSET,
                    Quat::from_euler(
                        EulerRot::YXZ,
                        -std::f32::consts::FRAC_PI_2, // yaw  -90°  (tip forward)
                        -0.35,                        // pitch ~-20° (look slightly down along it)
                        0.25,                         // roll  +14°  (handle tilt)
                    ),
                )
            }
        }
    }
}

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
    mut inventory: ResMut<Inventory>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyU) {
        config_store.config_mut::<AabbGizmoConfigGroup>().1.draw_all ^= true;
    }
    if keyboard_input.just_pressed(KeyCode::KeyI) {
        config_store.config_mut::<MyRoundGizmos>().0.enabled ^= true;
    }
    // cheats for testing
    if keyboard_input.just_pressed(KeyCode::Digit1) {
        *inventory = Inventory::Shovel(7);
    }
    if keyboard_input.just_pressed(KeyCode::Digit2) {
        *inventory = Inventory::MiningPick;
    }
    if keyboard_input.just_pressed(KeyCode::Digit3) {
        *inventory = Inventory::Food;
    }
    if keyboard_input.just_pressed(KeyCode::Digit0) {
        *inventory = Inventory::None;
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
    mut inventory: ResMut<Inventory>,
    diggables: Query<Entity, With<Diggable>>,
    minables: Query<Entity, With<Minable>>,
    mut collectables: Query<(Entity, &mut Transform, &Collectible, &mut OnHand)>,
    player_query: Query<Entity, With<Player>>,
) {
    match inventory.as_mut() {
        Inventory::Shovel(counter) => {
            if let Ok(digged) = diggables.get(trigger.target()) {
                // trigger.event().pointer_location can be used for particles etc.
                commands.entity(digged).despawn();
                *counter -= 1;
                for (_, _, _, mut on_hand) in collectables.iter_mut() {
                    if on_hand.active {
                        println!("on hand activated!");
                        on_hand.timer.unpause();
                        on_hand.timer.reset();
                    }
                }
                return;
            }
        }
        Inventory::MiningPick => {
            if let Ok(mined) = minables.get(trigger.target()) {
                // trigger.event().pointer_location can be used for particles etc.
                commands.entity(mined).despawn();
                return;
            }
        }
        _ => (),
    }
    if let Ok((collected, mut transform, collectible, mut on_hand)) =
        collectables.get_mut(trigger.target())
    {
        let Ok(player_ent) = player_query.single() else {
            return;
        };
        // despawn currently held tool if any
        commands.entity(player_ent).despawn_related::<Children>();
        let (rest_pos, rest_rot) = collectible.on_hand_poses();
        // trigger.event().pointer_location can be used for particles etc.
        transform.translation = rest_pos;
        transform.rotation = rest_rot;

        commands.entity(player_ent).insert_children(4, &[collected]);
        *inventory = match collectible {
            Collectible::Shovel => Inventory::Shovel(7),
            Collectible::MiningPick => Inventory::MiningPick,
            Collectible::Food => Inventory::Food,
        };
        on_hand.active = true;
    }
}

fn tick_on_hand_active(time: Res<Time>, mut on_hand_query: Query<&mut OnHand>) {
    for mut on_hand in &mut on_hand_query {
        if on_hand.active {
            on_hand.timer.tick(time.delta());
        }
    }
}

/// Check changes for inventory and update accordingly (shovel broken, etc.).
fn manage_inventory(
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    collectables: Query<(Entity, &mut Collectible)>,
) {
    if inventory.is_changed() {
        match inventory.as_mut() {
            Inventory::Shovel(counter) if (*counter <= 0) => {
                for entity in collectables
                    .iter()
                    .filter(|(_, collect)| matches!(collect, Collectible::Shovel))
                    .map(|(ent, _)| ent)
                {
                    commands.entity(entity).despawn();
                    *inventory = Inventory::None;
                }
            }
            _ => (),
        }
    }
}

#[derive(Component)]
struct OnHand {
    active: bool,
    timer: Timer,
}

impl OnHand {
    fn new() -> Self {
        let mut timer = Timer::new(Duration::from_millis(250), TimerMode::Once);
        timer.set_elapsed(Duration::from_millis(250));
        Self {
            active: false,
            timer,
        }
    }
}

/// Attach [`Collectible`] markers to Shovel and MiningPick on spawn from gltf.
fn add_collectibles(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    meshes: Query<(Entity, &Name), (Without<Collectible>, Added<Name>)>,
) {
    let _e = trigger.target();
    for (ent, name) in meshes.iter() {
        match name.as_str() {
            "Shovel" => {
                println!("added shovel");
                commands
                    .entity(ent)
                    .insert(Collectible::Shovel)
                    .insert(OnHand::new())
                    .observe(remove_on_click);
            }
            "MiningPick" => {
                println!("added pick");
                commands
                    .entity(ent)
                    .insert(Collectible::MiningPick)
                    .insert(OnHand::new())
                    .observe(remove_on_click);
            }
            _ => (),
        }
    }
}
fn wave(t: f32) -> f32 {
    (t * PI).sin()
}

fn animate_interaction(mut bones: Query<(&mut Transform, &OnHand, &Collectible)>) {
    for (mut transform, on_hand, collectible) in &mut bones {
        if !on_hand.active || on_hand.timer.finished() {
            continue;
        }
        // normalised time in the [0, 1] animation range
        let u = on_hand.timer.elapsed().as_secs_f32() / on_hand.timer.duration().as_secs_f32();
        let (rest_pos, rest_rot) = collectible.on_hand_poses();

        let (translation, rotation) = match collectible {
            Collectible::Shovel => {
                let a = wave(u);
                let t = rest_pos
                    + Vec3::new(
                        0.0,
                        -0.35 * a, // dip down
                        -0.45 * a, // and forward
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, -1.0 * a, 0.0, 0.0);
                (t, r)
            }

            Collectible::MiningPick => {
                // triangular pulse (faster change at the ends)
                let a = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
                let t = rest_pos
                    + Vec3::new(
                        0.0,
                        0.35 * a,  // up over head
                        -0.30 * a, // slight Z pull
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, 1.2 * a, 0.0, 0.0);
                (t, r)
            }

            Collectible::Food => {
                let a = wave(u);
                let t = rest_pos
                    + Vec3::new(
                        -0.15 * a, // toward centre
                        0.25 * a,  // up to mouth
                        0.80 * a,  // closer to camera
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::YXZ, 0.15 * a, -0.10 * a, 0.10 * a);
                (t, r)
            }
        };

        transform.translation = translation;
        transform.rotation = rotation;
    }
}
