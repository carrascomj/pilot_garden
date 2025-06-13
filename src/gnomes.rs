//! Mechanics related to Gnomes.

use std::time::Duration;

use bevy::prelude::*;

use crate::{
    config::GameState,
    digging::{Life, Minable},
    player_movement::{Collider, Player},
};

const GNOME_SIZE: Vec3 = Vec3::new(0.5, 1.5, 0.5);
// TODO: set this actually right
const GNOME_VELOCITY: f32 = 20.;
const ALERTER_POSITIONS: [Vec2; 3] = [
    Vec2::new(10., 20.),
    Vec2::new(10. - 5., 20.),
    Vec2::new(10. - 5., 20. - 9.),
];

/// Spawn gnomes were relevant and control the state machine,
/// transitions and animations of the gnomes.
pub struct GnomePlugin;

impl Plugin for GnomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_gnomes_above).add_systems(
            Update,
            (setup_animations, move_gnome).run_if(not(in_state(GameState::Menu))),
        );
    }
}

/// State for the gnomes.
#[derive(PartialEq)]
pub enum GnomeState {
    Inactive,
    Active,
    LoadingBanana,
    Moving {
        timer: Timer,
        /// Vec2 since the gnomes do not fly
        spline: CubicCurve<Vec2>,
    },
    Attacking,
    Dying,
}

/// State machine for the gnomes
#[derive(Component)]
pub struct GnomeMachine {
    state: GnomeState,
    is_changed: bool,
}

impl GnomeMachine {
    fn new() -> Self {
        Self {
            state: GnomeState::Inactive,
            is_changed: false,
        }
    }
    pub fn next_state(&mut self) {
        self.state = match self.state {
            GnomeState::Dying | GnomeState::Attacking => return,
            GnomeState::Inactive => GnomeState::Dying,
            GnomeState::LoadingBanana => GnomeState::Moving {
                timer: Timer::from_seconds(7., TimerMode::Once),
                spline: CubicCardinalSpline {
                    tension: 0.1,
                    control_points: ALERTER_POSITIONS.into(),
                }
                .to_curve()
                .expect("Spline failed to resolve."),
            },
            GnomeState::Active => GnomeState::Attacking,
            GnomeState::Moving { .. } => GnomeState::Active,
        };
        self.is_changed = true;
    }
}

#[derive(Resource)]
struct Animations {
    animations: Vec<AnimationNodeIndex>,
    graph_handle: Handle<AnimationGraph>,
}

/// Marker for entities that have an ([`AnimationPlayer`], [`AnimationTransitions`])
/// enitity as a child.
#[derive(Component)]
struct HasAnimationChild(Option<Entity>);
#[derive(Component)]
pub struct PlatformMover;

/// The gnomes above are tame and can only die.
fn spawn_gnomes_above(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    const GNOME_PATH: &str = "gnome.glb";
    let gnome = asset_server.load(GltfAssetLabel::Scene(0).from_asset(GNOME_PATH));

    // save animations in resource to add them later after the gnome scene is loaded
    let (graph, node_indices) = AnimationGraph::from_clips([
        asset_server.load(GltfAssetLabel::Animation(0).from_asset(GNOME_PATH)), // die
        asset_server.load(GltfAssetLabel::Animation(1).from_asset(GNOME_PATH)), // idle
        asset_server.load(GltfAssetLabel::Animation(2).from_asset(GNOME_PATH)), // run
    ]);
    let graph_handle = graphs.add(graph);
    commands.insert_resource(Animations {
        animations: node_indices,
        graph_handle,
    });
    let mut first = true;
    for (x, y, z) in [(27., 0.1, 8.6), (24., -2.2, 25.6)] {
        let mut ent_comm = commands.spawn((
            SceneRoot(gnome.clone()),
            HasAnimationChild(None),
            GnomeMachine::new(),
            Collider::from_translation(Vec3::new(x, y, z) + Vec3::Y * 0.5, GNOME_SIZE),
            Transform::from_xyz(x, y, z),
            Minable,
            Life::JustSpawned,
        ));
        if first {
            ent_comm.insert(PlatformMover);
            first = false;
        }
    }
}

fn setup_animations(
    mut commands: Commands,
    animations: Res<Animations>,
    mut anim_parents: Query<(Entity, &mut HasAnimationChild)>,
    children: Query<&Children>,
    mut anim_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (parent, mut has_animation_child) in anim_parents
        .iter_mut()
        .filter(|(_, has_anim)| has_anim.0.is_none())
    {
        for child in children.iter_descendants(parent) {
            if let Ok((entity, mut player)) = anim_players.get_mut(child) {
                let mut transitions = AnimationTransitions::new();

                // Make sure to start the animation via the `AnimationTransitions`
                // component. The `AnimationTransitions` component wants to manage all
                // the animations and will get confused if the animations are started
                // directly via the `AnimationPlayer`.
                transitions.play(&mut player, animations.animations[1], Duration::ZERO); // start at idle

                commands
                    .entity(entity)
                    .insert(transitions)
                    .insert(AnimationGraphHandle(animations.graph_handle.clone()));
                has_animation_child.0 = Some(entity);
            }
        }
    }
}

/// Control movement of the gnome based on its state.
fn move_gnome(
    time: Res<Time>,
    animations: Res<Animations>,
    mut gnomes: Query<(&mut GnomeMachine, &HasAnimationChild, &mut Transform)>,
    mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    player_transform: Query<&Transform, (With<Player>, Without<GnomeMachine>)>,
) {
    for (mut gnome, has_anim, mut transform) in &mut gnomes {
        // change the animation itself
        if gnome.is_changed {
            if let Some(anim_entity) = has_anim.0 {
                let anim_index = match &mut gnome.state {
                    GnomeState::Dying => 0,
                    GnomeState::Attacking => 2,
                    GnomeState::Moving { timer, spline: _ } => {
                        timer.unpause();
                        2
                    }
                    _ => 1,
                };
                if let Ok((mut player, mut transitions)) = animation_players.get_mut(anim_entity) {
                    let active_animation = transitions.play(
                        &mut player,
                        animations.animations[anim_index],
                        Duration::from_millis(2),
                    );
                    // if running, repeat forever, it is more menacing in Game Over
                    // that the gnomes just keep moving towards the player and the
                    // player won't the see the gnome at the end of `Moving`.
                    if anim_index == 2 {
                        active_animation.repeat();
                    }
                    active_animation.replay();
                }
            }
            gnome.is_changed = false;
        }
        // handle movement (only across x, z axis)
        match &mut gnome.state {
            GnomeState::Moving { timer, spline } => {
                if !timer.finished() {
                    timer.tick(time.delta());
                    let u = timer.fraction(); // [0,1]
                    let t = u * spline.segments().len() as f32;
                    let Vec2 { x, y: z } = spline.position(t);
                    let Vec2 { x: dx, y: dz } = spline.velocity(t);
                    let direction = Vec3::new(dx, 0.0, dz);
                    if direction.length_squared() > 0.01 {
                        let direction = direction.normalize();
                        let next_position = Vec3::new(x, transform.translation.y, z);
                        *transform = transform
                            .with_translation(next_position)
                            .looking_at(next_position + direction, Vec3::Y);
                    }
                }
            }
            GnomeState::Attacking => {
                if let Ok(target) = player_transform.single() {
                    let delta = time.delta().as_secs_f32();
                    let dir = (target.translation - transform.translation).with_y(0.);
                    if dir.length_squared() > 5. {
                        let dir = dir.normalize();
                        transform.translation =
                            transform.translation + (dir * delta * GNOME_VELOCITY);
                        transform.look_to(dir, Vec3::Y);
                    };
                }
            }
            _ => (),
        }
    }
}
