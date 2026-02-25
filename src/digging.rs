//! Systems for digable terrain.

use std::f32::consts::PI;

use crate::audio::AudioStart;
use crate::config::{GameState, REST_ROT, SEEDS_ROT, SHOVEL_DURABILITY, TOOL_ANIM_TIME};
use crate::emoji_particles::{EmojiBurst, SecretRevealed};
use crate::gnomes::{GnomeMachine, PlatformMover};
use crate::player_movement::{Collider, Player};
use crate::world_timer::TimerComp;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

/// Initializes colliders for dirt, attaches markers for digging
/// and implements digging capabilities for [`Diggable`] entities.
pub struct DiggingPlugin;

impl Plugin for DiggingPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_collectibles)
            .add_event::<SeedsPlaced>()
            .add_systems(
                Update,
                (remove_when_life_depleted, animate_shrink).run_if(in_state(GameState::Above)),
            )
            .add_systems(
                Update,
                animate_interaction
                    // need it in below for the button
                    .run_if(in_state(GameState::Below).or(in_state(GameState::Above))),
            )
            // tools animation might be playing while in Below already
            .add_systems(
                Update,
                remove_animation.run_if(not(in_state(GameState::Menu))),
            )
            .add_systems(
                PostUpdate,
                add_colliders_to_diggables
                    .after(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::Above)),
            );
    }
}

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
#[derive(Component)]
pub enum Collectible {
    Shovel(usize),
    MiningPick,
    Seeds,
    Food,
    /// Not a collectible per se, but it can also be interacted with
    /// based on the logic of the rest (always, regardles of tool at hand)
    Button,
}

/// Implement our own so that [`Collectible::Shovel`] is eq to shovel irrespective
/// of its count.
impl PartialEq<Collectible> for Collectible {
    fn eq(&self, other: &Collectible) -> bool {
        match (self, other) {
            (Collectible::Shovel(_), Collectible::Shovel(_)) => true,
            (Collectible::MiningPick, Collectible::MiningPick) => true,
            (Collectible::Seeds, Collectible::Seeds) => true,
            (Collectible::Food, Collectible::Food) => true,
            (Collectible::Button, Collectible::Button) => true,
            _ => false,
        }
    }
}

impl Collectible {
    pub const fn on_hand_poses(&self) -> (Vec3, Quat) {
        match self {
            Collectible::MiningPick => {
                const PICK_OFFSET: Vec3 = Vec3::new(1.4, -0.2, -1.8);
                (PICK_OFFSET, REST_ROT)
            }
            Collectible::Seeds => {
                const SEED_OFFSET: Vec3 = Vec3::new(1.6, -0.25, -2.8); // X right, Y up, Z forward
                (SEED_OFFSET, SEEDS_ROT)
            }
            Collectible::Button => (Vec3::new(-65., -23.8, 80.), Quat::IDENTITY),
            _ => {
                const FPS_OFFSET: Vec3 = Vec3::new(1.4, -0.25, -1.8); // X right, Y up, Z forward
                (FPS_OFFSET, REST_ROT)
            }
        }
    }
}

/// Marker for entities that get a 3D unit size collider.
#[derive(Component)]
pub struct OneSizeCollider;
/// Marker for the hidden diggable ground that can be removed to win.
#[derive(Component)]
pub struct FakeGround;

/// Remove the platform mover when [`FakeGround`] is removed ONLY IF
/// [`FakeGround`] was removed by the player and not by the
/// despawning and spwaning of crops when the player enters
/// the menu.
pub fn was_removed_by_player(
    trigger: Trigger<OnAdd, RemoveTimer>,
    mut commands: Commands,
    fake_ground: Query<&FakeGround>,
    platform_mover: Single<Entity, With<PlatformMover>>,
) {
    if fake_ground.contains(trigger.target()) {
        commands
            .entity(platform_mover.entity())
            .insert(RemoveTimer::new());
    }
}

/// Calculate and attach colliders to [`Diggable`] entities.
fn add_colliders_to_diggables(
    mut commands: Commands,
    diggables: Populated<(Entity, &GlobalTransform), (With<OneSizeCollider>, Without<Collider>)>,
) {
    let size = Vec3::new(1.0, 0.5, 1.0);

    for (ent, trans) in diggables.iter() {
        commands.entity(ent).insert(Collider::from_translation(
            trans.translation() + Vec3::Y * 0.5,
            size,
        ));
    }
}
#[derive(Event)]
pub struct SeedsPlaced {
    pub hit_position: Vec3,
}
#[derive(Component)]
#[require(TimerComp::from_elapsed(TOOL_ANIM_TIME))]
pub struct OnHand {
    pub active: bool,
}

impl OnHand {
    pub fn new() -> Self {
        Self { active: false }
    }
}

/// Attach [`Collectible`] markers to Shovel and MiningPick on spawn from gltf.
fn add_collectibles(
    trigger: Trigger<SceneInstanceReady>,
    mut commands: Commands,
    added_names: Query<(Entity, &Name), (Without<Collectible>, Added<Name>)>,
) {
    let _e = trigger.target();
    for (entity, name) in added_names.iter() {
        match name.as_str() {
            "Shovel" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Shovel(SHOVEL_DURABILITY))
                    .insert(OnHand::new())
                    .observe(audio_item_picked);
            }
            "MiningPick" => {
                commands
                    .entity(entity)
                    .insert(Collectible::MiningPick)
                    .insert(OnHand::new())
                    .observe(audio_item_picked);
            }
            "Food" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Food)
                    .insert(OnHand::new())
                    .observe(audio_item_picked);
            }
            "Seeds" => {
                commands
                    .entity(entity)
                    .insert(Collectible::Seeds)
                    .insert(OnHand::new())
                    .observe(audio_item_picked);
            }
            _ => (),
        }
    }
}

/// Emit audio on picking an item (inserted unto the player).
fn audio_item_picked(
    on_insert: Trigger<OnInsert, ChildOf>,
    mut audio_event: EventWriter<AudioStart>,
    child: Query<&ChildOf>,
    player_query: Query<Entity, With<Player>>,
) {
    if child
        .get(on_insert.target())
        .map(|e| player_query.get(e.0))
        .is_ok()
    {
        audio_event.write(AudioStart::Tock);
    }
}

fn wave(t: f32) -> f32 {
    (t * PI).sin()
}

fn button_press_ease(u: f32) -> f32 {
    let t = u.clamp(0.0, 1.0);
    let press_portion = 0.22; // first 22% of time is the press-down

    if t < press_portion {
        // fast ease-out to max depth
        let x = t / press_portion;
        1.0 - (1.0 - x).powi(3)
    } else {
        // slower ease-out back to rest
        let x = (t - press_portion) / (1.0 - press_portion);
        1.0 - x.powi(2)
    }
}

fn animate_interaction(
    mut bones: Query<(&mut Transform, &OnHand, &TimerComp, &Collectible)>,
    mut audio_event: EventWriter<AudioStart>,
) {
    for (mut transform, on_hand, timer, collectible) in &mut bones {
        // the children its the mesh, transforms are better
        // applied to the parent object in the gltf since it has
        // absolute coordinates
        let (rest_pos, rest_rot) = collectible.on_hand_poses();
        if !on_hand.active {
            continue;
        }
        // normalised time in the [0, 1] animation range
        let u = timer.0.fraction();

        let (translation, rotation, audio) = match collectible {
            Collectible::Shovel(_) => {
                let a = if u < 0.5 { u * 2.0 } else { (1.0 - u) * 2.0 };
                let t = rest_pos
                    + Vec3::new(
                        0.0,
                        -0.35 * a, // dip down
                        -0.45 * a, // and forward
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, -1.0 * a, 0.5, 0.0);
                (t, r, Some(AudioStart::Shovel))
            }
            Collectible::MiningPick => {
                let curve = CubicCardinalSpline {
                    tension: 0.4,
                    control_points: [
                        rest_pos,
                        rest_pos + Vec3::new(0.35, 0.35, 0.84),
                        rest_pos + Vec3::new(-0.5, -0.35, -1.),
                        rest_pos,
                    ]
                    .into(),
                }
                .to_curve()
                .expect("Should work");
                let t = curve.position(u * 2.);
                let r = rest_rot * Quat::from_euler(EulerRot::XYZ, -0.4 * wave(u), 0.5, 0.0);
                (t, r, Some(AudioStart::Pick))
            }
            Collectible::Food | Collectible::Seeds => {
                let a = wave(u);
                let t = rest_pos
                    + Vec3::new(
                        -0.15 * a, // toward centre
                        0.25 * a,  // up to mouth
                        1.7 * a,   // closer to camera
                    );
                let r = rest_rot * Quat::from_euler(EulerRot::YXZ, 0.15 * a, -0.10 * a, 0.10 * a);
                (t, r, Some(AudioStart::Pop))
            }
            Collectible::Button => {
                let a = button_press_ease(u);
                // move down
                let t = rest_pos + Vec3::new(0., -0.12 * a, 0.);
                (t, rest_rot, None)
            }
        };
        if timer.0.just_finished() {
            if let Some(au) = audio {
                audio_event.write(au);
            }
        } else if timer.0.finished() {
            transform.translation = rest_pos;
            transform.rotation = rest_rot;
            continue;
        }

        transform.translation = translation;
        transform.rotation = rotation;
    }
}

// `RemoveTimer` owns its own [`Timer`] because it will be inserted in a
// existing Entity, presumably with a [`TimerComp`] already that would
// clash with it already.
#[derive(Component)]
pub struct RemoveTimer {
    timer: Timer,
    emoji_strength: f32,
}
/// Marker for shrinking a [`Transform`] at the end of the timer.
#[derive(Component)]
struct ShrinkAnimation {
    timer: Timer,
}

impl RemoveTimer {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(0.65 + TOOL_ANIM_TIME, TimerMode::Once),
            emoji_strength: 0.65 + TOOL_ANIM_TIME,
        }
    }
    pub fn with_emoji(strength: f32) -> Self {
        {
            Self {
                timer: Timer::from_seconds(0.65 + TOOL_ANIM_TIME, TimerMode::Once),
                emoji_strength: strength,
            }
        }
    }
}

/// Decrease in life (which can only be decreased) -> small scale decrease.
///
/// If life == 0, add a [`RemoveTimer`] that will play an animation and despawn
/// the entity afterwards at [`remove_animation`].
fn remove_when_life_depleted(
    mut commands: Commands,
    mut secret_rev: ResMut<SecretRevealed>,
    mut lifes: Query<(Entity, &mut Life, Option<&mut GnomeMachine>), Changed<Life>>,
) {
    for (entity, mut life, maybe_gnome) in lifes.iter_mut() {
        if let Life::Left(count) = life.as_ref() {
            let not_gnome = maybe_gnome.is_none();
            if count <= &0 {
                if let Some(mut gnome) = maybe_gnome {
                    gnome.next_state();
                    // if a gnome is hitted, the secret is revealed
                    secret_rev.0 = true;
                } else {
                    commands.entity(entity).insert(RemoveTimer::with_emoji(4.));
                }
            }
            if count < &3 && not_gnome {
                commands.entity(entity).insert(ShrinkAnimation {
                    timer: Timer::from_seconds(TOOL_ANIM_TIME, TimerMode::Once),
                });
            }
        } else {
            *life = Life::Left(1);
        }
    }
}

fn remove_animation(
    mut commands: Commands,
    mut emoji_event: EventWriter<EmojiBurst>,
    mut audio_event: EventWriter<AudioStart>,
    time: Res<Time>,
    mut to_remove: Query<(Entity, &mut Transform, &mut RemoveTimer)>,
) {
    for (ent, mut trans, mut rm_timer) in &mut to_remove {
        if rm_timer.timer.just_finished() {
            commands.entity(ent).despawn();
            audio_event.write(AudioStart::PopOut);
            // celebrate
            emoji_event.write(EmojiBurst {
                velocity: 2000.,
                percent: 0.05 * rm_timer.emoji_strength,
            });
        } else {
            rm_timer.timer.tick(time.delta());
            let u = rm_timer.timer.fraction();
            // wait for tool animation to finish
            if u >= TOOL_ANIM_TIME {
                let u = (u - TOOL_ANIM_TIME) / (1. - TOOL_ANIM_TIME);
                trans.scale = (1. - u) * Vec3::ONE + u * Vec3::ZERO;
                trans.rotation *= Quat::from_rotation_y(0.2);
            }
        }
    }
}

fn animate_shrink(time: Res<Time>, mut query: Query<(&mut Transform, &mut ShrinkAnimation)>) {
    for (mut trans, mut shrink) in &mut query {
        shrink.timer.tick(time.delta());
        if shrink.timer.just_finished() {
            trans.scale *= 0.9;
        }
    }
}
