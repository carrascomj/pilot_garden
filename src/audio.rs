//! Audio controllers.

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::{GameOverRemove, dodgy::GaussianNoise};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AudioStart>()
            .add_systems(Startup, load_assets)
            .add_systems(Update, play_audio);
    }
}

#[derive(Clone, Message)]
pub enum AudioStart {
    Bush,
    Fence,
    Rock,
    Laser,
    Cheering,
    DrumChase,
    GnomeDying,
    Goat,
    Pick,
    Pop,
    PopOut,
    Secret,
    Shovel,
    UhOh,
    Tock,
    SwitchOn,
    SwitchOff,
    ShovelBroken,
}

#[derive(Resource)]
struct AudioAssets {
    bush: Handle<AudioSource>,
    rock: Handle<AudioSource>,
    fence: Handle<AudioSource>,
    laser: Handle<AudioSource>,
    cheering: Handle<AudioSource>,
    drum_chase: Handle<AudioSource>,
    gnome_dying: Handle<AudioSource>,
    goat: Handle<AudioSource>,
    pop: Handle<AudioSource>,
    pop_out: Handle<AudioSource>,
    shovel: Handle<AudioSource>,
    pick: Handle<AudioSource>,
    secret: Handle<AudioSource>,
    tock: Handle<AudioSource>,
    uhoh: Handle<AudioSource>,
    switch_on: Handle<AudioSource>,
    switch_off: Handle<AudioSource>,
    shovel_broken: Handle<AudioSource>,
}

fn load_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(AudioAssets {
        bush: asset_server.load("sfx/bush.ogg"),
        rock: asset_server.load("sfx/bounce_rock.mp3"),
        fence: asset_server.load("sfx/bounce_fence.ogg"),
        laser: asset_server.load("sfx/laser.ogg"),
        cheering: asset_server.load("sfx/cheering.ogg"),
        drum_chase: asset_server.load("sfx/chas_drums.ogg"),
        gnome_dying: asset_server.load("sfx/gnome_dying.ogg"),
        goat: asset_server.load("sfx/goat.ogg"),
        pop: asset_server.load("sfx/pop.mp3"),
        pop_out: asset_server.load("sfx/pop-out.mp3"),
        shovel: asset_server.load("sfx/shovel.ogg"),
        secret: asset_server.load("sfx/secret.mp3"),
        tock: asset_server.load("sfx/tock.ogg"),
        pick: asset_server.load("sfx/pick_axe.ogg"),
        uhoh: asset_server.load("sfx/uhoh.mp3"),
        switch_on: asset_server.load("sfx/switch_on.ogg"),
        switch_off: asset_server.load("sfx/switch_off.ogg"),
        shovel_broken: asset_server.load("sfx/shovel_break.mp3"),
    });
}

#[derive(Component)]
pub struct DrumsToStop;

fn play_audio(
    mut commands: Commands,
    mut audio_triggers: MessageReader<AudioStart>,
    sound_assets: Res<AudioAssets>,
    mut gaussian: ResMut<GaussianNoise>,
) {
    for trigger in audio_triggers.read() {
        let audio = match trigger {
            AudioStart::Bush => sound_assets.bush.clone(),
            AudioStart::Fence => sound_assets.fence.clone(),
            AudioStart::Rock => sound_assets.rock.clone(),
            AudioStart::Cheering => sound_assets.cheering.clone(),
            AudioStart::Laser => sound_assets.laser.clone(),
            AudioStart::GnomeDying => sound_assets.gnome_dying.clone(),
            AudioStart::Goat => sound_assets.goat.clone(),
            AudioStart::Pick => sound_assets.pick.clone(),
            AudioStart::Pop => sound_assets.pop.clone(),
            AudioStart::PopOut => sound_assets.pop_out.clone(),
            AudioStart::Secret => sound_assets.secret.clone(),
            AudioStart::Shovel => sound_assets.shovel.clone(),
            AudioStart::UhOh => sound_assets.uhoh.clone(),
            AudioStart::SwitchOn => sound_assets.switch_on.clone(),
            AudioStart::SwitchOff => sound_assets.switch_off.clone(),
            AudioStart::Tock => sound_assets.tock.clone(),
            AudioStart::ShovelBroken => sound_assets.shovel_broken.clone(),
            AudioStart::DrumChase => {
                commands.spawn((
                    AudioPlayer::<AudioSource>(sound_assets.drum_chase.clone()),
                    PlaybackSettings::LOOP,
                    DrumsToStop,
                    GameOverRemove,
                ));
                continue;
            }
        };
        commands.spawn((
            AudioPlayer::<AudioSource>(audio),
            // we cannot set the pitch directly, but we can use the speed
            // for a similar effect, to avoid being too repetitive
            PlaybackSettings::DESPAWN.with_speed((gaussian.sample() * 0.3 + 1.).clamp(0.7, 1.4)),
        ));
    }
}
