//! Mechanics related to Gnomes.

use std::{f32::consts::PI, time::Duration};

use bevy::prelude::*;

use crate::{
    config::GameState,
    digging::{Life, Minable},
    player_movement::Collider,
};

const GNOME_SIZE: Vec3 = Vec3::new(0.5, 1.5, 0.5);

/// Spawn gnomes were relevant and control the state machine,
/// transitions and animations of the gnomes.

pub struct GnomePlugin;

impl Plugin for GnomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_gnomes_above).add_systems(
            Update,
            (setup_animations, change_animation).run_if(not(in_state(GameState::Menu))),
        );
    }
}

/// State machine for the gnomes
#[derive(Component, PartialEq)]
pub enum GnomeMachine {
    Inactive,
    Active,
    LoadingBanana,
    Moving,
    Attacking,
    Dying,
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

    for (x, y, z) in [(20., 0.1, 9.6), (24., -2.2, 25.6)] {
        commands.spawn((
            // TODO: check out https://bevy.org/examples/animation/animated-mesh-control/
            SceneRoot(gnome.clone()),
            HasAnimationChild(None),
            GnomeMachine::Inactive,
            Collider::from_translation(Vec3::new(x, y, z) + Vec3::Y * 0.5, GNOME_SIZE),
            Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(PI)),
            Minable,
            Life::JustSpawned,
        ));
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
                // .repeat();

                commands
                    .entity(entity)
                    .insert(transitions)
                    .insert(AnimationGraphHandle(animations.graph_handle.clone()));
                has_animation_child.0 = Some(entity);
                println!("Setup animation!");
            }
        }
    }
}

fn change_animation(
    animations: Res<Animations>,
    gnomes: Query<(&GnomeMachine, &HasAnimationChild), Changed<GnomeMachine>>,
    mut animation_players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (gnome, has_anim) in gnomes {
        if let Some(anim_entity) = has_anim.0 {
            let anim_index = match gnome {
                GnomeMachine::Dying => 0,
                _ => 1,
            };
            println!("play 1!");
            if let Ok((mut player, mut transitions)) = animation_players.get_mut(anim_entity) {
                println!("play 2!");
                transitions
                    .play(
                        &mut player,
                        animations.animations[anim_index],
                        Duration::from_millis(2),
                    )
                    .replay();
            }
        }
    }
}
