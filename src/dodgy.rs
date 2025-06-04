//! Game mechanics for entities that hide when the player is looking.

use std::ops::Deref;

use bevy::{prelude::*, render::view::VisibilitySystems};

use crate::digging::{Minable, remove_on_click};
use crate::player_movement::{Collider, Player};

pub struct DodgyPlugin;

impl Plugin for DodgyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (animate_dodge,)).add_systems(
            PostUpdate,
            activate_dodge.after(VisibilitySystems::CheckVisibility),
        );

        if cfg!(debug_assertions) {
            app.add_systems(Startup, spawn_bananon);
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
        ndc.x.abs() <= 2.0 && ndc.y.abs() <= 2.0 && ndc.z >= 0.0 && ndc.z <= 2.0
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

fn spawn_bananon(mut commands: Commands, asset_server: Res<AssetServer>) {
    let init_trans = Vec3::new(20.0, -5.0, -5.0);
    let last_trans = Vec3::new(20.0, -0.2, -5.0);
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
            SceneRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("banana.gltf#bananon")),
            ),
        ))
        .observe(remove_on_click);
}
