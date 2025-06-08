//! Systems for digable terrain.

use std::f32::consts::PI;

use crate::config::{GameState, REST_ROT};
use crate::player_movement::{Collider, Player};
use crate::world_timer::TimerComp;
use bevy::color::palettes::tailwind::{PINK_100, RED_500};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

/// Only one item can be held at a time.
#[derive(Resource)]
pub enum Inventory {
    Shovel(usize),
    MiningPick,
    Food(usize),
    Seeds(usize),
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
            .add_event::<SeedsPlaced>()
            .init_gizmo_group::<MyRoundGizmos>()
            .init_resource::<Inventory>()
            // TODO: remove this for custom interaction system
            .add_plugins(MeshPickingPlugin)
            .add_systems(
                Update,
                (
                    draw_mesh_intersections,
                    manage_inventory,
                    animate_interaction,
                    remove_when_life_depleted,
                )
                    .run_if(in_state(GameState::Above)),
            )
            // tools animation might be playing while in Below already
            .add_systems(
                Update,
                (manage_inventory, remove_animation).run_if(not(in_state(GameState::Menu))),
            )
            .add_systems(
                PostUpdate,
                (
                    add_colliders_to_diggables.after(TransformSystem::TransformPropagate),
                    remove_animation,
                )
                    .run_if(in_state(GameState::Above)),
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
pub struct Diggable;
/// Can be removed with the Mining Pick.
#[derive(Component)]
pub struct Minable;
#[derive(Component)]
pub enum Life {
    Left(u8),
    JustSpawned,
}
/// Can be taken (shovel, mining pick, food)
#[derive(Component, PartialEq)]
pub enum Collectible {
    Shovel,
    MiningPick,
    Seeds,
    Food,
}

impl Collectible {
    // const fn on_hand_poses(&self) -> (Vec3, Quat) {
    fn on_hand_poses(&self) -> (Vec3, Quat) {
        let seeds_rot = Quat::from_euler(
            EulerRot::YXZ,
            -0.4,  // yaw   -90°  (tip forward)
            -0.10, // pitch -20°  (look slightly down along it)
            -0.3,  // roll  +14°  (handle tilt)
        );
        match self {
            Collectible::MiningPick => {
                const PICK_OFFSET: Vec3 = Vec3::new(1.8, 1.5, -2.4);
                (PICK_OFFSET, REST_ROT)
            }
            Collectible::Seeds => {
                const SEED_OFFSET: Vec3 = Vec3::new(1.6, -0.25, -2.4); // X right, Y up, Z forward
                (SEED_OFFSET, seeds_rot)
            }
            _ => {
                const FPS_OFFSET: Vec3 = Vec3::new(1.4, -0.25, -1.8); // X right, Y up, Z forward
                (FPS_OFFSET, REST_ROT)
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
    mut next_state: ResMut<NextState<GameState>>,
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
        *inventory = Inventory::Food(1);
    }
    if keyboard_input.just_pressed(KeyCode::Digit0) {
        *inventory = Inventory::None;
    }
    if keyboard_input.just_pressed(KeyCode::KeyM) {
        next_state.set(GameState::Menu);
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

#[derive(Event)]
pub struct SeedsPlaced {
    pub hit_position: Vec3,
}

pub fn remove_on_click(
    trigger: Trigger<Pointer<Pressed>>,
    mut seeds_event: EventWriter<SeedsPlaced>,
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    diggables: Query<Entity, With<Diggable>>,
    mut minables: Query<&mut Life, With<Minable>>,
    mut collectables: Query<(
        Entity,
        &mut Transform,
        &Collectible,
        &mut OnHand,
        &mut TimerComp,
    )>,
    player_query: Query<Entity, With<Player>>,
) {
    match inventory.as_mut() {
        Inventory::Shovel(counter) => {
            if let Ok(digged) = diggables.get(trigger.target()) {
                // println!("Target at {:?}", trigger.event().hit.position.unwrap());
                commands.entity(digged).insert(RemoveTimer::new());
                if *counter > 0 {
                    *counter -= 1;
                }
                for (_, _, _, on_hand, mut timer) in collectables.iter_mut() {
                    if on_hand.active {
                        timer.0.unpause();
                        timer.0.reset();
                    }
                }
                return;
            }
        }
        Inventory::MiningPick => {
            if let Ok(mut mined_life) = minables.get_mut(trigger.target()) {
                // trigger.event().pointer_location can be used for particles etc.
                if let Life::Left(count) = mined_life.as_mut() {
                    if *count > 0 {
                        *count -= 1;
                    }
                } else {
                    *mined_life = Life::Left(1);
                }

                for (_, _, _, on_hand, mut timer) in collectables.iter_mut() {
                    if on_hand.active {
                        timer.0.unpause();
                        timer.0.reset();
                    }
                }
                return;
            }
        }
        Inventory::Food(counter) => {
            // the picked entity does not matter, simply eat the banana.
            *counter -= 1;
            for (_, _, _, on_hand, mut timer) in collectables.iter_mut() {
                if on_hand.active {
                    timer.0.unpause();
                    timer.0.reset();
                }
            }
        }
        Inventory::Seeds(counter) => {
            if let Ok(_) = diggables.get(trigger.target()) {
                if let Some(hit_position) = trigger.event().hit.position {
                    seeds_event.write(SeedsPlaced { hit_position });
                    *counter -= 1;
                    for (_, _, _, on_hand, mut timer) in collectables.iter_mut() {
                        if on_hand.active {
                            timer.0.unpause();
                            timer.0.reset();
                        }
                    }
                }

                return;
            }
        }
        _ => (),
    }
    if let Ok((collected, mut transform, collectible, mut on_hand, _)) =
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
            Collectible::Food => Inventory::Food(1),
            Collectible::Seeds => Inventory::Seeds(1),
        };
        on_hand.active = true;
    }
}

/// Check changes for inventory and update accordingly (shovel broken, etc.).
fn manage_inventory(
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    collectables: Query<(Entity, &mut Collectible, &OnHand)>,
) {
    if inventory.is_changed() {
        let check_for = match inventory.as_mut() {
            Inventory::Shovel(counter) if (*counter <= 0) => Collectible::Shovel,
            Inventory::Food(counter) if (*counter <= 0) => Collectible::Food,
            Inventory::Seeds(counter) if (*counter <= 0) => Collectible::Seeds,
            _ => return,
        };
        for entity in collectables
            .iter()
            .filter(|(_, collect, on_hand)| (collect == &&check_for) && on_hand.active)
            .map(|(ent, _, _)| ent)
        {
            commands.entity(entity).insert(RemoveTimer::new());
            *inventory = Inventory::None;
        }
    }
}

#[derive(Component)]
#[require(TimerComp::from_elapsed(0.25))]
pub struct OnHand {
    active: bool,
}

impl OnHand {
    fn new() -> Self {
        Self { active: false }
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
                commands
                    .entity(ent)
                    .insert(Collectible::Shovel)
                    .insert(OnHand::new())
                    .observe(remove_on_click);
            }
            "MiningPick" => {
                commands
                    .entity(ent)
                    .insert(Collectible::MiningPick)
                    .insert(OnHand::new())
                    .observe(remove_on_click);
            }
            "Food" => {
                commands
                    .entity(ent)
                    .insert(Collectible::Food)
                    .insert(OnHand::new())
                    .observe(remove_on_click);
            }
            "Seeds" => {
                commands
                    .entity(ent)
                    .insert(Collectible::Seeds)
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

fn animate_interaction(mut bones: Query<(&mut Transform, &OnHand, &TimerComp, &Collectible)>) {
    for (mut transform, on_hand, timer, collectible) in &mut bones {
        let (rest_pos, rest_rot) = collectible.on_hand_poses();
        if !on_hand.active {
            continue;
        } else if timer.0.finished() {
            transform.translation = rest_pos;
            transform.rotation = rest_rot;
            continue;
        }
        // normalised time in the [0, 1] animation range
        let u = timer.0.fraction();

        let (translation, rotation) = match collectible {
            Collectible::Shovel => {
                let a = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
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
                let t = (1.0 - a) * rest_pos
                    + Vec3::new(
                        0.0,
                        0.35 * a, // up over head
                        2.0 * a,  // slight Z pull
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, 1.2 * a, 0.0, 0.0);
                (t, r)
            }

            Collectible::Food | Collectible::Seeds => {
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

// `RemoveTimer` owns its own [`Timer`] because it will be inserted in a
// existing Entity, presumably with a [`TimerComp`] already that would
// clash with it already.
#[derive(Component)]
struct RemoveTimer(Timer);

impl RemoveTimer {
    fn new() -> Self {
        Self(Timer::from_seconds(0.8, TimerMode::Once))
    }
}

/// Decrease in life (which can only be decreased) -> small scale decrease.
///
/// If life == 0, add a [`RemoveTimer`] that will play an animation and despawn
/// the entity afterwards at [`remove_animation`].
fn remove_when_life_depleted(
    mut commands: Commands,
    mut lifes: Query<(Entity, &mut Transform, &mut Life), Changed<Life>>,
) {
    for (entity, mut trans, mut life) in lifes.iter_mut() {
        if let Life::Left(count) = life.as_ref() {
            if count <= &0 {
                commands.entity(entity).insert(RemoveTimer::new());
            }
            if count < &3 {
                trans.scale *= 0.9;
            }
        } else {
            *life = Life::Left(1);
        }
    }
}

fn remove_animation(
    mut commands: Commands,
    time: Res<Time>,
    mut to_remove: Query<(Entity, &mut Transform, &mut RemoveTimer)>,
) {
    for (ent, mut trans, mut rm_timer) in &mut to_remove {
        if rm_timer.0.just_finished() {
            commands.entity(ent).despawn();
        } else {
            rm_timer.0.tick(time.delta());
            let u = rm_timer.0.fraction();
            trans.scale = (1. - u) * Vec3::ONE + u * Vec3::ZERO;
            trans.rotation *= Quat::from_rotation_y(0.2);
        }
    }
}
