#![allow(deprecated)]

use bevy::camera::{ClearColor, Hdr};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::input::mouse::MouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::Button;
use bevy::window::{CursorGrabMode, CursorOptions, WindowResolution};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink, Source};
use std::f32::consts::PI;

const EYE: f32 = 1.62;
const WATER_Y: f32 = 0.0;
const SAND_TOP: f32 = 0.25;
const WORLD_X: f32 = 60.0;
const BEACH_MIN_Z: f32 = -1.5;
const BEACH_MAX_Z: f32 = 42.0;
const SKY: Color = Color::srgb(0.55, 0.70, 0.90);
const SUN_POS: Vec3 = Vec3::new(70.0, 26.0, -260.0);
const HAMMOCK_POS: Vec3 = Vec3::new(0.0, SAND_TOP, 10.0);
const HAMMOCK_RADIUS: f32 = 3.2;
const BAR_POS: Vec3 = Vec3::new(11.0, SAND_TOP, 19.0);
const BAR_RADIUS: f32 = 3.8;
const REST_CAM: Vec3 = Vec3::new(-1.3, 1.28, 10.0);

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameState { #[default] Playing, Paused }

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum RelaxMode { #[default] Resting, Walking }

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum DrinkKind { #[default] Lemonade, Coffee }

#[derive(Resource, Default)]
struct DrinkState { holding: bool, selected: DrinkKind }

#[derive(Resource)]
struct Settings { show_fps: bool, volume: f32 }
impl Default for Settings {
    fn default() -> Self { Self { show_fps: false, volume: 0.6 } }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InteractKind { Hammock, Bar }

#[derive(Component)] struct Player { yaw: f32, pitch: f32, bob: f32 }
#[derive(Component)] struct Ocean;
#[derive(Component)] struct SunPath;
#[derive(Component)] struct Foam { base_z: f32, phase: f32, speed: f32 }
#[derive(Component)] struct Sparkle { phase: f32 }
#[derive(Component)] struct Steam { base: Vec3, phase: f32 }
#[derive(Component)] struct Sway { base: Vec3, base_rot: Quat, phase: f32 }
#[derive(Component)] struct CloudPuff { speed: f32 }
#[derive(Component)] struct Bird { center: Vec3, radius: f32, speed: f32, phase: f32, height: f32 }
#[derive(Component)] struct Yacht { base: Vec3, phase: f32 }
#[derive(Component)] struct DrinkGroup { kind: DrinkKind }
#[derive(Component)] struct DrinkScale { current: f32 }
#[derive(Component)] struct BarIndicator { kind: DrinkKind }
#[derive(Component)] struct InteractMarker { kind: InteractKind }
#[derive(Component)] struct CharacterPart;
#[derive(Component)] struct FpsText;
#[derive(Component)] struct MenuRoot;
#[derive(Component)] struct VolumeSlider;
#[derive(Component)] struct VolumeSliderHandle;
#[derive(Component)] struct VolumeSliderFill;
#[derive(Component)] struct VolumeLabel;

#[derive(Component)]
enum MenuButton { Resume, FpsToggle, Quit }

#[derive(Component)] struct FpsToggleText;

#[derive(Resource)]
struct Obstacles(Vec<(f32, f32, f32)>);

struct AmbientAudio { _stream: OutputStream, sink: Sink }

#[derive(Resource)]
struct FpsCounter { current: f32, frame_count: u32, time_accum: f32 }
impl Default for FpsCounter {
    fn default() -> Self { Self { current: 0.0, frame_count: 0, time_accum: 0.0 } }
}

fn next_rand(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*rng >> 8) as f32) / 16_777_216.0
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ocean Relax".into(),
                resolution: WindowResolution::new(1920, 1080), // ЦЕЛЫЕ ЧИСЛА!
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(SKY))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.78, 0.72, 0.85),
            brightness: 200.0,
            ..default()
        })
        .init_resource::<RelaxMode>()
        .init_resource::<DrinkState>()
        .init_resource::<Settings>()
        .init_resource::<FpsCounter>()
        .init_state::<GameState>()
        .add_systems(Startup, (setup, setup_ui, setup_audio))
        .add_systems(Update, pause_toggle.run_if(in_state(GameState::Playing)))
        .add_systems(Update, (camera_control, drink_input, drink_animation.after(drink_input)).run_if(in_state(GameState::Playing)))
        .add_systems(Update, (bar_indicator_system, interact_hint_system, character_visibility).run_if(in_state(GameState::Playing)))
        .add_systems(Update, (sway_system, water_system, foam_system).run_if(in_state(GameState::Playing)))
        .add_systems(Update, (sparkle_system, steam_system, cloud_system).run_if(in_state(GameState::Playing)))
        .add_systems(Update, (bird_system, yacht_system).run_if(in_state(GameState::Playing)))
        .add_systems(Update, fps_update_system)
        .add_systems(Update, fps_visibility_system)
        .add_systems(Update, slider_update_system.run_if(in_state(GameState::Paused)))
        .add_systems(Update, menu_button_system.run_if(in_state(GameState::Paused)))
        .run();
}

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("FPS: 0"),
        TextFont { font_size: FontSize::Px(32.0), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: Val::Px(20.0), left: Val::Px(20.0), ..default() },
        FpsText, Visibility::Hidden,
    ));

    commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            MenuRoot, Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((Text::new("PAUSED"), TextFont { font_size: FontSize::Px(64.0), ..default() }, TextColor(Color::WHITE), Node { margin: UiRect::bottom(Val::Px(40.0)), ..default() }));

            parent.spawn((Button, Node { width: Val::Px(300.0), height: Val::Px(60.0), margin: UiRect::all(Val::Px(10.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(Color::srgb(0.3, 0.5, 0.8)), MenuButton::Resume))
                .with_children(|p| { p.spawn((Text::new("Resume"), TextFont { font_size: FontSize::Px(32.0), ..default() }, TextColor(Color::WHITE))); });

            parent.spawn((Button, Node { width: Val::Px(300.0), height: Val::Px(60.0), margin: UiRect::all(Val::Px(10.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(Color::srgb(0.3, 0.5, 0.8)), MenuButton::FpsToggle))
                .with_children(|p| { p.spawn((Text::new("FPS: OFF"), TextFont { font_size: FontSize::Px(32.0), ..default() }, TextColor(Color::WHITE), FpsToggleText)); });

            parent.spawn((
                Node { width: Val::Px(340.0), height: Val::Px(50.0), margin: UiRect::all(Val::Px(10.0)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() },
            )).with_children(|p| {
                p.spawn((Text::new("Vol"), TextFont { font_size: FontSize::Px(28.0), ..default() }, TextColor(Color::WHITE), Node { width: Val::Px(50.0), ..default() }));
                
                p.spawn((
                    Node { width: Val::Px(220.0), height: Val::Px(30.0), align_items: AlignItems::Center, justify_content: JustifyContent::FlexStart, ..default() },
                    Interaction::None,
                    RelativeCursorPosition::default(),
                    VolumeSlider,
                )).with_children(|p2| {
                    p2.spawn((Node { width: Val::Px(200.0), height: Val::Px(8.0), ..default() }, BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.9))))
                        .with_children(|p3| {
                            p3.spawn((Node { width: Val::Percent(60.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.3, 0.6, 0.9)), VolumeSliderFill));
                        });
                    p2.spawn((Node { width: Val::Px(18.0), height: Val::Px(18.0), position_type: PositionType::Absolute, left: Val::Px(20.0 + 200.0 * 0.6 - 9.0), ..default() }, BackgroundColor(Color::WHITE), VolumeSliderHandle));
                });

                p.spawn((Text::new("60%"), TextFont { font_size: FontSize::Px(28.0), ..default() }, TextColor(Color::WHITE), Node { width: Val::Px(60.0), justify_content: JustifyContent::FlexEnd, ..default() }, VolumeLabel));
            });

            parent.spawn((Button, Node { width: Val::Px(300.0), height: Val::Px(60.0), margin: UiRect::all(Val::Px(10.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(Color::srgb(0.8, 0.3, 0.3)), MenuButton::Quit))
                .with_children(|p| { p.spawn((Text::new("Quit"), TextFont { font_size: FontSize::Px(32.0), ..default() }, TextColor(Color::WHITE))); });
        });
}

fn setup_audio(world: &mut World) {
    let sample_rate: u32 = 44100;
    let secs = 12.0f32;
    let n = (sample_rate as f32 * secs) as usize;
    let mut samples: Vec<f32> = Vec::with_capacity(n);
    let mut rng: u32 = 987_654_321;
    let mut noise = || {
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((rng >> 8) as f32) / 16_777_216.0 * 2.0 - 1.0
    };
    let mut prev = 0.0f32;
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let raw = noise();
        prev += 0.02 * (raw - prev);
        let swell = ((t * 0.14 * 2.0 * PI).sin() * 0.5 + 0.5) * 0.7 + ((t * 0.06 * 2.0 * PI).sin() * 0.5 + 0.5) * 0.3;
        let amp = 0.35 * (0.25 + 0.75 * swell);
        samples.push((prev * amp).clamp(-1.0, 1.0));
    }
    match OutputStream::try_default() {
        Ok((stream, handle)) => match Sink::try_new(&handle) {
            Ok(sink) => {
                let buffer = SamplesBuffer::new(1, sample_rate, samples);
                let volume = world.resource::<Settings>().volume;
                sink.set_volume(volume);
                sink.append(buffer.repeat_infinite());
                sink.play();
                println!("Звук океана включён");
                world.insert_non_send(AmbientAudio { _stream: stream, sink });
            }
            Err(_) => println!("Не удалось создать аудио-синк"),
        },
        Err(_) => println!("Аудио-устройство не найдено, звук выключен"),
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    println!("===== OCEAN RELAX =====");
    println!("Подойди к ГАМАКУ и жми E  -> лечь");
    println!("Лежишь: жми E             -> встать");
    println!("Подойди к БАРУ и жми E    -> сменить лимонад/кофе");
    println!("SPACE                     -> достать / убрать напиток");
    println!("Escape                    -> пауза / меню");
    println!("WASD + мышь, Shift — бег");
    println!("=======================");

    let mut bloom = Bloom::default();
    bloom.intensity = 0.4;
    bloom.prefilter.threshold = 0.65;

    commands.spawn((
        Camera3d::default(), Hdr, Tonemapping::AcesFitted, bloom,
        DistanceFog { color: Color::srgb(0.60, 0.66, 0.82), falloff: FogFalloff::Linear { start: 55.0, end: 420.0 }, ..default() },
        Transform::from_translation(REST_CAM),
        Player { yaw: 0.0, pitch: 0.0, bob: 0.0 },
    ));

    let to_sun = SUN_POS.normalize();
    commands.spawn((
        DirectionalLight { illuminance: 30_000.0, shadow_maps_enabled: true, color: Color::srgb(1.0, 0.86, 0.66), ..default() },
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, -to_sun)),
        Visibility::default(),
    ));

    let mat_ocean = materials.add(StandardMaterial { base_color: Color::srgb(0.015, 0.16, 0.33), metallic: 0.45, perceptual_roughness: 0.05, emissive: LinearRgba::from(Color::srgb(0.02, 0.06, 0.12)) * 1.8, ..default() });
    let mat_shallow = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.48, 0.55), alpha_mode: AlphaMode::Blend, metallic: 0.3, perceptual_roughness: 0.08, emissive: LinearRgba::from(Color::srgb(0.03, 0.12, 0.13)) * 1.4, ..default() });
    let mat_seabed = materials.add(StandardMaterial { base_color: Color::srgb(0.28, 0.32, 0.27), perceptual_roughness: 0.95, ..default() });
    let mat_foam = materials.add(StandardMaterial { base_color: Color::srgb(0.97, 0.99, 1.0), alpha_mode: AlphaMode::Blend, emissive: LinearRgba::from(Color::srgb(0.5, 0.55, 0.6)) * 1.2, ..default() });
    let mat_sand = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.85, 0.63), perceptual_roughness: 0.98, ..default() });
    let mat_sand_dark = materials.add(StandardMaterial { base_color: Color::srgb(0.86, 0.74, 0.52), perceptual_roughness: 0.9, ..default() });
    let mat_wet = materials.add(StandardMaterial { base_color: Color::srgb(0.56, 0.49, 0.36), metallic: 0.3, perceptual_roughness: 0.10, ..default() });
    let mat_bark = materials.add(StandardMaterial { base_color: Color::srgb(0.38, 0.26, 0.16), perceptual_roughness: 0.92, ..default() });
    let mat_frond = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.48, 0.21), perceptual_roughness: 0.72, ..default() });
    let mat_coco = materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.2, 0.11), perceptual_roughness: 0.85, ..default() });
    let mat_cloth = materials.add(StandardMaterial { base_color: Color::srgb(0.88, 0.32, 0.24), perceptual_roughness: 0.72, ..default() });
    let mat_cloth2 = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.88, 0.72), perceptual_roughness: 0.75, ..default() });
    let mat_rope = materials.add(StandardMaterial { base_color: Color::srgb(0.72, 0.62, 0.44), perceptual_roughness: 0.9, ..default() });
    let mat_skin = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.77, 0.62), perceptual_roughness: 0.65, ..default() });
    let mat_shirt = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.46, 0.74), perceptual_roughness: 0.78, ..default() });
    let mat_shorts = materials.add(StandardMaterial { base_color: Color::srgb(0.9, 0.62, 0.2), perceptual_roughness: 0.8, ..default() });
    let mat_hat = materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.82, 0.55), perceptual_roughness: 0.85, ..default() });
    let mat_rock = materials.add(StandardMaterial { base_color: Color::srgb(0.52, 0.5, 0.48), perceptual_roughness: 0.88, ..default() });
    let mat_bush = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.42, 0.2), perceptual_roughness: 0.85, ..default() });
    let mat_island = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.33, 0.23), perceptual_roughness: 0.95, ..default() });
    let mat_forest = materials.add(StandardMaterial { base_color: Color::srgb(0.13, 0.32, 0.18), perceptual_roughness: 0.9, ..default() });
    let mat_cloud = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.97, 0.92), emissive: LinearRgba::from(Color::srgb(0.6, 0.55, 0.5)) * 1.4, perceptual_roughness: 1.0, ..default() });
    let mat_bird = materials.add(StandardMaterial { base_color: Color::srgb(0.1, 0.1, 0.11), perceptual_roughness: 1.0, ..default() });
    let mat_sun = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.92, 0.72), emissive: LinearRgba::from(Color::srgb(1.0, 0.82, 0.5)) * 30.0, ..default() });
    let mat_halo = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.88, 0.65), alpha_mode: AlphaMode::Blend, emissive: LinearRgba::from(Color::srgb(1.0, 0.8, 0.45)) * 3.0, ..default() });
    let mat_sparkle = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.97, 0.85), emissive: LinearRgba::from(Color::srgb(1.0, 0.93, 0.7)) * 9.0, unlit: true, ..default() });
    let mat_sunpath = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.85, 0.55), alpha_mode: AlphaMode::Blend, emissive: LinearRgba::from(Color::srgb(1.0, 0.75, 0.4)) * 2.2, ..default() });
    let mat_marker = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.95, 0.4), emissive: LinearRgba::from(Color::srgb(1.0, 0.9, 0.3)) * 5.0, unlit: true, ..default() });
    let mat_lemonade = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.88, 0.22), emissive: LinearRgba::from(Color::srgb(1.0, 0.85, 0.15)) * 3.0, perceptual_roughness: 0.2, ..default() });
    let mat_lemon = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.92, 0.2), emissive: LinearRgba::from(Color::srgb(1.0, 0.88, 0.1)) * 3.5, perceptual_roughness: 0.3, ..default() });
    let mat_straw = materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.3, 0.3), emissive: LinearRgba::from(Color::srgb(0.4, 0.08, 0.08)) * 1.2, perceptual_roughness: 0.4, ..default() });
    let mat_coffee = materials.add(StandardMaterial { base_color: Color::srgb(0.28, 0.16, 0.08), emissive: LinearRgba::from(Color::srgb(0.25, 0.12, 0.05)) * 1.6, perceptual_roughness: 0.4, ..default() });
    let mat_mug = materials.add(StandardMaterial { base_color: Color::srgb(0.92, 0.9, 0.86), perceptual_roughness: 0.35, ..default() });
    let mat_steam = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 1.0, 1.0), alpha_mode: AlphaMode::Blend, emissive: LinearRgba::from(Color::srgb(0.8, 0.85, 0.9)) * 0.8, ..default() });
    let mat_wood = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.44, 0.26), perceptual_roughness: 0.82, ..default() });
    let mat_wood_dark = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.30, 0.17), perceptual_roughness: 0.85, ..default() });
    let mat_canopy = materials.add(StandardMaterial { base_color: Color::srgb(0.98, 0.66, 0.16), perceptual_roughness: 0.7, ..default() });
    let mat_board = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.16, 0.12), emissive: LinearRgba::from(Color::srgb(0.12, 0.09, 0.06)) * 1.2, perceptual_roughness: 0.9, ..default() });

    let m_sphere = meshes.add(Sphere::new(1.0));
    let m_cuboid = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let m_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    spawn_scaled(&mut commands, m_cuboid.clone(), mat_seabed.clone(), Vec3::new(0.0, -1.4, -200.0), Vec3::new(800.0, 0.4, 420.0));
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_ocean.clone()), Transform { translation: Vec3::new(0.0, -0.12, -200.0), scale: Vec3::new(800.0, 0.25, 420.0), ..default() }, Ocean));
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_shallow.clone()), Transform { translation: Vec3::new(0.0, -0.03, -8.0), scale: Vec3::new(300.0, 0.12, 30.0), ..default() }));

    let d = Vec3::new(SUN_POS.x, 0.0, SUN_POS.z).normalize();
    let yaw = d.x.atan2(d.z);
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_sunpath.clone()), Transform { translation: Vec3::new(d.x * 70.0, 0.015, d.z * 70.0), rotation: Quat::from_rotation_y(yaw), scale: Vec3::new(7.0, 0.02, 150.0), ..default() }, SunPath));

    for (i, bz) in [0.6, 1.6, 2.6, 3.8].iter().enumerate() {
        commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_foam.clone()), Transform { translation: Vec3::new(0.0, 0.02, *bz), scale: Vec3::new(60.0, 0.03, 0.5), ..default() }, Foam { base_z: *bz, phase: i as f32 * 1.7, speed: 0.5 + i as f32 * 0.08 }));
    }

    spawn_scaled(&mut commands, m_cuboid.clone(), mat_sand.clone(), Vec3::new(0.0, 0.05, 26.0), Vec3::new(240.0, SAND_TOP, 56.0));
    for (dx, dz, r) in [(-40.0, 36.0, 7.0), (-18.0, 40.0, 5.5), (20.0, 38.0, 7.5), (42.0, 34.0, 6.0), (55.0, 40.0, 5.0), (-55.0, 38.0, 6.5)] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_sand_dark.clone(), Vec3::new(dx, SAND_TOP - r * 0.4, dz), Vec3::new(r * 2.0, r * 0.5, r * 1.4));
    }
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_wet.clone(), Vec3::new(0.0, SAND_TOP + 0.006, 1.6), Vec3::new(240.0, 0.02, 3.2));

    for (x, z, r) in [(-140.0, -360.0, 46.0), (150.0, -380.0, 60.0), (20.0, -400.0, 40.0)] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_island.clone(), Vec3::new(x, -r * 0.55, z), Vec3::new(r, r * 0.5, r * 0.7));
    }

    let mut rng: u32 = 1234;
    for _ in 0..40 {
        let r1 = next_rand(&mut rng); let r2 = next_rand(&mut rng);
        let x = -80.0 + r1 * 160.0; let z = 34.0 + r2 * 8.0; let h = 2.4 + r2 * 2.6;
        spawn_scaled(&mut commands, m_cyl.clone(), mat_bark.clone(), Vec3::new(x, SAND_TOP + h * 0.5, z), Vec3::new(0.28, h, 0.28));
        spawn_scaled(&mut commands, m_sphere.clone(), mat_forest.clone(), Vec3::new(x, SAND_TOP + h + 0.6, z), Vec3::new(2.0 + r1, 1.7 + r2, 2.0 + r2));
    }

    let mut obstacles: Vec<(f32, f32, f32)> = vec![];
    for (x, z, lean, height) in [(-3.2, 10.0, 0.055, 5.4), (3.2, 10.0, -0.055, 5.4), (-12.0, 14.0, 0.10, 6.2), (14.0, 12.0, -0.11, 6.6), (-22.0, 18.0, 0.07, 5.8), (24.0, 20.0, -0.08, 6.0), (-34.0, 15.0, 0.12, 5.2), (-8.0, 26.0, 0.06, 5.6), (18.0, 27.0, -0.09, 6.0), (30.0, 16.0, 0.05, 5.4)] {
        spawn_palm(&mut commands, &m_cuboid, &m_sphere, &mat_bark, &mat_frond, &mat_coco, Vec3::new(x, SAND_TOP, z), lean, height);
        obstacles.push((x, z, 0.55));
    }

    spawn_hammock(&mut commands, &m_cuboid, &m_cyl, &mat_cloth, &mat_cloth2, &mat_rope, HAMMOCK_POS);
    spawn_character(&mut commands, &m_sphere, &m_cuboid, &m_cyl, &mat_skin, &mat_shirt, &mat_shorts, &mat_hat);

    spawn_bar(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood, &mat_wood_dark, &mat_canopy, &mat_board, &mat_lemonade, &mat_coffee);
    obstacles.push((BAR_POS.x, BAR_POS.z, 1.9));

    let umb = Vec3::new(-7.0, SAND_TOP, 13.0);
    spawn_scaled(&mut commands, m_cyl.clone(), mat_wood.clone(), umb + Vec3::new(0.12, 1.05, 0.0), Vec3::new(0.035, 2.1, 0.035));
    spawn_scaled(&mut commands, m_sphere.clone(), mat_canopy.clone(), umb + Vec3::new(0.24, 1.98, 0.0), Vec3::new(2.6, 0.8, 2.6));
    obstacles.push((umb.x, umb.z, 0.4));
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_cloth.clone(), Vec3::new(-7.0, SAND_TOP + 0.03, 15.0), Vec3::new(1.6, 0.03, 2.6));

    for (x, z, r, sy) in [(-14.0, 8.0, 0.9, 0.5), (-12.5, 7.2, 0.5, 0.35), (16.0, 9.0, 1.0, 0.55), (18.0, 8.2, 0.55, 0.32), (-28.0, 12.0, 1.2, 0.6), (28.0, 14.0, 0.9, 0.45), (6.0, 24.0, 0.8, 0.4), (-20.0, 26.0, 1.0, 0.5)] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_rock.clone(), Vec3::new(x, SAND_TOP - r * sy * 0.6, z), Vec3::new(r, r * sy, r * 0.85));
        obstacles.push((x, z, r * 0.9));
    }
    for (x, z, r) in [(-18.0, 22.0, 1.3), (-15.0, 24.0, 0.9), (22.0, 23.0, 1.2), (26.0, 20.0, 0.8), (-32.0, 24.0, 1.1), (10.0, 28.0, 1.0)] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_bush.clone(), Vec3::new(x, SAND_TOP + r * 0.25, z), Vec3::new(r * 1.4, r * 0.8, r * 1.3));
        obstacles.push((x, z, r * 0.8));
    }
    for _ in 0..26 {
        let r1 = next_rand(&mut rng); let r2 = next_rand(&mut rng);
        let x = -40.0 + r1 * 80.0; let z = 4.0 + r2 * 30.0;
        spawn_scaled(&mut commands, m_sphere.clone(), mat_sand_dark.clone(), Vec3::new(x, SAND_TOP + 0.02, z), Vec3::new(0.12 + r2 * 0.08, 0.05, 0.1 + r1 * 0.06));
    }

    spawn_scaled(&mut commands, m_sphere.clone(), mat_sun.clone(), SUN_POS, Vec3::new(16.0, 16.0, 16.0));
    spawn_scaled(&mut commands, m_sphere.clone(), mat_halo.clone(), SUN_POS * 0.995, Vec3::new(42.0, 42.0, 42.0));

    for (_i, (x, y, z, s)) in [(-80.0, 34.0, -90.0, 1.2), (-20.0, 40.0, -140.0, 1.6), (60.0, 32.0, -80.0, 1.0), (130.0, 38.0, -150.0, 1.7), (-150.0, 36.0, -60.0, 1.3), (20.0, 44.0, 60.0, 1.5), (-90.0, 42.0, 80.0, 1.2), (110.0, 34.0, 100.0, 1.4)].into_iter().enumerate() {
        for (dx, dy, dz, r) in [(0.0, 0.0, 0.0, 7.0), (6.5, -0.8, 1.5, 5.0), (-6.0, -1.0, -1.0, 5.5), (3.0, 1.6, -2.5, 4.2), (-3.5, 1.2, 2.5, 4.6), (10.5, -1.4, 0.5, 3.6)] {
            commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_cloud.clone()), Transform { translation: Vec3::new(x + dx * s, y + dy * s, z + dz * s), scale: Vec3::new(r * s, r * s * 0.45, r * s * 0.8), ..default() }, CloudPuff { speed: 0.8 + next_rand(&mut rng) * 1.4 }));
        }
    }

    for (i, (cx, cz, rad, h)) in [(0.0, -30.0, 30.0, 16.0), (20.0, -15.0, 22.0, 19.0), (-25.0, -35.0, 34.0, 14.0), (8.0, -5.0, 26.0, 22.0), (-12.0, -12.0, 18.0, 18.0)].into_iter().enumerate() {
        for k in 0..3 {
            commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_bird.clone()), Transform { translation: Vec3::new(cx + k as f32 * 0.6, h, cz), scale: Vec3::new(0.55, 0.2, 0.22), ..default() }, Bird { center: Vec3::new(cx, 0.0, cz), radius: rad, speed: 0.08 + i as f32 * 0.012, phase: k as f32 * 0.5 + i as f32 * 1.3, height: h }));
        }
    }

    spawn_yacht(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood_dark, &mat_cloth2, Vec3::new(-30.0, 0.0, -60.0), 0.0);
    spawn_yacht(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood_dark, &mat_cloth, Vec3::new(40.0, 0.0, -90.0), 1.5);

    for _ in 0..60 {
        let r1 = next_rand(&mut rng); let r2 = next_rand(&mut rng);
        let along = 6.0 + r1 * 120.0; let side = (r2 - 0.5) * 14.0;
        let x = d.x * along + -d.z * side; let z = d.z * along + d.x * side;
        commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_sparkle.clone()), Transform { translation: Vec3::new(x, 0.02, z), scale: Vec3::splat(0.12), ..default() }, Sparkle { phase: r1 * 12.0 }));
    }

    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_marker.clone()), Transform { translation: HAMMOCK_POS + Vec3::new(0.0, 2.8, 0.0), scale: Vec3::splat(0.001), ..default() }, InteractMarker { kind: InteractKind::Hammock }));
    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_marker.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 3.8, 0.0), scale: Vec3::splat(0.001), ..default() }, InteractMarker { kind: InteractKind::Bar }));

    let hidden = Vec3::new(0.0, -5.0, 0.0);
    commands.spawn((Transform::from_translation(hidden), Visibility::default(), DrinkGroup { kind: DrinkKind::Lemonade }, DrinkScale { current: 0.0 })).with_children(|parent| {
        parent.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_lemonade.clone()), Transform { translation: Vec3::ZERO, scale: Vec3::new(0.24, 0.8, 0.24), ..default() }));
        parent.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_lemon.clone()), Transform { translation: Vec3::new(0.0, 0.42, 0.0), scale: Vec3::new(0.3, 0.1, 0.3), rotation: Quat::from_rotation_z(0.3), ..default() }));
        parent.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_lemon.clone()), Transform { translation: Vec3::new(0.22, 0.12, 0.0), scale: Vec3::new(0.16, 0.05, 0.16), ..default() }));
        parent.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_straw.clone()), Transform { translation: Vec3::new(0.07, 0.55, 0.0), scale: Vec3::new(0.03, 0.7, 0.03), rotation: Quat::from_rotation_z(-0.18), ..default() }));
    });

    commands.spawn((Transform::from_translation(hidden), Visibility::default(), DrinkGroup { kind: DrinkKind::Coffee }, DrinkScale { current: 0.0 })).with_children(|parent| {
        parent.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_mug.clone()), Transform { translation: Vec3::ZERO, scale: Vec3::new(0.2, 0.36, 0.2), ..default() }));
        parent.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_coffee.clone()), Transform { translation: Vec3::new(0.0, 0.17, 0.0), scale: Vec3::new(0.18, 0.03, 0.18), ..default() }));
        for i in 0..5 {
            let a = -0.9 + i as f32 * 0.45;
            parent.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_mug.clone()), Transform { translation: Vec3::new(0.22 + a.cos() * 0.08, a.sin() * 0.11, 0.0), scale: Vec3::splat(0.045), ..default() }));
        }
        for i in 0..3 {
            parent.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_steam.clone()), Transform { translation: Vec3::new((i as f32 - 1.0) * 0.06, 0.3, 0.0), scale: Vec3::splat(0.05), ..default() }, Steam { base: Vec3::new((i as f32 - 1.0) * 0.06, 0.3, 0.0), phase: i as f32 * 0.4 }));
        }
    });

    commands.insert_resource(Obstacles(obstacles));
}

fn spawn_scaled(commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>, translation: Vec3, scale: Vec3) {
    commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform { translation, scale, ..default() }));
}

fn spawn_beam(commands: &mut Commands, mesh: &Handle<Mesh>, material: &Handle<StandardMaterial>, start: Vec3, end: Vec3, thickness: f32) {
    let dir = end - start; let len = dir.length();
    if len <= f32::EPSILON { return; }
    commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform { translation: start + dir * 0.5, rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()), scale: Vec3::new(thickness, len, thickness) }));
}

fn spawn_palm(commands: &mut Commands, m_cuboid: &Handle<Mesh>, m_sphere: &Handle<Mesh>, mat_bark: &Handle<StandardMaterial>, mat_frond: &Handle<StandardMaterial>, mat_coco: &Handle<StandardMaterial>, pos: Vec3, lean: f32, height: f32) {
    let segments = 8; let seg_h = height / segments as f32;
    let mut cur = pos; let mut ang = 0.0;
    for i in 0..segments {
        ang += lean; let dir = Vec3::new(ang.sin(), ang.cos(), 0.0); let w = 0.42 - i as f32 * 0.028;
        commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_bark.clone()), Transform { translation: cur + dir * seg_h * 0.5, rotation: Quat::from_rotation_z(-ang), scale: Vec3::new(w, seg_h * 1.06, w) }));
        cur += dir * seg_h;
    }
    for (dx, dz) in [(0.25, 0.15), (-0.22, 0.2), (0.02, -0.28)] {
        commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_coco.clone()), Transform { translation: Vec3::new(cur.x + dx, cur.y - 0.28, cur.z + dz), scale: Vec3::splat(0.19), ..default() }));
    }
    for i in 0..9 {
        let a = (i as f32 / 9.0) * PI * 2.0 + lean * 2.0; let droop = 0.42 + (i % 3) as f32 * 0.12; let len = 2.6 + (i % 4) as f32 * 0.35;
        let base_rot = Quat::from_rotation_y(a) * Quat::from_rotation_x(droop); let offset = base_rot * Vec3::new(0.0, 0.12, -len * 0.5);
        commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_frond.clone()), Transform { translation: cur + offset, rotation: base_rot, scale: Vec3::new(0.5, 0.07, len) }, Sway { base: cur + offset, base_rot, phase: i as f32 * 0.9 + pos.x }));
    }
}

fn spawn_hammock(commands: &mut Commands, m_cuboid: &Handle<Mesh>, m_cyl: &Handle<Mesh>, mat_cloth: &Handle<StandardMaterial>, mat_cloth2: &Handle<StandardMaterial>, mat_rope: &Handle<StandardMaterial>, center: Vec3) {
    let left = center + Vec3::new(-3.2, 0.0, 0.0); let right = center + Vec3::new(3.2, 0.0, 0.0); let z = center.z;
    let anchor_l = left + Vec3::new(0.15, 2.55, 0.0); let anchor_r = right + Vec3::new(-0.15, 2.55, 0.0);
    for (x, y, len, mat) in [(-2.05, 0.72, 1.5, mat_cloth2.clone()), (0.0, 0.62, 3.2, mat_cloth.clone()), (2.05, 0.72, 1.5, mat_cloth2.clone())] {
        commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat), Transform { translation: Vec3::new(center.x + x, y, z), scale: Vec3::new(len, 0.07, 0.95), ..default() }, Sway { base: Vec3::new(center.x + x, y, z), base_rot: Quat::IDENTITY, phase: 0.0 }));
    }
    for dz in [-0.42, -0.14, 0.14, 0.42] {
        spawn_beam(commands, m_cuboid, mat_rope, anchor_l, Vec3::new(center.x - 2.72, 0.72, z + dz * 0.8), 0.028);
        spawn_beam(commands, m_cuboid, mat_rope, anchor_r, Vec3::new(center.x + 2.72, 0.72, z + dz * 0.8), 0.028);
    }
    for x in [-2.68, 2.68] {
        for dz in [-0.3, 0.0, 0.3] {
            commands.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_rope.clone()), Transform { translation: Vec3::new(center.x + x, 0.62, z + dz), scale: Vec3::new(0.02, 0.16, 0.02), ..default() }));
        }
    }
}

fn spawn_character(commands: &mut Commands, m_sphere: &Handle<Mesh>, m_cuboid: &Handle<Mesh>, m_cyl: &Handle<Mesh>, mat_skin: &Handle<StandardMaterial>, mat_shirt: &Handle<StandardMaterial>, mat_shorts: &Handle<StandardMaterial>, mat_hat: &Handle<StandardMaterial>) {
    let cx = HAMMOCK_POS.x; let z = HAMMOCK_POS.z;
    let parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Vec3, Vec3, Quat)> = vec![
        (m_sphere.clone(), mat_skin.clone(), Vec3::new(cx - 2.02, 0.9, z), Vec3::new(0.38, 0.42, 0.38), Quat::IDENTITY),
        (m_sphere.clone(), mat_hat.clone(), Vec3::new(cx - 2.02, 1.06, z), Vec3::new(0.92, 0.1, 0.92), Quat::IDENTITY),
        (m_cyl.clone(), mat_hat.clone(), Vec3::new(cx - 2.02, 1.16, z), Vec3::new(0.42, 0.22, 0.42), Quat::IDENTITY),
        (m_cuboid.clone(), mat_shirt.clone(), Vec3::new(cx - 1.15, 0.79, z), Vec3::new(1.15, 0.3, 0.46), Quat::from_rotation_z(0.05)),
        (m_cuboid.clone(), mat_shorts.clone(), Vec3::new(cx - 0.35, 0.75, z), Vec3::new(0.55, 0.26, 0.44), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx + 0.55, 0.72, z - 0.12), Vec3::new(1.2, 0.17, 0.17), Quat::from_rotation_z(-0.04)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx + 0.55, 0.72, z + 0.12), Vec3::new(1.2, 0.17, 0.17), Quat::from_rotation_z(-0.04)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx + 1.2, 0.7, z - 0.12), Vec3::new(0.24, 0.13, 0.2), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx + 1.2, 0.7, z + 0.12), Vec3::new(0.24, 0.13, 0.2), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx - 1.35, 0.86, z - 0.32), Vec3::new(0.75, 0.14, 0.14), Quat::from_rotation_z(0.5)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(cx - 0.82, 0.86, z + 0.3), Vec3::new(0.8, 0.14, 0.14), Quat::from_rotation_z(-0.25)),
    ];
    for (mesh, mat, pos, scale, rot) in parts {
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform { translation: pos, rotation: rot, scale, ..default() }, Sway { base: pos, base_rot: rot, phase: 0.0 }, CharacterPart));
    }
}

fn spawn_bar(commands: &mut Commands, m_cuboid: &Handle<Mesh>, m_cyl: &Handle<Mesh>, m_sphere: &Handle<Mesh>, mat_wood: &Handle<StandardMaterial>, mat_wood_dark: &Handle<StandardMaterial>, mat_canopy: &Handle<StandardMaterial>, mat_board: &Handle<StandardMaterial>, mat_lemonade: &Handle<StandardMaterial>, mat_coffee: &Handle<StandardMaterial>) {
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_wood.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 0.55, 0.0), scale: Vec3::new(3.4, 1.1, 1.0), ..default() }));
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_wood_dark.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 1.12, 0.0), scale: Vec3::new(3.7, 0.08, 1.2), ..default() }));
    commands.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_wood.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 1.6, -0.6), scale: Vec3::new(0.06, 3.2, 0.06), ..default() }));
    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_canopy.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 3.1, -0.6), scale: Vec3::new(4.6, 1.2, 4.6), ..default() }));
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_board.clone()), Transform { translation: BAR_POS + Vec3::new(0.0, 1.8, 0.6), scale: Vec3::new(2.2, 1.0, 0.06), ..default() }));
    for dx in [-1.2, 1.2] {
        commands.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_wood_dark.clone()), Transform { translation: BAR_POS + Vec3::new(dx, 0.35, 1.4), scale: Vec3::new(0.35, 0.7, 0.35), ..default() }));
    }
    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_lemonade.clone()), Transform { translation: BAR_POS + Vec3::new(-0.7, 1.45, 0.0), scale: Vec3::splat(0.22), ..default() }, BarIndicator { kind: DrinkKind::Lemonade }));
    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_coffee.clone()), Transform { translation: BAR_POS + Vec3::new(0.7, 1.45, 0.0), scale: Vec3::splat(0.001), ..default() }, BarIndicator { kind: DrinkKind::Coffee }));
}

fn spawn_yacht(commands: &mut Commands, m_cuboid: &Handle<Mesh>, m_cyl: &Handle<Mesh>, m_sphere: &Handle<Mesh>, mat_hull: &Handle<StandardMaterial>, mat_sail: &Handle<StandardMaterial>, pos: Vec3, phase: f32) {
    commands.spawn((Mesh3d(m_sphere.clone()), MeshMaterial3d(mat_hull.clone()), Transform { translation: pos, scale: Vec3::new(3.4, 0.9, 1.4), ..default() }, Yacht { base: pos, phase }));
    commands.spawn((Mesh3d(m_cyl.clone()), MeshMaterial3d(mat_hull.clone()), Transform { translation: pos + Vec3::new(0.0, 2.6, 0.0), scale: Vec3::new(0.08, 5.2, 0.08), ..default() }, Yacht { base: pos + Vec3::new(0.0, 2.6, 0.0), phase }));
    commands.spawn((Mesh3d(m_cuboid.clone()), MeshMaterial3d(mat_sail.clone()), Transform { translation: pos + Vec3::new(0.5, 3.2, 0.0), scale: Vec3::new(0.06, 4.0, 2.2), ..default() }, Yacht { base: pos + Vec3::new(0.5, 3.2, 0.0), phase }));
}

fn ground_height(z: f32) -> f32 {
    if z >= 1.0 { SAND_TOP } else if z <= -1.0 { WATER_Y } else { WATER_Y + ((z + 1.0) / 2.0) * (SAND_TOP - WATER_Y) }
}

fn camera_control(time: Res<Time>, keys: Res<ButtonInput<KeyCode>>, mut mouse: MessageReader<MouseMotion>, mut mode: ResMut<RelaxMode>, mut state: ResMut<DrinkState>, mut cursor: Single<&mut CursorOptions>, cam: Single<(&mut Transform, &mut Player)>, obstacles: Res<Obstacles>) {
    let dt = time.delta_secs().min(0.05); let t = time.elapsed_secs();
    let (mut tf, mut player) = cam.into_inner();

    if keys.just_pressed(KeyCode::KeyE) {
        match *mode {
            RelaxMode::Resting => {
                *mode = RelaxMode::Walking; cursor.grab_mode = CursorGrabMode::Locked; cursor.visible = false;
                let exit_z = HAMMOCK_POS.z + 2.2;
                tf.translation = Vec3::new(HAMMOCK_POS.x, ground_height(exit_z) + EYE, exit_z);
                player.bob = 0.0; println!("Встал с гамака");
            }
            RelaxMode::Walking => {
                let p = tf.translation;
                let d_bar = Vec2::new(p.x - BAR_POS.x, p.z - BAR_POS.z).length();
                let d_ham = Vec2::new(p.x - HAMMOCK_POS.x, p.z - HAMMOCK_POS.z).length();
                if d_bar <= BAR_RADIUS {
                    state.selected = if state.selected == DrinkKind::Lemonade { DrinkKind::Coffee } else { DrinkKind::Lemonade };
                    println!("Бар: выбран {:?}", state.selected);
                } else if d_ham <= HAMMOCK_RADIUS {
                    *mode = RelaxMode::Resting; cursor.grab_mode = CursorGrabMode::None; cursor.visible = true;
                    player.yaw = 0.0; player.pitch = 0.0; player.bob = 0.0;
                    println!("Лёг в гамак. Крути камерой сколько хочешь.");
                } else {
                    println!("Подойди ближе к гамаку или бару, чтобы нажать E.");
                }
            }
        }
    }

    for ev in mouse.read() {
        match *mode {
            RelaxMode::Walking => { player.yaw -= ev.delta.x * 0.0022; player.pitch = (player.pitch - ev.delta.y * 0.0022).clamp(-1.35, 1.35); }
            RelaxMode::Resting => { player.yaw -= ev.delta.x * 0.0022; player.pitch = (player.pitch - ev.delta.y * 0.0022).clamp(-1.4, 1.4); }
        }
    }

    match *mode {
        RelaxMode::Resting => {
            let a = (t * 1.15).sin(); let b = (t * 0.78).cos();
            tf.translation = Vec3::new(REST_CAM.x, REST_CAM.y + a * 0.02, REST_CAM.z + b * 0.012);
            tf.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, b * 0.014);
        }
        RelaxMode::Walking => {
            let mut input = Vec2::ZERO;
            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { input.y += 1.0; }
            if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { input.y -= 1.0; }
            if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { input.x += 1.0; }
            if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { input.x -= 1.0; }
            let sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            let speed = if sprint { 6.2 } else { 3.4 };

            if input.length_squared() > 0.001 {
                let fwd = Vec3::new(-player.yaw.sin(), 0.0, -player.yaw.cos());
                let right = Vec3::new(player.yaw.cos(), 0.0, -player.yaw.sin());
                let dir = (fwd * input.y + right * input.x).normalize();
                let step = dir * speed * dt;
                let mut next = tf.translation + step;
                next.x = next.x.clamp(-WORLD_X, WORLD_X);
                next.z = next.z.clamp(BEACH_MIN_Z, BEACH_MAX_Z);
                for &(ox, oz, r) in obstacles.0.iter() {
                    let d = Vec2::new(next.x - ox, next.z - oz); let len = d.length();
                    if len < r && len > 1e-4 { let push = d / len * r; next.x = ox + push.x; next.z = oz + push.y; }
                }
                player.bob += step.length() * 8.5;
                next.y = ground_height(next.z) + EYE + player.bob.sin() * 0.045;
                tf.translation = next;
            } else {
                player.bob += dt * 1.1;
                let target = ground_height(tf.translation.z) + EYE + player.bob.sin() * 0.006;
                tf.translation.y = tf.translation.y.lerp(target, (dt * 6.0).min(1.0));
            }
            let lean = (player.bob * 0.5).sin() * 0.011;
            tf.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, lean);
        }
    }
}

fn drink_input(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<DrinkState>) {
    if keys.just_pressed(KeyCode::Space) {
        state.holding = !state.holding;
        if state.holding { println!("Достал {:?}", state.selected); } else { println!("Убрал напиток в карман"); }
    }
}

fn drink_animation(time: Res<Time>, state: Res<DrinkState>, cam: Single<&Transform, (With<Player>, Without<DrinkGroup>)>, mut q: Query<(&mut Transform, &mut DrinkScale, &DrinkGroup), Without<Player>>) {
    let dt = time.delta_secs().min(0.05); let alpha = (dt * 6.0).min(1.0); let t = time.elapsed_secs();
    let cam_tf = cam.into_inner();
    let hand_local = Vec3::new(0.55, -0.5, -1.15);
    let hand_world = cam_tf.translation + cam_tf.rotation * hand_local;

    for (mut tf, mut ds, dg) in q.iter_mut() {
        let target = if state.holding && dg.kind == state.selected { 1.0 } else { 0.0 };
        ds.current = ds.current.lerp(target, alpha);
        if ds.current > 0.01 {
            let sway = Vec3::new((t * 1.4).sin() * 0.02, (t * 2.0).cos() * 0.015, 0.0);
            tf.translation = hand_world + sway;
            tf.rotation = cam_tf.rotation * Quat::from_rotation_z((t * 1.0).sin() * 0.06);
        } else {
            tf.translation = Vec3::new(0.0, -5.0, 0.0);
        }
        tf.scale = Vec3::splat(ds.current.max(0.0001));
    }
}

fn bar_indicator_system(state: Res<DrinkState>, mut q: Query<(&mut Transform, &BarIndicator)>) {
    for (mut tf, ind) in q.iter_mut() {
        let on = ind.kind == state.selected;
        let target = if on { 0.22 } else { 0.001 };
        tf.scale.x = tf.scale.x.lerp(target, 0.15);
        tf.scale.y = tf.scale.y.lerp(target, 0.15);
        tf.scale.z = tf.scale.z.lerp(target, 0.15);
    }
}

fn interact_hint_system(time: Res<Time>, mode: Res<RelaxMode>, cam: Single<&Transform, (With<Player>, Without<InteractMarker>)>, mut q: Query<(&mut Transform, &InteractMarker), Without<Player>>) {
    let t = time.elapsed_secs(); let p = cam.translation; let walking = *mode == RelaxMode::Walking;
    let d_ham = Vec2::new(p.x - HAMMOCK_POS.x, p.z - HAMMOCK_POS.z).length();
    let d_bar = Vec2::new(p.x - BAR_POS.x, p.z - BAR_POS.z).length();
    for (mut tf, m) in q.iter_mut() {
        let near = match m.kind {
            InteractKind::Hammock => walking && d_ham <= HAMMOCK_RADIUS,
            InteractKind::Bar => walking && d_bar <= BAR_RADIUS,
        };
        let base = if near { 0.35 } else { 0.001 };
        let bob = if near { 1.0 + (t * 3.0).sin() * 0.15 } else { 1.0 };
        let target = base * bob;
        tf.scale = tf.scale.lerp(Vec3::splat(target), 0.15);
    }
}

fn character_visibility(mode: Res<RelaxMode>, mut q: Query<&mut Visibility, With<CharacterPart>>) {
    let vis = if *mode == RelaxMode::Resting { Visibility::Visible } else { Visibility::Hidden };
    for mut v in q.iter_mut() { if *v != vis { *v = vis; } }
}

fn sway_system(time: Res<Time>, mut q: Query<(&mut Transform, &Sway)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in q.iter_mut() {
        let a = (t * 1.15 + s.phase).sin(); let b = (t * 0.78 + s.phase * 1.7).cos();
        tf.translation = s.base + Vec3::new(b * 0.01, a * 0.018, a * 0.05);
        tf.rotation = s.base_rot * Quat::from_rotation_x(a * 0.026) * Quat::from_rotation_z(b * 0.012);
    }
}

fn water_system(time: Res<Time>, mut q: Query<&mut Transform, With<Ocean>>) {
    let t = time.elapsed_secs();
    for mut tf in q.iter_mut() { tf.translation.y = -0.12 + (t * 0.5).sin() * 0.05; }
}

fn foam_system(time: Res<Time>, mut q: Query<(&mut Transform, &Foam)>) {
    let t = time.elapsed_secs();
    for (mut tf, f) in q.iter_mut() {
        let wave = (t * f.speed + f.phase).sin();
        tf.translation.z = f.base_z + wave * 1.1;
        tf.translation.y = 0.02 + wave.max(0.0) * 0.03;
        tf.scale.x = 60.0 + wave * 6.0;
        let fade = (wave * 0.5 + 0.5).clamp(0.2, 1.0);
        tf.scale.z = 0.5 * fade;
    }
}

fn sparkle_system(time: Res<Time>, mut q: Query<(&mut Transform, &Sparkle)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in q.iter_mut() {
        let tw = (t * 3.0 + s.phase).sin().max(0.0);
        tf.scale = Vec3::splat(0.06 + tw * 0.16);
    }
}

fn steam_system(time: Res<Time>, mut q: Query<(&mut Transform, &Steam)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in q.iter_mut() {
        let p = ((t * 0.5 + s.phase) % 1.0 + 1.0) % 1.0;
        tf.translation.y = s.base.y + p * 0.55;
        tf.translation.x = s.base.x + (p * 4.0).sin() * 0.03;
        let sc = 0.05 * (1.0 - p * 0.6);
        tf.scale = Vec3::splat(sc.max(0.001));
    }
}

fn cloud_system(time: Res<Time>, mut q: Query<(&mut Transform, &CloudPuff)>) {
    let dt = time.delta_secs();
    for (mut tf, cp) in q.iter_mut() {
        tf.translation.x += dt * cp.speed;
        if tf.translation.x > 360.0 { tf.translation.x = -360.0; }
    }
}

fn bird_system(time: Res<Time>, mut q: Query<(&mut Transform, &Bird)>) {
    let t = time.elapsed_secs();
    for (mut tf, b) in q.iter_mut() {
        let a = t * b.speed + b.phase;
        tf.translation = b.center + Vec3::new(a.cos() * b.radius, b.height + (a * 2.0).sin() * 1.1, a.sin() * b.radius * 0.55);
        tf.rotation = Quat::from_rotation_y(-(a + PI * 0.5));
        tf.scale.y = 0.2 * (0.4 + (a * 8.0).sin().abs() * 0.8);
    }
}

fn yacht_system(time: Res<Time>, mut q: Query<(&mut Transform, &Yacht)>) {
    let t = time.elapsed_secs();
    for (mut tf, y) in q.iter_mut() {
        let bob = (t * 0.8 + y.phase).sin(); let roll = (t * 0.6 + y.phase).sin();
        tf.translation.y = y.base.y + bob * 0.25;
        tf.rotation = Quat::from_rotation_z(roll * 0.05) * Quat::from_rotation_x(roll * 0.03);
    }
}

fn fps_update_system(time: Res<Time>, mut fps: ResMut<FpsCounter>) {
    fps.frame_count += 1; fps.time_accum += time.delta_secs();
    if fps.time_accum >= 0.5 {
        fps.current = fps.frame_count as f32 / fps.time_accum;
        fps.frame_count = 0; fps.time_accum = 0.0;
    }
}

fn fps_visibility_system(fps: Res<FpsCounter>, settings: Res<Settings>, mut q_text: Query<(&mut Text, &mut Visibility), With<FpsText>>) {
    for (mut text, mut vis) in q_text.iter_mut() {
        if settings.show_fps {
            *vis = Visibility::Visible;
            *text = Text::new(format!("FPS: {:.0}", fps.current));
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn slider_update_system(
    mut slider_query: Query<(&Interaction, &RelativeCursorPosition), With<VolumeSlider>>,
    mut handle_query: Query<&mut Node, With<VolumeSliderHandle>>,
    mut fill_query: Query<&mut Node, (With<VolumeSliderFill>, Without<VolumeSliderHandle>)>,
    mut label_query: Query<&mut Text, With<VolumeLabel>>,
    mut settings: ResMut<Settings>,
    audio: Option<NonSend<AmbientAudio>>,
) {
    for (interaction, rel_pos) in slider_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            if let Some(pos) = rel_pos.normalized {
                let new_vol = pos.x.clamp(0.0, 1.0);
                settings.volume = new_vol;
                if let Some(audio) = &audio { audio.sink.set_volume(new_vol); }
            }
        }
    }
    
    let vol = settings.volume;
    for mut handle in handle_query.iter_mut() {
        handle.left = Val::Px(20.0 + 200.0 * vol - 9.0);
    }
    for mut fill in fill_query.iter_mut() {
        fill.width = Val::Percent(vol * 100.0);
    }
    for mut text in label_query.iter_mut() {
        *text = Text::new(format!("{:.0}%", vol * 100.0));
    }
}

fn pause_toggle(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<GameState>>, mut cursor: Single<&mut CursorOptions>, mut menu_q: Query<&mut Visibility, (With<MenuRoot>, Without<FpsText>)>) {
    if keys.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Paused);
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        if let Ok(mut vis) = menu_q.single_mut() { *vis = Visibility::Visible; }
        println!("Пауза");
    }
}

fn menu_button_system(
    mut interaction_query: Query<(&Interaction, &MenuButton), (Changed<Interaction>, With<Button>)>,
    mut next_state: ResMut<NextState<GameState>>,
    mut settings: ResMut<Settings>,
    mut cursor: Single<&mut CursorOptions>,
    mut menu_q: Query<&mut Visibility, (With<MenuRoot>, Without<FpsText>)>,
    mut fps_toggle_text: Query<&mut Text, With<FpsToggleText>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            match button {
                MenuButton::Resume => {
                    next_state.set(GameState::Playing);
                    cursor.grab_mode = CursorGrabMode::Locked;
                    cursor.visible = false;
                    if let Ok(mut vis) = menu_q.single_mut() { *vis = Visibility::Hidden; }
                    println!("Возврат в игру");
                }
                MenuButton::FpsToggle => {
                    settings.show_fps = !settings.show_fps;
                    for mut text in fps_toggle_text.iter_mut() {
                        *text = Text::new(if settings.show_fps { "FPS: ON" } else { "FPS: OFF" });
                    }
                    println!("FPS: {}", if settings.show_fps { "ON" } else { "OFF" });
                }
                MenuButton::Quit => {
                    println!("Выход");
                    exit.write(AppExit::Success);
                }
            }
        }
    }
}