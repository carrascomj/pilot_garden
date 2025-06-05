//! Game mechanics for entities that hide when the player is looking.

use std::ops::Deref;

use bevy::{prelude::*, render::view::VisibilitySystems};

use crate::config::{MAX_CROP_BOUNDS, MIN_CROP_BOUNDS};
use crate::digging::{Life, Minable, SeedsPlaced, remove_on_click};
use crate::player_movement::{Collider, Player};
use fastrand::Rng;
use smallvec;

pub struct DodgyPlugin;

impl Plugin for DodgyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (animate_dodge, animate_arch, plant_bananite_on_seeds),
        )
        .add_systems(
            PostUpdate,
            activate_dodge.after(VisibilitySystems::CheckVisibility),
        )
        .init_resource::<GaussianNoise>()
        .add_observer(spawn_banana_on_bananite_depletion);

        if cfg!(debug_assertions) {
            app.add_systems(Startup, spawn_bananite);
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
struct Dodgy {
    init_pos: Vec3,
    last_pos: Vec3,
    timer: Timer,
    /// If true: go back to `init_pos` before the user looks at it.
    go_back: bool,
}

fn point_in_view(camera: &Camera, cam_tf: &GlobalTransform, world_pos: Vec3) -> bool {
    if let Some(ndc) = camera.world_to_ndc(cam_tf, world_pos) {
        ndc.x.abs() <= 1.0 && ndc.y.abs() <= 10.0 && ndc.z >= 0.0 && ndc.z <= 1.0
    } else {
        false // world_to_ndc returns None when the point is behind the camera
    }
}

fn activate_dodge(
    time: Res<Time>,
    camera: Single<(&Camera, &GlobalTransform), With<Player>>,
    mut dodgers: Query<(&GlobalTransform, &mut Dodgy)>,
) {
    let (cam, gt_cam) = camera.deref();
    for (gt, mut dodger) in &mut dodgers {
        dodger.timer.tick(time.delta());
        let visible = point_in_view(cam, gt_cam, gt.translation());
        if visible {
            // println!("seeing");
            dodger.timer.pause();
            if dodger.go_back {
                dodger.timer.reset();
            }
        } else {
            // println!("not in viewport");
            dodger.timer.unpause()
        }
    }
}

fn animate_dodge(mut dodgers: Query<(&mut Transform, &Dodgy)>) {
    for (mut trans, dodger) in &mut dodgers {
        if !dodger.timer.finished() && !dodger.timer.paused() {
            let u = dodger.timer.fraction();
            trans.translation = u * dodger.last_pos + (1. - u) * dodger.init_pos;
        }
    }
}

/// Marks an entity to have an arch animation on spawn.
/// Animation at [`animate_arch`].
#[derive(Component)]
struct ArchAnimation {
    timer: Timer,
    init_pos: Vec3,
    last_pos: Vec3,
}

/// Spawn some bananas once a bananite is depleted.
///
/// (A bananite is a banana ore.)
fn spawn_banana_on_bananite_depletion(
    trigger: Trigger<OnRemove, Minable>,
    asset_server: Res<AssetServer>,
    mut gaussian: ResMut<GaussianNoise>,
    mut commands: Commands,
    bananite_query: Query<&Transform>,
) {
    let entity = trigger.target();
    if let Ok(trans) = bananite_query.get(entity) {
        let mut init_pos = trans.translation;
        init_pos.y = 0.3;
        const MAX_BANANAS: usize = 4;
        let bananas = (0..gaussian.rng.u8(3..(MAX_BANANAS as u8)))
            .map(|_| {
                let x_offset = gaussian.sample() * 2.0;
                let z_offset = gaussian.sample() * 2.0;
                let last_pos = (trans.translation + Vec3::new(x_offset, init_pos.y, z_offset))
                    .clamp(MIN_CROP_BOUNDS, MAX_CROP_BOUNDS);
                let timer = Timer::from_seconds(0.5, TimerMode::Once);
                (
                    Transform::from_translation(init_pos),
                    ArchAnimation {
                        timer,
                        init_pos,
                        last_pos,
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

fn animate_arch(time: Res<Time>, mut dodgers: Populated<(&mut Transform, &mut ArchAnimation)>) {
    for (mut trans, mut arch) in dodgers.iter_mut() {
        if !arch.timer.finished() && !arch.timer.paused() {
            let u = arch.timer.fraction();
            let mut next_translation = u * arch.last_pos + (1. - u) * arch.init_pos;
            next_translation.y = arch_bezier(arch.init_pos.y, arch.last_pos.y, 5.0, u);
            trans.translation = next_translation;
            arch.timer.tick(time.delta());
        } else if arch.timer.just_finished() {
            trans.translation = arch.last_pos;
        }
    }
}

fn spawn_bananite(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
) {
    if keyboard_input.pressed(KeyCode::Digit7) {
        for (x, y) in [(20.0, -2.0), (20.0, -5.0), (25.0, -5.0), (25.0, -2.0)] {
            let init_trans = Vec3::new(x, -5.0, y);
            let last_trans = Vec3::new(x, -0.2, y);
            commands
                .spawn((
                    Dodgy {
                        init_pos: init_trans,
                        last_pos: last_trans,
                        timer: Timer::from_seconds(2.5, TimerMode::Once),
                        go_back: false,
                    },
                    Transform::from_translation(init_trans),
                    Minable {},
                    Collider::from_translation(last_trans, Vec3::new(1.0, 4.0, 1.0)),
                    Life { left: 3 },
                    SceneRoot(
                        asset_server
                            .load(GltfAssetLabel::Scene(0).from_asset("bananite.gltf#bananite")),
                    ),
                ))
                .observe(remove_on_click);
        }
    }
}

fn plant_bananite_on_seeds(
    mut commands: Commands,
    mut seeds_event: EventReader<SeedsPlaced>,
    asset_server: Res<AssetServer>,
) {
    for ev in seeds_event.read() {
        let Vec3 { x, y: _, z } = ev
            .hit_position
            .clamp(MIN_CROP_BOUNDS + 0.5, MAX_CROP_BOUNDS - 0.5);
        let init_trans = Vec3::new(x, -5.0, z);
        let last_trans = Vec3::new(x, -0.2, z);
        commands
            .spawn((
                Dodgy {
                    init_pos: init_trans,
                    last_pos: last_trans,
                    timer: Timer::from_seconds(3.0, TimerMode::Once),
                    go_back: false,
                },
                Transform::from_translation(init_trans),
                Minable {},
                Collider::from_translation(last_trans, Vec3::new(1.0, 4.0, 1.0)),
                Life { left: 3 },
                SceneRoot(
                    asset_server
                        .load(GltfAssetLabel::Scene(0).from_asset("bananite.gltf#bananite")),
                ),
            ))
            .observe(remove_on_click);
    }
}
