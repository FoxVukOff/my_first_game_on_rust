use bevy::camera::{ClearColor, Hdr};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::MouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, WindowResolution};
use std::f32::consts::PI;

// ---------- КОНСТАНТЫ ----------
const EYE: f32 = 1.62;
const WATER_Y: f32 = 0.0;
const SAND_TOP: f32 = 0.25;
const WORLD_X: f32 = 60.0;
const BEACH_MIN_Z: f32 = -1.5;
const BEACH_MAX_Z: f32 = 42.0;

const SKY: Color = Color::srgb(0.50, 0.73, 0.93);
const SUN_POS: Vec3 = Vec3::new(60.0, 42.0, -300.0);

const BAR_POS: Vec3 = Vec3::new(11.0, SAND_TOP, 19.0);

// ---------- РЕЖИМЫ / НАПИТКИ ----------
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum RelaxMode {
    #[default]
    Resting,
    Walking,
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum DrinkKind {
    #[default]
    Lemonade,
    Coffee,
}

#[derive(Resource, Default)]
struct DrinkState {
    holding: bool,
    selected: DrinkKind,
}

// ---------- КОМПОНЕНТЫ ----------
#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
    bob: f32,
}

#[derive(Component)]
struct Ocean;

#[derive(Component)]
struct Foam {
    base_z: f32,
    phase: f32,
    speed: f32,
}

#[derive(Component)]
struct Sparkle {
    phase: f32,
}

#[derive(Component)]
struct Steam {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct Sway {
    base: Vec3,
    base_rot: Quat,
    phase: f32,
}

#[derive(Component)]
struct CloudPuff {
    speed: f32,
}

#[derive(Component)]
struct Bird {
    center: Vec3,
    radius: f32,
    speed: f32,
    phase: f32,
    height: f32,
}

#[derive(Component)]
struct Yacht {
    base: Vec3,
    phase: f32,
}

#[derive(Component)]
struct DrinkGroup {
    kind: DrinkKind,
}

#[derive(Component)]
struct DrinkScale {
    current: f32,
}

#[derive(Component)]
struct BarIndicator {
    kind: DrinkKind,
}

// ---------- РЕСУРСЫ ----------
#[derive(Resource)]
struct Obstacles(Vec<(f32, f32, f32)>);

// ---------- ГЕНЕРАТОР ----------
fn next_rand(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    ((*rng >> 8) as f32) / 16_777_216.0
}

// ---------- MAIN ----------
fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "River Relax — Ocean Edition".into(),
                    resolution: WindowResolution::new(1920, 1080),
                    resizable: true,
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(ClearColor(SKY))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.70, 0.80, 0.95),
            brightness: 180.0,
            ..default()
        })
        .init_resource::<RelaxMode>()
        .init_resource::<DrinkState>()
        .add_systems(Startup, setup)
        .add_systems(Update, camera_control)
        .add_systems(Update, drink_input)
        .add_systems(Update, drink_animation.after(drink_input))
        .add_systems(Update, bar_indicator_system)
        .add_systems(Update, (sway_system, water_system, foam_system))
        .add_systems(Update, (sparkle_system, steam_system, cloud_system))
        .add_systems(Update, (bird_system, yacht_system))
        .run();
}

// ---------- SETUP ----------
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("===== OCEAN RELAX =====");
    println!("E      - встать / лечь в гамак");
    println!("WASD   - ходить");
    println!("Mouse  - осмотреться");
    println!("Shift  - быстрее");
    println!("SPACE  - достать / убрать напиток");
    println!("1      - выбрать ЛИМОНАД");
    println!("2      - выбрать КОФЕ");
    println!("Бар-стойка стоит на пляже справа.");
    println!("=======================");

    // ---- КАМЕРА: HDR + ACES + Bloom + туман ----
    let mut bloom = Bloom::default();
    bloom.intensity = 0.35;
    bloom.prefilter.threshold = 0.7;

    commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::AcesFitted,
        bloom,
        DistanceFog {
            color: SKY,
            falloff: FogFalloff::Linear {
                start: 60.0,
                end: 430.0,
            },
            ..default()
        },
        Transform::from_xyz(-1.0, 1.15, 10.0),
        Player {
            yaw: 0.0,
            pitch: 0.0,
            bob: 0.0,
        },
    ));

    // ---- СОЛНЦЕ (направленный свет) ----
    let to_sun = SUN_POS.normalize();
    commands.spawn((
        DirectionalLight {
            illuminance: 34_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.94, 0.82),
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, -to_sun)),
        Visibility::default(),
    ));

    // ---- МАТЕРИАЛЫ ----
    let mat_ocean = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.20, 0.38),
        metallic: 0.40,
        perceptual_roughness: 0.05,
        emissive: LinearRgba::from(Color::srgb(0.0, 0.05, 0.10)) * 1.6,
        ..default()
    });
    let mat_shallow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.45, 0.55),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.30,
        perceptual_roughness: 0.08,
        emissive: LinearRgba::from(Color::srgb(0.02, 0.12, 0.14)) * 1.4,
        ..default()
    });
    let mat_seabed = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.34, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mat_foam = materials.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.99, 1.0),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::from(Color::srgb(0.5, 0.55, 0.6)) * 1.1,
        ..default()
    });
    let mat_sand = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.86, 0.66),
        perceptual_roughness: 0.98,
        ..default()
    });
    let mat_sand_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.76, 0.55),
        perceptual_roughness: 0.9,
        ..default()
    });
    let mat_wet = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.49, 0.36),
        metallic: 0.25,
        perceptual_roughness: 0.12,
        ..default()
    });
    let mat_bark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.38, 0.26, 0.16),
        perceptual_roughness: 0.92,
        ..default()
    });
    let mat_frond = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.5, 0.22),
        perceptual_roughness: 0.72,
        ..default()
    });
    let mat_coco = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.2, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_cloth = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.32, 0.24),
        perceptual_roughness: 0.72,
        ..default()
    });
    let mat_cloth2 = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.88, 0.72),
        perceptual_roughness: 0.75,
        ..default()
    });
    let mat_rope = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.62, 0.44),
        perceptual_roughness: 0.9,
        ..default()
    });
    let mat_skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.77, 0.62),
        perceptual_roughness: 0.65,
        ..default()
    });
    let mat_shirt = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.46, 0.74),
        perceptual_roughness: 0.78,
        ..default()
    });
    let mat_shorts = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.62, 0.2),
        perceptual_roughness: 0.8,
        ..default()
    });
    let mat_hat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.82, 0.55),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.52, 0.5),
        perceptual_roughness: 0.88,
        ..default()
    });
    let mat_bush = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.44, 0.2),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_island = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.36, 0.24),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mat_forest = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.34, 0.19),
        perceptual_roughness: 0.9,
        ..default()
    });
    let mat_cloud = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.99, 0.97),
        emissive: LinearRgba::from(Color::srgb(0.55, 0.58, 0.65)) * 1.4,
        perceptual_roughness: 1.0,
        ..default()
    });
    let mat_bird = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.11),
        perceptual_roughness: 1.0,
        ..default()
    });
    let mat_sun = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.96, 0.80),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.87, 0.55)) * 28.0,
        ..default()
    });
    let mat_halo = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.7),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::from(Color::srgb(1.0, 0.85, 0.5)) * 2.8,
        ..default()
    });
    let mat_sparkle = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.98, 0.9),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.95, 0.8)) * 8.0,
        unlit: true,
        ..default()
    });
    // Напитки
    let mat_lemonade = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.22),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.85, 0.15)) * 3.0,
        perceptual_roughness: 0.2,
        ..default()
    });
    let mat_lemon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.2),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.88, 0.1)) * 3.5,
        perceptual_roughness: 0.3,
        ..default()
    });
    let mat_straw = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.3, 0.3),
        emissive: LinearRgba::from(Color::srgb(0.4, 0.08, 0.08)) * 1.2,
        perceptual_roughness: 0.4,
        ..default()
    });
    let mat_coffee = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.16, 0.08),
        emissive: LinearRgba::from(Color::srgb(0.25, 0.12, 0.05)) * 1.6,
        perceptual_roughness: 0.4,
        ..default()
    });
    let mat_mug = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.9, 0.86),
        perceptual_roughness: 0.35,
        ..default()
    });
    let mat_steam = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::from(Color::srgb(0.8, 0.85, 0.9)) * 0.8,
        ..default()
    });
    // Бар
    let mat_wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.44, 0.26),
        perceptual_roughness: 0.82,
        ..default()
    });
    let mat_wood_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.30, 0.17),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_canopy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.66, 0.16),
        perceptual_roughness: 0.7,
        ..default()
    });
    let mat_board = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.16, 0.12),
        emissive: LinearRgba::from(Color::srgb(0.12, 0.09, 0.06)) * 1.2,
        perceptual_roughness: 0.9,
        ..default()
    });

    // ---- МЕШИ ----
    let m_sphere = meshes.add(Sphere::new(1.0));
    let m_cuboid = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let m_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    // ---- ОКЕАН: дно + глубина + мелководье ----
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_seabed.clone(), Vec3::new(0.0, -1.4, -200.0), Vec3::new(800.0, 0.4, 420.0));
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_ocean.clone()),
        Transform {
            translation: Vec3::new(0.0, -0.12, -200.0),
            scale: Vec3::new(800.0, 0.25, 420.0),
            ..default()
        },
        Ocean,
    ));
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_shallow.clone()),
        Transform {
            translation: Vec3::new(0.0, -0.03, -8.0),
            scale: Vec3::new(300.0, 0.12, 30.0),
            ..default()
        },
    ));

    // ---- ПЕННЫЕ ВОЛНЫ (накатывают на берег) ----
    for (i, bz) in [0.6, 1.6, 2.6, 3.8].iter().enumerate() {
        commands.spawn((
            Mesh3d(m_cuboid.clone()),
            MeshMaterial3d(mat_foam.clone()),
            Transform {
                translation: Vec3::new(0.0, 0.02, *bz),
                scale: Vec3::new(60.0, 0.03, 0.5),
                ..default()
            },
            Foam {
                base_z: *bz,
                phase: i as f32 * 1.7,
                speed: 0.5 + i as f32 * 0.08,
            },
        ));
    }

    // ---- ПЛЯЖ (широкий) ----
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_sand.clone(), Vec3::new(0.0, 0.05, 26.0), Vec3::new(240.0, SAND_TOP, 56.0));
    // Дюны
    for (dx, dz, r) in [
        (-40.0, 36.0, 7.0), (-18.0, 40.0, 5.5), (20.0, 38.0, 7.5),
        (42.0, 34.0, 6.0), (55.0, 40.0, 5.0), (-55.0, 38.0, 6.5),
    ] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_sand_dark.clone(), Vec3::new(dx, SAND_TOP - r * 0.4, dz), Vec3::new(r * 2.0, r * 0.5, r * 1.4));
    }
    // Мокрый песок у кромки
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_wet.clone(), Vec3::new(0.0, SAND_TOP + 0.006, 1.6), Vec3::new(240.0, 0.02, 3.2));

    // ---- ОСТРОВ НА ГОРИЗОНТЕ ----
    for (x, z, r) in [(-140.0, -360.0, 46.0), (150.0, -380.0, 60.0), (20.0, -400.0, 40.0)] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_island.clone(), Vec3::new(x, -r * 0.55, z), Vec3::new(r, r * 0.5, r * 0.7));
    }

    // ---- ДАЛЬНИЙ ЛЕС НА ПЛЯЖЕ ----
    let mut rng: u32 = 1234;
    for _ in 0..40 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let x = -80.0 + r1 * 160.0;
        let z = 34.0 + r2 * 8.0;
        let h = 2.4 + r2 * 2.6;
        spawn_scaled(&mut commands, m_cyl.clone(), mat_bark.clone(), Vec3::new(x, SAND_TOP + h * 0.5, z), Vec3::new(0.28, h, 0.28));
        spawn_scaled(&mut commands, m_sphere.clone(), mat_forest.clone(), Vec3::new(x, SAND_TOP + h + 0.6, z), Vec3::new(2.0 + r1, 1.7 + r2, 2.0 + r2));
    }

    // ---- ПАЛЬМЫ ----
    let mut obstacles: Vec<(f32, f32, f32)> = vec![];
    for (x, z, lean, height) in [
        (-3.2, 10.0, 0.055, 5.4), (3.2, 10.0, -0.055, 5.4),
        (-12.0, 14.0, 0.10, 6.2), (14.0, 12.0, -0.11, 6.6),
        (-22.0, 18.0, 0.07, 5.8), (24.0, 20.0, -0.08, 6.0),
        (-34.0, 15.0, 0.12, 5.2), (-8.0, 26.0, 0.06, 5.6),
        (18.0, 27.0, -0.09, 6.0), (30.0, 16.0, 0.05, 5.4),
    ] {
        spawn_palm(&mut commands, &m_cuboid, &m_sphere, &mat_bark, &mat_frond, &mat_coco, Vec3::new(x, SAND_TOP, z), lean, height);
        obstacles.push((x, z, 0.55));
    }

    // ---- ГАМАК + ПЕРСОНАЖ ----
    let left = Vec3::new(-3.2, SAND_TOP, 10.0);
    let right = Vec3::new(3.2, SAND_TOP, 10.0);
    spawn_hammock(&mut commands, &m_cuboid, &m_cyl, &mat_cloth, &mat_cloth2, &mat_rope, left, right);
    spawn_character(&mut commands, &m_sphere, &m_cuboid, &m_cyl, &mat_skin, &mat_shirt, &mat_shorts, &mat_hat);

    // ---- БАР-СТОЙКА ----
    spawn_bar(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood, &mat_wood_dark, &mat_canopy, &mat_board, &mat_lemonade, &mat_coffee);
    obstacles.push((BAR_POS.x, BAR_POS.z, 1.9));

    // ---- ЗОНТИК + ПОЛОТЕНЦЕ près гамака ----
    let umb = Vec3::new(-7.0, SAND_TOP, 13.0);
    spawn_scaled(&mut commands, m_cyl.clone(), mat_wood.clone(), umb + Vec3::new(0.12, 1.05, 0.0), Vec3::new(0.035, 2.1, 0.035));
    spawn_scaled(&mut commands, m_sphere.clone(), mat_canopy.clone(), umb + Vec3::new(0.24, 1.98, 0.0), Vec3::new(2.6, 0.8, 2.6));
    obstacles.push((umb.x, umb.z, 0.4));
    // Полотенце
    spawn_scaled(&mut commands, m_cuboid.clone(), mat_cloth.clone(), Vec3::new(-7.0, SAND_TOP + 0.03, 15.0), Vec3::new(1.6, 0.03, 2.6));

    // ---- ДЕКОР: камни, кусты, ракушки ----
    for (x, z, r, sy) in [
        (-14.0, 8.0, 0.9, 0.5), (-12.5, 7.2, 0.5, 0.35),
        (16.0, 9.0, 1.0, 0.55), (18.0, 8.2, 0.55, 0.32),
        (-28.0, 12.0, 1.2, 0.6), (28.0, 14.0, 0.9, 0.45),
        (6.0, 24.0, 0.8, 0.4), (-20.0, 26.0, 1.0, 0.5),
    ] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_rock.clone(), Vec3::new(x, SAND_TOP - r * sy * 0.6, z), Vec3::new(r, r * sy, r * 0.85));
        obstacles.push((x, z, r * 0.9));
    }
    for (x, z, r) in [
        (-18.0, 22.0, 1.3), (-15.0, 24.0, 0.9),
        (22.0, 23.0, 1.2), (26.0, 20.0, 0.8),
        (-32.0, 24.0, 1.1), (10.0, 28.0, 1.0),
    ] {
        spawn_scaled(&mut commands, m_sphere.clone(), mat_bush.clone(), Vec3::new(x, SAND_TOP + r * 0.25, z), Vec3::new(r * 1.4, r * 0.8, r * 1.3));
        obstacles.push((x, z, r * 0.8));
    }
    for _ in 0..26 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let x = -40.0 + r1 * 80.0;
        let z = 4.0 + r2 * 30.0;
        spawn_scaled(&mut commands, m_sphere.clone(), mat_sand_dark.clone(), Vec3::new(x, SAND_TOP + 0.02, z), Vec3::new(0.12 + r2 * 0.08, 0.05, 0.1 + r1 * 0.06));
    }

    // ---- НЕБО: солнце + гало ----
    spawn_scaled(&mut commands, m_sphere.clone(), mat_sun.clone(), SUN_POS, Vec3::new(16.0, 16.0, 16.0));
    spawn_scaled(&mut commands, m_sphere.clone(), mat_halo.clone(), SUN_POS * 0.995, Vec3::new(42.0, 42.0, 42.0));

    // ---- ОБЛАКА (большие, медленные) ----
    for (_i, (x, y, z, s)) in [
        (-80.0, 34.0, -90.0, 1.2), (-20.0, 40.0, -140.0, 1.6),
        (60.0, 32.0, -80.0, 1.0), (130.0, 38.0, -150.0, 1.7),
        (-150.0, 36.0, -60.0, 1.3), (20.0, 44.0, 60.0, 1.5),
        (-90.0, 42.0, 80.0, 1.2), (110.0, 34.0, 100.0, 1.4),
    ].into_iter().enumerate() {
        for (dx, dy, dz, r) in [
            (0.0, 0.0, 0.0, 7.0), (6.5, -0.8, 1.5, 5.0),
            (-6.0, -1.0, -1.0, 5.5), (3.0, 1.6, -2.5, 4.2),
            (-3.5, 1.2, 2.5, 4.6), (10.5, -1.4, 0.5, 3.6),
        ] {
            commands.spawn((
                Mesh3d(m_sphere.clone()),
                MeshMaterial3d(mat_cloud.clone()),
                Transform {
                    translation: Vec3::new(x + dx * s, y + dy * s, z + dz * s),
                    scale: Vec3::new(r * s, r * s * 0.45, r * s * 0.8),
                    ..default()
                },
                CloudPuff { speed: 0.012 + next_rand(&mut rng) * 0.02 },
            ));
        }
    }

    // ---- ЧАЙКИ над океаном ----
    for (i, (cx, cz, rad, h)) in [
        (0.0, -30.0, 30.0, 16.0), (20.0, -15.0, 22.0, 19.0),
        (-25.0, -35.0, 34.0, 14.0), (8.0, -5.0, 26.0, 22.0),
        (-12.0, -12.0, 18.0, 18.0),
    ].into_iter().enumerate() {
        for k in 0..3 {
            commands.spawn((
                Mesh3d(m_sphere.clone()),
                MeshMaterial3d(mat_bird.clone()),
                Transform {
                    translation: Vec3::new(cx + k as f32 * 0.6, h, cz),
                    scale: Vec3::new(0.55, 0.2, 0.22),
                    ..default()
                },
                Bird {
                    center: Vec3::new(cx, 0.0, cz),
                    radius: rad,
                    speed: 0.08 + i as f32 * 0.012,
                    phase: k as f32 * 0.5 + i as f32 * 1.3,
                    height: h,
                },
            ));
        }
    }

    // ---- ЯХТЫ в океане ----
    spawn_yacht(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood_dark, &mat_cloth2, Vec3::new(-30.0, 0.0, -60.0), 0.0);
    spawn_yacht(&mut commands, &m_cuboid, &m_cyl, &m_sphere, &mat_wood_dark, &mat_cloth, Vec3::new(40.0, 0.0, -90.0), 1.5);

    // ---- СОЛНЕЧНЫЕ ЗАЙЧИКИ на воде ----
    for _ in 0..46 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let x = -60.0 + r1 * 120.0;
        let z = -4.0 - r2 * 60.0;
        commands.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_sparkle.clone()),
            Transform {
                translation: Vec3::new(x, 0.02, z),
                scale: Vec3::splat(0.12),
                ..default()
            },
            Sparkle { phase: r1 * 12.0 },
        ));
    }

    // ---- НАПИТКИ В КАРМАНЕ (два: лимонад и кофе) ----
    let hand_start = Vec3::new(0.0, -5.0, 0.0); // спрятаны, пока не достанешь

    // ЛИМОНАД (большой, без кружки)
    commands.spawn((
        Transform::from_translation(hand_start),
        Visibility::default(),
        DrinkGroup { kind: DrinkKind::Lemonade },
        DrinkScale { current: 0.0 },
    )).with_children(|parent| {
        // Большая порция лимонада
        parent.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_lemonade.clone()),
            Transform { translation: Vec3::ZERO, scale: Vec3::new(0.24, 0.8, 0.24), ..default() },
        ));
        // Долька лимона сверху
        parent.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_lemon.clone()),
            Transform { translation: Vec3::new(0.0, 0.42, 0.0), scale: Vec3::new(0.3, 0.1, 0.3), rotation: Quat::from_rotation_z(0.3), ..default() },
        ));
        // Долька сбоку
        parent.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_lemon.clone()),
            Transform { translation: Vec3::new(0.22, 0.12, 0.0), scale: Vec3::new(0.16, 0.05, 0.16), ..default() },
        ));
        // Трубочка
        parent.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_straw.clone()),
            Transform { translation: Vec3::new(0.07, 0.55, 0.0), scale: Vec3::new(0.03, 0.7, 0.03), rotation: Quat::from_rotation_z(-0.18), ..default() },
        ));
    });

    // КОФЕ (в кружке, с паром)
    commands.spawn((
        Transform::from_translation(hand_start),
        Visibility::default(),
        DrinkGroup { kind: DrinkKind::Coffee },
        DrinkScale { current: 0.0 },
    )).with_children(|parent| {
        // Кружка
        parent.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_mug.clone()),
            Transform { translation: Vec3::ZERO, scale: Vec3::new(0.2, 0.36, 0.2), ..default() },
        ));
        // Кофе сверху
        parent.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_coffee.clone()),
            Transform { translation: Vec3::new(0.0, 0.17, 0.0), scale: Vec3::new(0.18, 0.03, 0.18), ..default() },
        ));
        // Ручка кружки (из маленьких сфер по дуге)
        for i in 0..5 {
            let a = -0.9 + i as f32 * 0.45;
            parent.spawn((
                Mesh3d(m_sphere.clone()),
                MeshMaterial3d(mat_mug.clone()),
                Transform { translation: Vec3::new(0.22 + a.cos() * 0.08, a.sin() * 0.11, 0.0), scale: Vec3::splat(0.045), ..default() },
            ));
        }
        // Пар
        for i in 0..3 {
            parent.spawn((
                Mesh3d(m_sphere.clone()),
                MeshMaterial3d(mat_steam.clone()),
                Transform { translation: Vec3::new((i as f32 - 1.0) * 0.06, 0.3, 0.0), scale: Vec3::splat(0.05), ..default() },
                Steam { base: Vec3::new((i as f32 - 1.0) * 0.06, 0.3, 0.0), phase: i as f32 * 0.4 },
            ));
        }
    });

    commands.insert_resource(Obstacles(obstacles));
}

// ---------- ХЕЛПЕРЫ ----------
fn spawn_scaled(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    translation: Vec3,
    scale: Vec3,
) {
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform { translation, scale, ..default() },
    ));
}

fn spawn_beam(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
    thickness: f32,
) {
    let dir = end - start;
    let len = dir.length();
    if len <= f32::EPSILON { return; }
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material.clone()),
        Transform {
            translation: start + dir * 0.5,
            rotation: Quat::from_rotation_arc(Vec3::Y, dir.normalize()),
            scale: Vec3::new(thickness, len, thickness),
        },
    ));
}

fn spawn_palm(
    commands: &mut Commands,
    m_cuboid: &Handle<Mesh>,
    m_sphere: &Handle<Mesh>,
    mat_bark: &Handle<StandardMaterial>,
    mat_frond: &Handle<StandardMaterial>,
    mat_coco: &Handle<StandardMaterial>,
    pos: Vec3,
    lean: f32,
    height: f32,
) {
    let segments = 8;
    let seg_h = height / segments as f32;
    let mut cur = pos;
    let mut ang = 0.0;

    for i in 0..segments {
        ang += lean;
        let dir = Vec3::new(ang.sin(), ang.cos(), 0.0);
        let w = 0.42 - i as f32 * 0.028;
        commands.spawn((
            Mesh3d(m_cuboid.clone()),
            MeshMaterial3d(mat_bark.clone()),
            Transform {
                translation: cur + dir * seg_h * 0.5,
                rotation: Quat::from_rotation_z(-ang),
                scale: Vec3::new(w, seg_h * 1.06, w),
            },
        ));
        cur += dir * seg_h;
    }

    for (dx, dz) in [(0.25, 0.15), (-0.22, 0.2), (0.02, -0.28)] {
        commands.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_coco.clone()),
            Transform {
                translation: Vec3::new(cur.x + dx, cur.y - 0.28, cur.z + dz),
                scale: Vec3::splat(0.19),
                ..default()
            },
        ));
    }

    for i in 0..9 {
        let a = (i as f32 / 9.0) * PI * 2.0 + lean * 2.0;
        let droop = 0.42 + (i % 3) as f32 * 0.12;
        let len = 2.6 + (i % 4) as f32 * 0.35;
        let base_rot = Quat::from_rotation_y(a) * Quat::from_rotation_x(droop);
        let offset = base_rot * Vec3::new(0.0, 0.12, -len * 0.5);
        commands.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_frond.clone()),
            Transform {
                translation: cur + offset,
                rotation: base_rot,
                scale: Vec3::new(0.5, 0.07, len),
            },
            Sway { base: cur + offset, base_rot, phase: i as f32 * 0.9 + pos.x },
        ));
    }
}

fn spawn_hammock(
    commands: &mut Commands,
    m_cuboid: &Handle<Mesh>,
    m_cyl: &Handle<Mesh>,
    mat_cloth: &Handle<StandardMaterial>,
    mat_cloth2: &Handle<StandardMaterial>,
    mat_rope: &Handle<StandardMaterial>,
    left: Vec3,
    right: Vec3,
) {
    let z = left.z;
    let anchor_l = left + Vec3::new(0.15, 2.55, 0.0);
    let anchor_r = right + Vec3::new(-0.15, 2.55, 0.0);

    for (x, y, len, mat) in [
        (-2.05, 0.72, 1.5, mat_cloth2.clone()),
        (0.0, 0.62, 3.2, mat_cloth.clone()),
        (2.05, 0.72, 1.5, mat_cloth2.clone()),
    ] {
        commands.spawn((
            Mesh3d(m_cuboid.clone()),
            MeshMaterial3d(mat),
            Transform {
                translation: Vec3::new(x, y, z),
                scale: Vec3::new(len, 0.07, 0.95),
                ..default()
            },
            Sway { base: Vec3::new(x, y, z), base_rot: Quat::IDENTITY, phase: 0.0 },
        ));
    }

    for dz in [-0.42, -0.14, 0.14, 0.42] {
        spawn_beam(commands, m_cuboid, mat_rope, anchor_l, Vec3::new(-2.72, 0.72, z + dz * 0.8), 0.028);
        spawn_beam(commands, m_cuboid, mat_rope, anchor_r, Vec3::new(2.72, 0.72, z + dz * 0.8), 0.028);
    }

    for x in [-2.68, 2.68] {
        for dz in [-0.3, 0.0, 0.3] {
            commands.spawn((
                Mesh3d(m_cyl.clone()),
                MeshMaterial3d(mat_rope.clone()),
                Transform {
                    translation: Vec3::new(x, 0.62, z + dz),
                    scale: Vec3::new(0.02, 0.16, 0.02),
                    ..default()
                },
            ));
        }
    }
}

fn spawn_character(
    commands: &mut Commands,
    m_sphere: &Handle<Mesh>,
    m_cuboid: &Handle<Mesh>,
    m_cyl: &Handle<Mesh>,
    mat_skin: &Handle<StandardMaterial>,
    mat_shirt: &Handle<StandardMaterial>,
    mat_shorts: &Handle<StandardMaterial>,
    mat_hat: &Handle<StandardMaterial>,
) {
    let z = 10.0;

    let parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Vec3, Vec3, Quat)> = vec![
        (m_sphere.clone(), mat_skin.clone(), Vec3::new(-2.02, 0.9, z), Vec3::new(0.38, 0.42, 0.38), Quat::IDENTITY),
        (m_sphere.clone(), mat_hat.clone(), Vec3::new(-2.02, 1.06, z), Vec3::new(0.92, 0.1, 0.92), Quat::IDENTITY),
        (m_cyl.clone(), mat_hat.clone(), Vec3::new(-2.02, 1.16, z), Vec3::new(0.42, 0.22, 0.42), Quat::IDENTITY),
        (m_cuboid.clone(), mat_shirt.clone(), Vec3::new(-1.15, 0.79, z), Vec3::new(1.15, 0.3, 0.46), Quat::from_rotation_z(0.05)),
        (m_cuboid.clone(), mat_shorts.clone(), Vec3::new(-0.35, 0.75, z), Vec3::new(0.55, 0.26, 0.44), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(0.55, 0.72, z - 0.12), Vec3::new(1.2, 0.17, 0.17), Quat::from_rotation_z(-0.04)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(0.55, 0.72, z + 0.12), Vec3::new(1.2, 0.17, 0.17), Quat::from_rotation_z(-0.04)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(1.2, 0.7, z - 0.12), Vec3::new(0.24, 0.13, 0.2), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(1.2, 0.7, z + 0.12), Vec3::new(0.24, 0.13, 0.2), Quat::IDENTITY),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(-1.35, 0.86, z - 0.32), Vec3::new(0.75, 0.14, 0.14), Quat::from_rotation_z(0.5)),
        (m_cuboid.clone(), mat_skin.clone(), Vec3::new(-0.82, 0.86, z + 0.3), Vec3::new(0.8, 0.14, 0.14), Quat::from_rotation_z(-0.25)),
    ];

    for (mesh, mat, pos, scale, rot) in parts {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform { translation: pos, rotation: rot, scale, ..default() },
            Sway { base: pos, base_rot: rot, phase: 0.0 },
        ));
    }
}

fn spawn_bar(
    commands: &mut Commands,
    m_cuboid: &Handle<Mesh>,
    m_cyl: &Handle<Mesh>,
    m_sphere: &Handle<Mesh>,
    mat_wood: &Handle<StandardMaterial>,
    mat_wood_dark: &Handle<StandardMaterial>,
    mat_canopy: &Handle<StandardMaterial>,
    mat_board: &Handle<StandardMaterial>,
    mat_lemonade: &Handle<StandardMaterial>,
    mat_coffee: &Handle<StandardMaterial>,
) {
    // Стойка (прилавок)
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_wood.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.0, 0.55, 0.0),
            scale: Vec3::new(3.4, 1.1, 1.0),
            ..default()
        },
    ));
    // Столешница
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_wood_dark.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.0, 1.12, 0.0),
            scale: Vec3::new(3.7, 0.08, 1.2),
            ..default()
        },
    ));
    // Большой зонт-крыша
    commands.spawn((
        Mesh3d(m_cyl.clone()),
        MeshMaterial3d(mat_wood.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.0, 1.6, -0.6),
            scale: Vec3::new(0.06, 3.2, 0.06),
            ..default()
        },
    ));
    commands.spawn((
        Mesh3d(m_sphere.clone()),
        MeshMaterial3d(mat_canopy.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.0, 3.1, -0.6),
            scale: Vec3::new(4.6, 1.2, 4.6),
            ..default()
        },
    ));
    // Меню-доска
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_board.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.0, 1.8, 0.6),
            scale: Vec3::new(2.2, 1.0, 0.06),
            ..default()
        },
    ));
    // Табуретки
    for dx in [-1.2, 1.2] {
        commands.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_wood_dark.clone()),
            Transform {
                translation: BAR_POS + Vec3::new(dx, 0.35, 1.4),
                scale: Vec3::new(0.35, 0.7, 0.35),
                ..default()
            },
        ));
    }
    // Индикаторы выбора напитка (жёлтый = лимонад, коричневый = кофе)
    commands.spawn((
        Mesh3d(m_sphere.clone()),
        MeshMaterial3d(mat_lemonade.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(-0.7, 1.45, 0.0),
            scale: Vec3::splat(0.22),
            ..default()
        },
        BarIndicator { kind: DrinkKind::Lemonade },
    ));
    commands.spawn((
        Mesh3d(m_sphere.clone()),
        MeshMaterial3d(mat_coffee.clone()),
        Transform {
            translation: BAR_POS + Vec3::new(0.7, 1.45, 0.0),
            scale: Vec3::splat(0.0), // кофе не выбран по умолчанию
            ..default()
        },
        BarIndicator { kind: DrinkKind::Coffee },
    ));
}

fn spawn_yacht(
    commands: &mut Commands,
    m_cuboid: &Handle<Mesh>,
    m_cyl: &Handle<Mesh>,
    m_sphere: &Handle<Mesh>,
    mat_hull: &Handle<StandardMaterial>,
    mat_sail: &Handle<StandardMaterial>,
    pos: Vec3,
    phase: f32,
) {
    // Корпус
    commands.spawn((
        Mesh3d(m_sphere.clone()),
        MeshMaterial3d(mat_hull.clone()),
        Transform {
            translation: pos,
            scale: Vec3::new(3.4, 0.9, 1.4),
            ..default()
        },
        Yacht { base: pos, phase },
    ));
    // Мачта
    commands.spawn((
        Mesh3d(m_cyl.clone()),
        MeshMaterial3d(mat_hull.clone()),
        Transform {
            translation: pos + Vec3::new(0.0, 2.6, 0.0),
            scale: Vec3::new(0.08, 5.2, 0.08),
            ..default()
        },
        Yacht { base: pos + Vec3::new(0.0, 2.6, 0.0), phase },
    ));
    // Парус
    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_sail.clone()),
        Transform {
            translation: pos + Vec3::new(0.5, 3.2, 0.0),
            scale: Vec3::new(0.06, 4.0, 2.2),
            ..default()
        },
        Yacht { base: pos + Vec3::new(0.5, 3.2, 0.0), phase },
    ));
    let _ = m_cuboid;
}

// ---------- СИСТЕМЫ ----------
fn ground_height(z: f32) -> f32 {
    if z >= 1.0 {
        SAND_TOP
    } else if z <= -1.0 {
        WATER_Y
    } else {
        let t = (z + 1.0) / 2.0;
        WATER_Y + t * (SAND_TOP - WATER_Y)
    }
}

fn camera_control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse: MessageReader<MouseMotion>,
    mut mode: ResMut<RelaxMode>,
    mut cursor: Single<&mut CursorOptions>,
    cam: Single<(&mut Transform, &mut Player)>,
    obstacles: Res<Obstacles>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let (mut tf, mut player) = cam.into_inner();

    if keys.just_pressed(KeyCode::KeyE) {
        *mode = if *mode == RelaxMode::Resting { RelaxMode::Walking } else { RelaxMode::Resting };
        match *mode {
            RelaxMode::Walking => {
                cursor.grab_mode = CursorGrabMode::Locked;
                cursor.visible = false;
                tf.translation = Vec3::new(0.0, ground_height(14.0) + EYE, 14.0);
                player.yaw = 0.0;
                player.pitch = -0.03;
                player.bob = 0.0;
            }
            RelaxMode::Resting => {
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
                player.yaw = 0.0;
                player.pitch = 0.0;
                player.bob = 0.0;
            }
        }
    }

    for ev in mouse.read() {
        match *mode {
            RelaxMode::Walking => {
                player.yaw -= ev.delta.x * 0.0022;
                player.pitch = (player.pitch - ev.delta.y * 0.0022).clamp(-1.35, 1.35);
            }
            RelaxMode::Resting => {
                player.yaw = (player.yaw - ev.delta.x * 0.0012).clamp(-0.9, 0.9);
                player.pitch = (player.pitch - ev.delta.y * 0.001).clamp(-0.5, 0.6);
            }
        }
    }

    match *mode {
        RelaxMode::Resting => {
            let a = (t * 1.15).sin();
            let b = (t * 0.78).cos();
            tf.translation = Vec3::new(-1.0, 1.15 + a * 0.018, 10.0 + b * 0.012);
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
                    let d = Vec2::new(next.x - ox, next.z - oz);
                    let len = d.length();
                    if len < r && len > 1e-4 {
                        let push = d / len * r;
                        next.x = ox + push.x;
                        next.z = oz + push.y;
                    }
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

fn drink_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DrinkState>,
) {
    if keys.just_pressed(KeyCode::Space) {
        state.holding = !state.holding;
        if state.holding {
            println!("Достал напиток: {:?}", state.selected);
        } else {
            println!("Убрал напиток в карман");
        }
    }
    if keys.just_pressed(KeyCode::Digit1) {
        state.selected = DrinkKind::Lemonade;
        println!("Выбран ЛИМОНАД");
    }
    if keys.just_pressed(KeyCode::Digit2) {
        state.selected = DrinkKind::Coffee;
        println!("Выбран КОФЕ");
    }
}

fn drink_animation(
    time: Res<Time>,
    state: Res<DrinkState>,
    cam: Single<&Transform, (With<Player>, Without<DrinkGroup>)>,
    mut q: Query<(&mut Transform, &mut DrinkScale, &DrinkGroup), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let alpha = (dt * 6.0).min(1.0);
    let t = time.elapsed_secs();

    let cam_tf = cam.into_inner();
    // Позиция "в руке" — справа-снизу перед камерой
    let hand_local = Vec3::new(0.55, -0.5, -1.15);
    let hand_world = cam_tf.translation + cam_tf.rotation * hand_local;

    for (mut tf, mut ds, dg) in q.iter_mut() {
        let target = if state.holding && dg.kind == state.selected { 1.0 } else { 0.0 };
        ds.current = ds.current.lerp(target, alpha);

        if ds.current > 0.01 {
            // Покачивание в руке
            let sway = Vec3::new((t * 1.4).sin() * 0.02, (t * 2.0).cos() * 0.015, 0.0);
            tf.translation = hand_world + sway;
            tf.rotation = cam_tf.rotation * Quat::from_rotation_z((t * 1.0).sin() * 0.06);
        } else {
            tf.translation = Vec3::new(0.0, -5.0, 0.0);
        }
        tf.scale = Vec3::splat(ds.current.max(0.0001));
    }
}

fn bar_indicator_system(
    state: Res<DrinkState>,
    mut q: Query<(&mut Transform, &BarIndicator)>,
) {
    for (mut tf, ind) in q.iter_mut() {
        let on = ind.kind == state.selected;
        let target = if on { 0.22 } else { 0.0 };
        tf.scale.x = tf.scale.x.lerp(target, 0.15);
        tf.scale.y = tf.scale.y.lerp(target, 0.15);
        tf.scale.z = tf.scale.z.lerp(target, 0.15);
    }
}

fn sway_system(time: Res<Time>, mut q: Query<(&mut Transform, &Sway)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in q.iter_mut() {
        let a = (t * 1.15 + s.phase).sin();
        let b = (t * 0.78 + s.phase * 1.7).cos();
        tf.translation = s.base + Vec3::new(b * 0.01, a * 0.018, a * 0.05);
        tf.rotation = s.base_rot * Quat::from_rotation_x(a * 0.026) * Quat::from_rotation_z(b * 0.012);
    }
}

fn water_system(time: Res<Time>, mut q: Query<&mut Transform, With<Ocean>>) {
    let t = time.elapsed_secs();
    for mut tf in q.iter_mut() {
        tf.translation.y = -0.12 + (t * 0.5).sin() * 0.05;
    }
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
        tf.scale = Vec3::splat(0.06 + tw * 0.14);
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
    let t = time.elapsed_secs();
    for (mut tf, cp) in q.iter_mut() {
        let span = 700.0;
        tf.translation.x = (tf.translation.x + t * cp.speed).rem_euclid(span) - span * 0.5;
    }
}

fn bird_system(time: Res<Time>, mut q: Query<(&mut Transform, &Bird)>) {
    let t = time.elapsed_secs();
    for (mut tf, b) in q.iter_mut() {
        let a = t * b.speed + b.phase;
        tf.translation = b.center + Vec3::new(
            a.cos() * b.radius,
            b.height + (a * 2.0).sin() * 1.1,
            a.sin() * b.radius * 0.55,
        );
        tf.rotation = Quat::from_rotation_y(-(a + PI * 0.5));
        tf.scale.y = 0.2 * (0.4 + (a * 8.0).sin().abs() * 0.8);
    }
}

fn yacht_system(time: Res<Time>, mut q: Query<(&mut Transform, &Yacht)>) {
    let t = time.elapsed_secs();
    for (mut tf, y) in q.iter_mut() {
        let bob = (t * 0.8 + y.phase).sin();
        let roll = (t * 0.6 + y.phase).sin();
        tf.translation.y = y.base.y + bob * 0.25;
        tf.rotation = Quat::from_rotation_z(roll * 0.05) * Quat::from_rotation_x(roll * 0.03);
    }
}