//! Game mechanic for a timer that forces the player to go to sleep.

use std::time::Duration;

use bevy::prelude::*;

/// Introduces a global timer that makes the day turn into night.
/// At night, the player is hinted to go to the capsule and start again.
///
/// Also, centralizes all timer advancement.
pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, advance_timers);
    }
}

#[derive(Component)]
struct WorldTimer;

#[derive(Component)]
pub struct TimerComp(pub Timer);

impl TimerComp {
    pub fn from_elapsed(secs: f32) -> Self {
        let dur = Duration::from_secs_f32(secs);
        let mut timer = Timer::new(dur, TimerMode::Once);
        timer.set_elapsed(dur);
        Self(timer)
    }
}

fn advance_timers(time: Res<Time>, mut timers: Query<&mut TimerComp>) {
    for mut timer in timers.iter_mut() {
        timer.0.tick(time.delta());
    }
}
