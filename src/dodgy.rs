//! Game mechanics for entities that hide when the player is looking.

use std::ops::Deref;

use bevy::{prelude::*, render::view::VisibilitySystems};

use crate::config::{GameState, MAX_CROP_BOUNDS, MIN_CROP_BOUNDS};
use crate::digging::{Life, Minable, SeedsPlaced};
use crate::player_movement::{Collider, Player};
use crate::world_timer::TimerComp;
use fastrand::Rng;
use smallvec;

pub struct DodgyPlugin;

impl Plugin for DodgyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (animate_dodge, animate_arch, plant_bananite_on_seeds)
                .run_if(in_state(GameState::Above)),
        )
        .add_systems(
            PostUpdate,
            activate_dodge
                .after(VisibilitySystems::CheckVisibility)
                .run_if(in_state(GameState::Above)),
        )
        .init_resource::<GaussianNoise>()
        .add_observer(spawn_banana_on_bananite_depletion);

        if cfg!(debug_assertions) {
            app.add_systems(Update, spawn_bananite);
        }
    }
}

#[derive(Resource)]
struct GaussianNoise {
    rng: Rng,
}

impl Default for GaussianNoise {
    fn default() -> Self {
        Self {
            rng: fastrand::Rng::new(),
        }
    }
}

impl GaussianNoise {
    /// Marsaglia’s polar method for standard normal.
    fn sample(&mut self) -> f32 {
        loop {
            // fastrand::f64() gives you a uniform [0, 1) double
            let u1 = 2.0 * self.rng.f32() - 1.0; // uniform (-1,1)
            let u2 = 2.0 * self.rng.f32() - 1.0; // uniform (-1,1)
            let s = u1 * u1 + u2 * u2;
            if s == 0.0 || s >= 1.0 {
                continue;
            }
            // Only one ln/sqrt per accepted point:
            let factor = (-2.0 * s.ln() / s).sqrt();
            return u1 * factor; // one N(0,1)
        }
    }
}

/// Mark entities so that they move towards last_pos while the user is not looking.
#[derive(Component)]
#[require(TimerComp(Timer::from_seconds(2.5, TimerMode::Once)))]
pub struct Dodgy {
    pub init_pos: Vec3,
    pub last_pos: Vec3,
    /// If true: go back to `init_pos` before the user looks at it.
    pub go_back: bool,
    pub ignore_viewing: bool,
}

fn point_in_view(camera: &Camera, cam_tf: &GlobalTransform, world_pos: Vec3) -> bool {
    if let Some(ndc) = camera.world_to_ndc(cam_tf, world_pos) {
        ndc.x.abs() <= 1.0 && ndc.y.abs() <= 10.0 && ndc.z >= 0.0 && ndc.z <= 1.0
    } else {
        false // world_to_ndc returns None when the point is behind the camera
    }
}

fn activate_dodge(
    camera: Single<(&Camera, &GlobalTransform), With<Player>>,
    mut dodgers: Query<(&GlobalTransform, &Dodgy, &mut TimerComp)>,
) {
    let (cam, gt_cam) = camera.deref();
    for (gt, dodger, mut timer) in &mut dodgers {
        let visible = point_in_view(cam, gt_cam, gt.translation());
        if visible && !dodger.ignore_viewing {
            timer.0.pause();
            if dodger.go_back {
                timer.0.reset();
            }
        } else {
            timer.0.unpause()
        }
    }
}

fn animate_dodge(mut dodgers: Query<(&mut Transform, &Dodgy, &TimerComp)>) {
    for (mut trans, dodger, timer) in &mut dodgers {
        if !timer.0.finished() && !timer.0.paused() {
            let u = timer.0.fraction();
            trans.translation = u * dodger.last_pos + (1. - u) * dodger.init_pos;
        }
    }
}

/// Marks an entity to have an arch animation on spawn.
/// Animation at [`animate_arch`].
#[derive(Component)]
#[require(TimerComp(Timer::from_seconds(0.5, TimerMode::Once)))]
pub struct ArchAnimation {
    pub init_pos: Vec3,
    pub last_pos: Vec3,
    pub peak_y: f32,
}

/// Spawn some bananas once a bananite is depleted.
///
/// (A bananite is a banana ore.)
fn spawn_banana_on_bananite_depletion(
    trigger: Trigger<OnRemove, Bananite>,
    asset_server: Res<AssetServer>,
    mut gaussian: ResMut<GaussianNoise>,
    mut commands: Commands,
    bananite_query: Query<&Transform>,
) {
    let entity = trigger.target();
    if let Ok(trans) = bananite_query.get(entity) {
        let init_pos = trans.translation;
        let peak_y = 5.0;
        const MAX_BANANAS: usize = 4;
        let bananas = (0..gaussian.rng.u8(3..(MAX_BANANAS as u8)))
            .map(|_| {
                let offset = Vec3::new(gaussian.sample() * 2.0, 0., gaussian.sample() * 2.0);
                let last_pos = (trans.translation + offset)
                    .with_y(0.)
                    .clamp(MIN_CROP_BOUNDS, MAX_CROP_BOUNDS);
                (
                    Transform::from_translation(init_pos),
                    ArchAnimation {
                        init_pos,
                        last_pos,
                        peak_y,
                    },
                    SceneRoot(
                        asset_server.load(GltfAssetLabel::Scene(0).from_asset("banana.gltf")),
                    ),
                )
            })
            .collect::<smallvec::SmallVec<[_; MAX_BANANAS]>>();
        commands.spawn_batch(bananas);
    }
}

fn arch_bezier(from: f32, to: f32, peak: f32, u: f32) -> f32 {
    let one_minus_u = 1.0 - u;
    one_minus_u * one_minus_u * from + 2.0 * one_minus_u * u * peak + u * u * to
}

fn animate_arch(mut dodgers: Populated<(&mut Transform, &ArchAnimation, &TimerComp)>) {
    for (mut trans, arch, timer) in dodgers.iter_mut() {
        if !timer.0.finished() && !timer.0.paused() {
            let u = timer.0.fraction();
            let mut next_translation = u * arch.last_pos + (1. - u) * arch.init_pos;
            next_translation.y = arch_bezier(arch.init_pos.y, arch.last_pos.y, arch.peak_y, u);
            trans.translation = next_translation;
        } else {
            trans.translation = arch.last_pos;
        }
    }
}

#[derive(Component)]
#[require(Minable{}, Life::Left(3))]
struct Bananite;

fn spawn_bananite(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
) {
    if keyboard_input.just_pressed(KeyCode::Digit7) {
        for (x, y) in [(20.0, -2.0), (20.0, -5.0), (25.0, -5.0), (25.0, -2.0)] {
            let init_trans = Vec3::new(x, -5.0, y);
            let last_trans = Vec3::new(x, -0.2, y);
            commands.spawn((
                Dodgy {
                    init_pos: init_trans,
                    last_pos: last_trans,
                    go_back: false,
                    ignore_viewing: false,
                },
                Transform::from_translation(init_trans),
                Collider::from_translation(last_trans, Vec3::new(1.0, 4.0, 1.0)),
                Bananite,
                SceneRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("bananite.gltf#bananite")),
                ),
            ));
        }
    }
}

fn plant_bananite_on_seeds(
    mut commands: Commands,
    mut seeds_event: EventReader<SeedsPlaced>,
    asset_server: Res<AssetServer>,
) {
    for ev in seeds_event.read() {
        let Vec3 { x, y, z } = ev
            .hit_position
            .clamp(MIN_CROP_BOUNDS + 1.5, MAX_CROP_BOUNDS - 1.5);
        let init_trans = Vec3::new(x, y - 5.0, z);
        let last_trans = Vec3::new(x, y, z);
        commands.spawn((
            Dodgy {
                init_pos: init_trans,
                last_pos: last_trans,
                go_back: false,
                ignore_viewing: false,
            },
            Transform::from_translation(init_trans),
            Bananite,
            Collider::from_translation(last_trans, Vec3::new(1.0, 4.0, 1.0)),
            SceneRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("bananite.gltf#bananite")),
            ),
        ));
    }
}
