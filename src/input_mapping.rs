//! Systems to map all button inputs (Key, Mouse) to user actions (Jump, Left).

use bevy::input::InputSystem;
use bevy::prelude::*;

/// Introduces a resource that is updated to translate button inputs
/// into their actions.
pub struct InputMappingPlugin;

impl Plugin for InputMappingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputActions>()
            // refresh the mapping every frame after Bevy updates ButtonInput<*>
            .add_systems(PreUpdate, update_pressed_buttons.after(InputSystem));
    }
}

#[derive(Resource, Default)]
/// Recorded action buttons in this frame.
pub struct InputActions {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    /// player triggered interaction with the crosshair
    pub pick: bool,
}

fn update_pressed_buttons(
    mut pressed: ResMut<InputActions>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
) {
    pressed.up = keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp);
    pressed.down =
        keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown);
    pressed.left =
        keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft);
    pressed.right =
        keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight);
    pressed.jump =
        keyboard_input.just_pressed(KeyCode::Space) || mouse_input.just_pressed(MouseButton::Right);
    pressed.pick = mouse_input.just_pressed(MouseButton::Left);
}
