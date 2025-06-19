//! FPS-like pointer with a raycast coming from the view.
//!
//! This module handles all interactions when clicking the mouse.

use crate::{
    config::{GameState, INTERACTION_DISTANCE},
    digging::{Collectible, Diggable, Life, Minable, OnHand, RemoveTimer, SeedsPlaced},
    player_movement::{Collider, Player},
    surveillance::ButtonActivated,
    world_timer::{ShowOnAlarmTime, TimerComp},
};
use bevy::{
    ecs::{archetype::ArchetypeId, query::QueryEntityError},
    prelude::*,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::FilterQueryInspectorPlugin};

pub struct FirstPersonPickerPlugin;

impl Plugin for FirstPersonPickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameState::Menu), spawn_cross)
            .add_systems(OnEnter(GameState::Menu), despawn_cross)
            .init_resource::<Inventory>()
            .add_event::<DropTool>()
            .add_systems(
                Update,
                cast_player_ray.run_if(not(in_state(GameState::Menu))),
            )
            .add_systems(
                Update,
                (manage_inventory, eat_food, leave_tools_proper).run_if(in_state(GameState::Above)),
            );
        if cfg!(debug_assertions) {
            app.init_gizmo_group::<MyRoundGizmos>()
                .add_plugins(EguiPlugin {
                    enable_multipass_for_primary_context: true,
                })
                .add_plugins(FilterQueryInspectorPlugin::<With<ShowOnAlarmTime>>::default())
                .add_systems(Update, (activate_gizmos, draw_collider_gizmos));
        }
    }
}

#[derive(Component)]
pub struct RayBlocker;

/// Check if the `child` has a parent with the correct Component.
///
/// For minables, the hierarcy becomes a bit complex, so it's walked
/// recursively.
fn filter_recursive(
    child: Entity,
    inv: &Inventory,
    diggables: Query<Entity, With<Diggable>>,
    minables: &Query<&mut Life, With<Minable>>,
    children: Query<&ChildOf>,
    collectables: &Query<(
        Entity,
        &mut Transform,
        &Collectible,
        &mut OnHand,
        &mut TimerComp,
    )>,
    blockers: Query<&RayBlocker>,
) -> bool {
    if blockers.contains(child) {
        return true;
    }
    if let Ok(e) = children.get(child) {
        let contains = match inv {
            Inventory::Shovel(_) | Inventory::Seeds(_) => diggables.contains(e.0),
            Inventory::MiningPick => find_recursive_minable(e.0, minables, children),
            _ => false,
        } || collectables.contains(e.0);
        contains
    } else {
        false
    }
}

fn find_recursive_minable<'a>(
    trigger: Entity,
    minables: &Query<&mut Life, With<Minable>>,
    children: Query<&ChildOf>,
) -> bool {
    if minables.contains(trigger) {
        return true;
    }
    if let Ok(e) = children.get(trigger) {
        find_recursive_minable(e.0, minables, children)
    } else {
        false
    }
}

fn get_recursive_minable<'a>(
    trigger: Entity,
    minables: &'a mut Query<&mut Life, With<Minable>>,
    children: Query<&ChildOf>,
) -> Result<Mut<'a, Life>, QueryEntityError> {
    if minables.contains(trigger) {
        return minables.get_mut(trigger);
    }
    if let Ok(e) = children.get(trigger) {
        get_recursive_minable(e.0, minables, children)
    } else {
        Err(QueryEntityError::QueryDoesNotMatch(
            trigger,
            ArchetypeId::EMPTY,
        ))
    }
}

struct CooldownTimer(Timer);

impl Default for CooldownTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.8, TimerMode::Once))
    }
}

/// Pick click interactions and act accordingly.
fn cast_player_ray(
    time: Res<Time>,
    mut commands: Commands,
    mut seeds_event: EventWriter<SeedsPlaced>,
    mut button_event: EventWriter<ButtonActivated>,
    mut drop_tool: EventWriter<DropTool>,
    mut inventory: ResMut<Inventory>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut ray_cast: MeshRayCast,
    player_query: Single<(&GlobalTransform, Entity, Option<&Children>), With<Player>>,
    mut cross_q: Single<
        (
            &mut BorderColor,
            &mut BackgroundColor,
            &mut Outline,
            &mut Node,
        ),
        With<Cross>,
    >,
    diggables: Query<Entity, With<Diggable>>,
    mut minables: Query<&mut Life, With<Minable>>,
    mut collectables: Query<(
        Entity,
        &mut Transform,
        &Collectible,
        &mut OnHand,
        &mut TimerComp,
    )>,
    blockers: Query<&RayBlocker>,
    children: Query<&ChildOf>,
    mut interaction_cooldown: Local<CooldownTimer>,
) {
    if !interaction_cooldown.0.finished() {
        interaction_cooldown.0.tick(time.delta());
        return;
    }
    // Cast an automatically moving ray and bounce it off of surfaces
    let ray_pos = player_query.0.translation();
    let ray_dir = player_query.0.forward();
    let ray = Ray3d::new(ray_pos, ray_dir);
    let this_filter = |e| {
        filter_recursive(
            e,
            inventory.as_ref(),
            diggables,
            &minables,
            children,
            &collectables,
            blockers,
        )
    };

    if let Some((child, hit)) = ray_cast
        .cast_ray(
            ray,
            &MeshRayCastSettings::default()
                .with_filter(&this_filter)
                .always_early_exit(),
        )
        .first()
    {
        if hit.distance < INTERACTION_DISTANCE {
            let Ok(trigger_parent) = children.get(*child) else {
                return;
            };
            let trigger = &trigger_parent.0;
            if blockers.contains(*child) || blockers.contains(*trigger) {
                *cross_q.0 = BorderColor(Color::srgba(0., 0., 0., 0.2));
                *cross_q.1 = BackgroundColor(Color::srgba(0., 0., 0., 0.2));
                cross_q.2.color = Color::srgba(0., 0., 0., 0.2);
                cross_q.3.height = Val::Vh(1.);
                cross_q.3.width = Val::Vh(1.);
                return;
            }
            *cross_q.0 = BorderColor(Color::srgba(0.3, 1.0, 0.4, 0.7));
            *cross_q.1 = BackgroundColor(Color::srgba(0.3, 1.0, 0.4, 0.7));
            cross_q.2.color = Color::srgba(0.3, 1.0, 0.4, 0.7);
            cross_q.3.height = Val::Vh(2.);
            cross_q.3.width = Val::Vh(2.);
            if mouse_button_input.just_pressed(MouseButton::Left) {
                if let Ok((ent, mut transform, collectible, mut on_hand, _)) =
                    collectables.get_mut(*trigger)
                {
                    let (_, player_ent, on_hand_children) = player_query.into_inner();
                    // despawn currently held tool if any
                    let usage = inventory.get_usage();
                    *inventory = match collectible {
                        Collectible::Shovel(count) => Inventory::Shovel(*count),
                        Collectible::MiningPick => Inventory::MiningPick,
                        Collectible::Food => Inventory::Food(1),
                        Collectible::Seeds => Inventory::Seeds(1),
                        Collectible::Button => {
                            button_event.write(ButtonActivated);
                            return;
                        }
                    };
                    if let Some(to_drop) = on_hand_children {
                        for on_hand_child in to_drop {
                            commands
                                .entity(player_ent)
                                .remove_children(&[*on_hand_child]);
                            drop_tool.write(DropTool {
                                tool: *on_hand_child,
                                drop_position: ray_pos,
                                usage,
                            });
                        }
                    }
                    let (rest_pos, rest_rot) = collectible.on_hand_poses();
                    // the children its the mesh, the translation is better
                    // applied to the parent object in the gltf since it has
                    // absolute coordinates
                    transform.translation = rest_pos;
                    transform.rotation = rest_rot;

                    commands.entity(player_ent).insert_children(4, &[ent]);
                    on_hand.active = true;
                    return;
                }
                inventory.decrease();
                // start animation player for hand tool
                for (_, _, _, on_hand, mut timer) in collectables.iter_mut() {
                    if on_hand.active {
                        timer.0.unpause();
                        timer.0.reset();
                        interaction_cooldown.0.reset();
                    } else {
                        continue;
                    }
                    if diggables.contains(*trigger) && inventory.is_seeds() {
                        seeds_event.write(SeedsPlaced {
                            hit_position: hit.point,
                        });
                        return;
                    }

                    // decrease life or despawn object
                    if let Ok(mut mined_life) =
                        get_recursive_minable(*trigger, &mut minables, children)
                    {
                        if let Life::Left(count) = mined_life.as_mut() {
                            if *count > 0 {
                                *count -= 1;
                            }
                        } else {
                            *mined_life = Life::Left(1);
                        }
                    } else {
                        commands.entity(*trigger).insert(RemoveTimer::new());
                    }
                }
            }
        } else {
            *cross_q.0 = BorderColor(Color::srgba(0., 0., 0., 0.2));
            *cross_q.1 = BackgroundColor(Color::srgba(0., 0., 0., 0.2));
            cross_q.2.color = Color::srgba(0., 0., 0., 0.2);
            cross_q.3.height = Val::Vh(1.);
            cross_q.3.width = Val::Vh(1.);
        }
    } else {
        *cross_q.0 = BorderColor(Color::srgba(0., 0., 0., 0.2));
        *cross_q.1 = BackgroundColor(Color::srgba(0., 0., 0., 0.2));
        cross_q.2.color = Color::srgba(0., 0., 0., 0.2);
        cross_q.3.height = Val::Vh(1.);
        cross_q.3.width = Val::Vh(1.);
    }
}

/// Only one item can be held at a time.
///
/// This is not perfect since we have to pass state
/// from [`Collectible`] when picked and back to it when dropped,
/// probably it would better to not have any Inventory.
#[derive(Resource, PartialEq)]
enum Inventory {
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
impl Inventory {
    fn decrease(&mut self) {
        match self {
            &mut Inventory::Shovel(ref mut counter)
            | &mut Inventory::Seeds(ref mut counter)
            | &mut Inventory::Food(ref mut counter) => {
                if *counter > 0 {
                    *counter -= 1;
                }
            }
            _ => (),
        }
    }
    pub fn is_seeds(&self) -> bool {
        match self {
            &Inventory::Seeds(_) => true,
            _ => false,
        }
    }
    fn get_usage(&self) -> usize {
        match self {
            &Inventory::Shovel(count) => count,
            _ => 1,
        }
    }
}

#[derive(Component)]
struct Cross;

fn spawn_cross(mut commands: Commands) {
    commands.spawn((
        Node {
            display: Display::Block,
            position_type: PositionType::Absolute,
            top: Val::Vh(50.),
            right: Val::Vw(50.),
            height: Val::Vh(1.),
            width: Val::Vh(1.),
            ..default()
        },
        Cross,
        BorderRadius::MAX,
        BorderColor(Color::srgba(0., 0., 0., 0.2)),
        Outline {
            width: Val::Px(3.),
            offset: Val::Px(3.),
            color: Color::srgba(0., 0., 0., 0.2),
        },
        BackgroundColor(Color::srgba(0., 0., 0., 0.2)),
    ));
}

fn despawn_cross(mut commands: Commands, cross: Single<Entity, With<Cross>>) {
    commands.entity(cross.entity()).despawn();
}

/// Check changes for inventory and update accordingly (shovel broken, etc.).
fn manage_inventory(
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    collectables: Query<(Entity, &mut Collectible, &OnHand)>,
) {
    if inventory.is_changed() {
        let check_for = match inventory.as_mut() {
            Inventory::Shovel(counter) if (*counter <= 0) => Collectible::Shovel(0), // counter is irrelevant since it is ignore in `PartialEq<Collectible>`
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

fn eat_food(mut inventory: ResMut<Inventory>, mouse_button_input: Res<ButtonInput<MouseButton>>) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        match inventory.as_mut() {
            &mut Inventory::Food(ref mut counter) => {
                if *counter > 0 {
                    *counter -= 1
                } else {
                    return;
                }
            }
            _ => (),
        }
    }
}

#[derive(Event)]
struct DropTool {
    tool: Entity,
    drop_position: Vec3,
    usage: usize,
}

fn leave_tools_proper(
    mut commands: Commands,
    mut ev: EventReader<DropTool>,
    mut tool_query: Query<(&mut Transform, &ChildOf, &mut OnHand, &mut Collectible)>,
) {
    for DropTool {
        tool,
        drop_position,
        usage,
    } in ev.read()
    {
        if let Ok((mut trans, parent, mut on_hand, mut collectible)) = tool_query.get_mut(*tool) {
            commands.entity(parent.0).remove_children(&[*tool]);
            trans.translation = drop_position.with_y(1.);
            on_hand.active = false;
            match collectible.as_ref() {
                Collectible::Shovel(_) => *collectible = Collectible::Shovel(*usage),
                _ => (),
            };
        }
    }
}

// Gizmos to debug colliders.
#[derive(Default, Reflect, GizmoConfigGroup)]
struct MyRoundGizmos;

/// Debug cheats.
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
