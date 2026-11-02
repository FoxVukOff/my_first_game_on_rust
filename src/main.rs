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
const SAND_TOP: f32 = 0.16;
const WORLD_X: f32 = 38.0;
const WORLD_Z: f32 = 24.0;
const RIVER_EDGE: f32 = 8.6;

const SKY: Color = Color::srgb(0.55, 0.76, 0.93);
const SUN_POS: Vec3 = Vec3::new(58.0, 20.0, -152.0);

// ---------- РЕЖИМЫ ----------
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
enum RelaxMode {
    #[default]
    Resting,
    Walking,
}

// ---------- КОМПОНЕНТЫ ----------
#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
    bob: f32,
}

#[derive(Component)]
struct Water;

#[derive(Component)]
struct WaterSheen;

#[derive(Component)]
struct Foam {
    lane: f32,
    speed: f32,
    phase: f32,
}

#[derive(Component)]
struct Sway {
    base: Vec3,
    base_rot: Quat,
    phase: f32,
}

#[derive(Component)]
struct Frond {
    base_rot: Quat,
    phase: f32,
}

#[derive(Component)]
struct Mote {
    base: Vec3,
    phase: f32,
    radius: f32,
    speed: f32,
}

#[derive(Component)]
struct CloudPuff;

#[derive(Component)]
struct Bird {
    center: Vec3,
    radius: f32,
    speed: f32,
    phase: f32,
    height: f32,
}

#[derive(Component)]
struct Drink {
    rest: Vec3,
    mouth: Vec3,
    walk: Vec3,
}

#[derive(Component)]
struct Accessory {
    offset: Vec3,
    rot: Quat,
}

// ---------- РЕСУРСЫ ----------
#[derive(Resource)]
struct Obstacles(Vec<(f32, f32, f32)>);

#[derive(Resource, Default)]
struct DrinkPose {
    translation: Vec3,
    rotation: Quat,
}

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
                    title: "River Relax".into(),
                    resolution: WindowResolution::new(1600, 900),
                    resizable: true,
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(ClearColor(SKY))
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.72, 0.82, 0.95),
            brightness: 150.0,
            ..default()
        })
        .init_resource::<RelaxMode>()
        .init_resource::<DrinkPose>()
        .add_systems(Startup, setup)
        .add_systems(Update, camera_control)
        .add_systems(Update, drink_system)
        .add_systems(Update, accessory_system.after(drink_system))
        .add_systems(Update, (sway_system, frond_system, water_system))
        .add_systems(Update, (sheen_system, foam_system, mote_system))
        .add_systems(Update, (cloud_system, bird_system))
        .run();
}

// ---------- SETUP ----------
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("=== River Relax ===");
    println!("E      - встать / лечь в гамак");
    println!("WASD   - ходить (когда встал)");
    println!("Mouse  - осмотреться");
    println!("Shift  - быстрее");
    println!("SPACE  - отпить лимонада");
    println!("===================");

    // ---- КАМЕРА: HDR + ACES + Bloom + туман ----
    let mut bloom = Bloom::default();
    bloom.intensity = 0.32;
    bloom.prefilter.threshold = 0.75;

    commands.spawn((
        Camera3d::default(),
        Hdr,
        Tonemapping::AcesFitted,
        bloom,
        DistanceFog {
            color: SKY,
            falloff: FogFalloff::Linear {
                start: 35.0,
                end: 280.0,
            },
            ..default()
        },
        Transform::from_xyz(-1.62, 1.06, 13.95),
        Player {
            yaw: 0.0,
            pitch: 0.0,
            bob: 0.0,
        },
    ));

    // ---- СОЛНЦЕ ----
    let to_sun = SUN_POS.normalize();
    commands.spawn((
        DirectionalLight {
            illuminance: 32_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.93, 0.80),
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, -to_sun)),
        Visibility::default(),
    ));

    // ---- МАТЕРИАЛЫ ----
    let mat_water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.22, 0.36),
        metallic: 0.35,
        perceptual_roughness: 0.045,
        emissive: LinearRgba::from(Color::srgb(0.0, 0.05, 0.09)) * 1.5,
        ..default()
    });
    let mat_sheen = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.72, 0.82),
        alpha_mode: AlphaMode::Blend,
        metallic: 0.55,
        perceptual_roughness: 0.02,
        emissive: LinearRgba::from(Color::srgb(0.08, 0.2, 0.26)) * 2.0,
        ..default()
    });
    let mat_bed = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.38, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mat_foam = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.99, 1.0),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::from(Color::srgb(0.4, 0.5, 0.55)) * 1.2,
        ..default()
    });
    let mat_sand = materials.add(StandardMaterial {
        base_color: Color::srgb(0.93, 0.85, 0.66),
        perceptual_roughness: 0.98,
        ..default()
    });
    let mat_sand_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.76, 0.56),
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
    let mat_frond_dry = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.48, 0.16),
        perceptual_roughness: 0.8,
        ..default()
    });
    let mat_coco = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.2, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_rock = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.52, 0.5),
        perceptual_roughness: 0.88,
        ..default()
    });
    let mat_shell = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.93, 0.86),
        perceptual_roughness: 0.4,
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
    let mat_lemon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.28),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.85, 0.2)) * 3.2,
        perceptual_roughness: 0.25,
        ..default()
    });
    let mat_ice = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.96, 1.0),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.08,
        metallic: 0.1,
        ..default()
    });
    let mat_straw = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 0.35),
        emissive: LinearRgba::from(Color::srgb(0.3, 0.08, 0.08)),
        perceptual_roughness: 0.4,
        ..default()
    });
    let mat_wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.44, 0.26),
        perceptual_roughness: 0.82,
        ..default()
    });
    let mat_canopy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.72, 0.2),
        perceptual_roughness: 0.7,
        ..default()
    });
    let mat_ball = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 0.55),
        emissive: LinearRgba::from(Color::srgb(0.25, 0.05, 0.1)) * 1.5,
        perceptual_roughness: 0.35,
        ..default()
    });
    let mat_bush = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.44, 0.2),
        perceptual_roughness: 0.85,
        ..default()
    });
    let mat_hill = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.42, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    let mat_hill_far = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.48, 0.58),
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
        base_color: Color::srgb(0.98, 0.98, 0.96),
        perceptual_roughness: 0.8,
        ..default()
    });
    let mat_mote = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.96, 0.8),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.92, 0.6)) * 6.0,
        unlit: true,
        ..default()
    });
    let mat_sun = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.78),
        emissive: LinearRgba::from(Color::srgb(1.0, 0.86, 0.55)) * 26.0,
        ..default()
    });
    let mat_halo = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.7),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::from(Color::srgb(1.0, 0.85, 0.5)) * 2.6,
        ..default()
    });

    // ---- МЕШИ ----
    let m_sphere = meshes.add(Sphere::new(1.0));
    let m_cuboid = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let m_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    // ---- ВОДА ----
    spawn_scaled(
        &mut commands,
        m_cuboid.clone(),
        mat_bed.clone(),
        Vec3::new(0.0, -0.42, 0.0),
        Vec3::new(190.0, 0.1, 21.0),
    );

    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_water.clone()),
        Transform {
            translation: Vec3::new(0.0, -0.05, 0.0),
            scale: Vec3::new(190.0, 0.08, 20.0),
            ..default()
        },
        Water,
    ));

    commands.spawn((
        Mesh3d(m_cuboid.clone()),
        MeshMaterial3d(mat_sheen.clone()),
        Transform {
            translation: Vec3::new(0.0, -0.005, 0.0),
            scale: Vec3::new(189.0, 0.03, 19.6),
            ..default()
        },
        WaterSheen,
    ));

    for (i, lane) in [-7.4, -4.2, -1.0, 2.4, 5.6, 7.8].iter().enumerate() {
        commands.spawn((
            Mesh3d(m_cuboid.clone()),
            MeshMaterial3d(mat_foam.clone()),
            Transform {
                translation: Vec3::new(0.0, 0.012, *lane),
                scale: Vec3::new(9.0, 0.02, 0.35),
                ..default()
            },
            Foam {
                lane: *lane,
                speed: 1.6 + i as f32 * 0.35,
                phase: i as f32 * 17.0,
            },
        ));
    }

    // ---- ПЛЯЖИ ----
    for z in [-18.0, 18.0] {
        spawn_scaled(
            &mut commands,
            m_cuboid.clone(),
            mat_sand.clone(),
            Vec3::new(0.0, 0.08, z),
            Vec3::new(190.0, SAND_TOP, 18.0),
        );
        for (dx, dz, r) in [
            (-26.0, z + 4.0, 5.5),
            (-12.0, z + 5.5, 4.2),
            (14.0, z + 4.5, 6.0),
            (28.0, z + 5.0, 4.8),
            (40.0, z + 3.0, 5.2),
        ] {
            spawn_scaled(
                &mut commands,
                m_sphere.clone(),
                mat_sand_dark.clone(),
                Vec3::new(dx, SAND_TOP - r * 0.42, dz),
                Vec3::new(r * 1.9, r * 0.55, r * 1.3),
            );
        }
        let wz = if z > 0.0 { z - 8.4 } else { z + 8.4 };
        spawn_scaled(
            &mut commands,
            m_cuboid.clone(),
            mat_wet.clone(),
            Vec3::new(0.0, SAND_TOP + 0.005, wz),
            Vec3::new(190.0, 0.02, 2.4),
        );
    }

    // ---- ГОРЫ ----
    for (x, y, z, sx, sy, sz, mat) in [
        (-70.0, -6.0, -70.0, 70.0, 26.0, 40.0, mat_hill.clone()),
        (-10.0, -8.0, -95.0, 90.0, 34.0, 50.0, mat_hill.clone()),
        (60.0, -7.0, -80.0, 75.0, 28.0, 44.0, mat_hill_far.clone()),
        (120.0, -10.0, -110.0, 100.0, 40.0, 60.0, mat_hill_far.clone()),
        (-140.0, -10.0, -100.0, 95.0, 36.0, 55.0, mat_hill_far.clone()),
    ] {
        spawn_scaled(
            &mut commands,
            m_sphere.clone(),
            mat,
            Vec3::new(x, y, z),
            Vec3::new(sx, sy, sz),
        );
    }

    // ---- ДАЛЬНИЙ ЛЕС ----
    let mut rng: u32 = 1234;
    for _ in 0..46 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let x = -85.0 + r1 * 170.0;
        let z = -33.0 - r2 * 12.0;
        let h = 2.4 + r2 * 2.6;
        spawn_scaled(
            &mut commands,
            m_cyl.clone(),
            mat_bark.clone(),
            Vec3::new(x, SAND_TOP + h * 0.5, z),
            Vec3::new(0.28, h, 0.28),
        );
        spawn_scaled(
            &mut commands,
            m_sphere.clone(),
            mat_forest.clone(),
            Vec3::new(x, SAND_TOP + h + 0.6, z),
            Vec3::new(2.0 + r1, 1.7 + r2, 2.0 + r2),
        );
    }

    // ---- ПАЛЬМЫ ----
    let mut obstacles: Vec<(f32, f32, f32)> = vec![];
    for (x, z, lean, height) in [
        (-3.4, 14.2, 0.055, 5.4),
        (3.4, 14.2, -0.055, 5.4),
        (-9.5, 18.5, 0.10, 6.2),
        (10.5, 17.0, -0.11, 6.6),
        (-18.0, 21.0, 0.07, 5.8),
        (19.0, 22.0, -0.08, 6.0),
        (-27.0, 16.0, 0.12, 5.2),
        (-6.0, -19.5, 0.06, 5.6),
        (13.0, -20.5, -0.09, 6.0),
    ] {
        spawn_palm(
            &mut commands,
            &m_cuboid,
            &m_sphere,
            &mat_bark,
            &mat_frond,
            &mat_frond_dry,
            &mat_coco,
            Vec3::new(x, SAND_TOP, z),
            lean,
            height,
        );
        obstacles.push((x, z, 0.55));
    }

    // ---- ГАМАК + ПЕРСОНАЖ ----
    let left = Vec3::new(-3.4, SAND_TOP, 14.2);
    let right = Vec3::new(3.4, SAND_TOP, 14.2);
    spawn_hammock(
        &mut commands,
        &m_cuboid,
        &m_cyl,
        &mat_cloth,
        &mat_cloth2,
        &mat_rope,
        left,
        right,
    );
    spawn_character(
        &mut commands,
        &m_sphere,
        &m_cuboid,
        &m_cyl,
        &mat_skin,
        &mat_shirt,
        &mat_shorts,
        &mat_hat,
    );

    // ---- СТОЛИК + ЗОНТИК ----
    let table = Vec3::new(2.4, SAND_TOP, 16.4);
    spawn_scaled(
        &mut commands,
        m_cyl.clone(),
        mat_wood.clone(),
        table + Vec3::Y * 0.28,
        Vec3::new(0.1, 0.56, 0.1),
    );
    spawn_scaled(
        &mut commands,
        m_cyl.clone(),
        mat_wood.clone(),
        table + Vec3::Y * 0.58,
        Vec3::new(0.42, 0.05, 0.42),
    );
    obstacles.push((table.x, table.z, 0.55));

    let umb = Vec3::new(-2.6, SAND_TOP, 17.6);
    spawn_scaled(
        &mut commands,
        m_cyl.clone(),
        mat_wood.clone(),
        umb + Vec3::new(0.12, 1.05, 0.0),
        Vec3::new(0.035, 2.1, 0.035),
    );
    spawn_scaled(
        &mut commands,
        m_sphere.clone(),
        mat_canopy.clone(),
        umb + Vec3::new(0.24, 1.98, 0.0),
        Vec3::new(2.4, 0.75, 2.4),
    );
    obstacles.push((umb.x, umb.z, 0.4));

    // ---- ЛИМОНАД + АКСЕССУАРЫ ----
    let drink_rest = Vec3::new(-0.62, 0.98, 14.52);
    let drink_mouth = Vec3::new(-1.36, 1.0, 13.9);
    let drink_walk = table + Vec3::Y * 0.72;

    commands.spawn((
        Mesh3d(m_cyl.clone()),
        MeshMaterial3d(mat_lemon.clone()),
        Transform::from_translation(drink_rest),
        Drink {
            rest: drink_rest,
            mouth: drink_mouth,
            walk: drink_walk,
        },
    ));

    for (offset, rot, mesh, mat, scale) in [
        (
            Vec3::new(0.02, 0.09, 0.015),
            Quat::from_rotation_y(0.6),
            m_cuboid.clone(),
            mat_ice.clone(),
            Vec3::new(0.05, 0.05, 0.05),
        ),
        (
            Vec3::new(-0.03, 0.085, -0.02),
            Quat::from_rotation_y(1.9),
            m_cuboid.clone(),
            mat_ice.clone(),
            Vec3::new(0.045, 0.045, 0.045),
        ),
        (
            Vec3::new(0.075, 0.15, 0.0),
            Quat::from_rotation_z(-0.4),
            m_sphere.clone(),
            mat_lemon.clone(),
            Vec3::new(0.09, 0.03, 0.09),
        ),
        (
            Vec3::new(-0.012, 0.24, 0.01),
            Quat::from_rotation_z(0.22),
            m_cyl.clone(),
            mat_straw.clone(),
            Vec3::new(0.014, 0.34, 0.014),
        ),
    ] {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform {
                translation: drink_rest + offset,
                rotation: rot,
                scale,
            },
            Accessory { offset, rot },
        ));
    }

    // ---- ДЕКОР ----
    for (x, z, r, sy) in [
        (-6.5, 12.2, 0.75, 0.5),
        (-5.2, 11.4, 0.45, 0.35),
        (7.8, 13.0, 0.9, 0.55),
        (9.4, 12.1, 0.5, 0.32),
        (-14.5, 13.5, 1.1, 0.6),
        (16.0, 20.0, 0.8, 0.45),
        (-22.0, 19.0, 1.0, 0.5),
        (4.5, -13.0, 0.9, 0.5),
        (-9.0, -12.4, 0.6, 0.38),
    ] {
        spawn_scaled(
            &mut commands,
            m_sphere.clone(),
            mat_rock.clone(),
            Vec3::new(x, SAND_TOP - r * sy * 0.6, z),
            Vec3::new(r, r * sy, r * 0.85),
        );
        obstacles.push((x, z, r * 0.9));
    }

    for _ in 0..22 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let x = -30.0 + r1 * 60.0;
        let z = if r2 > 0.5 {
            11.5 + r1 * 12.0
        } else {
            -11.5 - r1 * 12.0
        };
        spawn_scaled(
            &mut commands,
            m_sphere.clone(),
            mat_shell.clone(),
            Vec3::new(x, SAND_TOP + 0.02, z),
            Vec3::new(0.12 + r2 * 0.08, 0.06, 0.1 + r1 * 0.06),
        );
    }

    for (x, z, ry, len) in [
        (-11.0, 12.8, 0.7, 4.2),
        (12.5, 14.5, -0.4, 3.4),
        (-3.0, -12.6, 1.2, 3.8),
    ] {
        commands.spawn((
            Mesh3d(m_cyl.clone()),
            MeshMaterial3d(mat_wood.clone()),
            Transform {
                translation: Vec3::new(x, SAND_TOP + 0.16, z),
                rotation: Quat::from_rotation_y(ry) * Quat::from_rotation_z(PI / 2.0),
                scale: Vec3::new(0.16, len, 0.16),
            },
        ));
        obstacles.push((x, z, len * 0.3));
    }

    for (x, z, r) in [
        (-16.0, 17.5, 1.3),
        (-13.5, 19.0, 0.9),
        (17.5, 18.5, 1.2),
        (21.0, 16.0, 0.8),
        (-25.0, 20.5, 1.1),
        (8.0, -17.0, 1.0),
    ] {
        spawn_scaled(
            &mut commands,
            m_sphere.clone(),
            mat_bush.clone(),
            Vec3::new(x, SAND_TOP + r * 0.25, z),
            Vec3::new(r * 1.4, r * 0.8, r * 1.3),
        );
        obstacles.push((x, z, r * 0.8));
    }

    spawn_scaled(
        &mut commands,
        m_sphere.clone(),
        mat_ball.clone(),
        Vec3::new(1.2, SAND_TOP + 0.17, 18.6),
        Vec3::new(0.34, 0.34, 0.34),
    );

    // ---- НЕБО ----
    spawn_scaled(
        &mut commands,
        m_sphere.clone(),
        mat_sun.clone(),
        SUN_POS,
        Vec3::new(13.0, 13.0, 13.0),
    );
    spawn_scaled(
        &mut commands,
        m_sphere.clone(),
        mat_halo.clone(),
        SUN_POS * 0.995,
        Vec3::new(34.0, 34.0, 34.0),
    );

    for (_i, (x, y, z, s)) in [
        (-60.0, 30.0, -70.0, 1.0),
        (-15.0, 36.0, -95.0, 1.4),
        (45.0, 28.0, -60.0, 0.9),
        (95.0, 34.0, -100.0, 1.6),
        (-110.0, 32.0, -40.0, 1.2),
        (10.0, 40.0, 40.0, 1.5),
        (-70.0, 38.0, 60.0, 1.1),
        (80.0, 31.0, 75.0, 1.3),
    ]
    .into_iter()
    .enumerate()
    {
        for (dx, dy, dz, r) in [
            (0.0, 0.0, 0.0, 7.0),
            (6.5, -0.8, 1.5, 5.0),
            (-6.0, -1.0, -1.0, 5.5),
            (3.0, 1.6, -2.5, 4.2),
            (-3.5, 1.2, 2.5, 4.6),
            (10.5, -1.4, 0.5, 3.6),
        ] {
            commands.spawn((
                Mesh3d(m_sphere.clone()),
                MeshMaterial3d(mat_cloud.clone()),
                Transform {
                    translation: Vec3::new(x + dx * s, y + dy * s, z + dz * s),
                    scale: Vec3::new(r * s, r * s * 0.45, r * s * 0.8),
                    ..default()
                },
                CloudPuff,
            ));
        }
    }

    // ---- ПТИЦЫ ----
    for (i, (cx, cz, rad, h)) in [
        (0.0, -18.0, 22.0, 14.0),
        (12.0, -6.0, 16.0, 17.0),
        (-16.0, -22.0, 26.0, 12.0),
        (5.0, 8.0, 19.0, 20.0),
        (-8.0, -2.0, 14.0, 16.0),
    ]
    .into_iter()
    .enumerate()
    {
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
                    speed: 0.22 + i as f32 * 0.03,
                    phase: k as f32 * 0.5 + i as f32 * 1.3,
                    height: h,
                },
            ));
        }
    }

    // ---- ПЫЛИНКИ ----
    for _ in 0..70 {
        let r1 = next_rand(&mut rng);
        let r2 = next_rand(&mut rng);
        let r3 = next_rand(&mut rng);
        let base = Vec3::new(-24.0 + r1 * 48.0, 0.6 + r2 * 6.0, -14.0 + r3 * 34.0);
        commands.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat_mote.clone()),
            Transform {
                translation: base,
                scale: Vec3::splat(0.022 + r3 * 0.02),
                ..default()
            },
            Mote {
                base,
                phase: r1 * 12.0,
                radius: 0.4 + r2 * 1.4,
                speed: 0.25 + r3 * 0.5,
            },
        ));
    }

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
        Transform {
            translation,
            scale,
            ..default()
        },
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
    if len <= f32::EPSILON {
        return;
    }
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
    mat_frond_dry: &Handle<StandardMaterial>,
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
        let mat = if i % 4 == 0 { mat_frond_dry } else { mat_frond };
        commands.spawn((
            Mesh3d(m_sphere.clone()),
            MeshMaterial3d(mat.clone()),
            Transform {
                translation: cur + offset,
                rotation: base_rot,
                scale: Vec3::new(0.5, 0.07, len),
            },
            Frond {
                base_rot,
                phase: i as f32 * 0.9 + pos.x,
            },
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
            Sway {
                base: Vec3::new(x, y, z),
                base_rot: Quat::IDENTITY,
                phase: 0.0,
            },
        ));
    }

    for dz in [-0.42, -0.14, 0.14, 0.42] {
        spawn_beam(
            commands,
            m_cuboid,
            mat_rope,
            anchor_l,
            Vec3::new(-2.72, 0.72, z + dz * 0.8),
            0.028,
        );
        spawn_beam(
            commands,
            m_cuboid,
            mat_rope,
            anchor_r,
            Vec3::new(2.72, 0.72, z + dz * 0.8),
            0.028,
        );
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
    let z = 14.2;

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
            Transform {
                translation: pos,
                rotation: rot,
                scale,
            },
            Sway {
                base: pos,
                base_rot: rot,
                phase: 0.0,
            },
        ));
    }
}

// ---------- СИСТЕМЫ ----------
fn ground_height(z: f32) -> f32 {
    let az = z.abs();
    if az <= 9.4 {
        0.0
    } else if az >= 11.8 {
        SAND_TOP
    } else {
        let t = (az - 9.4) / 2.4;
        t * SAND_TOP
    }
}

fn camera_control(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse: MessageReader<MouseMotion>,
    mut mode: ResMut<RelaxMode>,
    mut cursor: Single<&mut CursorOptions>,
    mut cam: Single<(&mut Transform, &mut Player), With<Camera>>,
    obstacles: Res<Obstacles>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let (mut tf, mut player) = cam.into_inner();

    if keys.just_pressed(KeyCode::KeyE) {
        *mode = if *mode == RelaxMode::Resting {
            RelaxMode::Walking
        } else {
            RelaxMode::Resting
        };
        match *mode {
            RelaxMode::Walking => {
                cursor.grab_mode = CursorGrabMode::Locked;
                cursor.visible = false;
                tf.translation = Vec3::new(1.4, ground_height(17.4) + EYE, 17.4);
                player.yaw = PI * 0.02;
                player.pitch = -0.04;
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
                player.yaw = (player.yaw - ev.delta.x * 0.0012).clamp(-0.8, 0.8);
                player.pitch = (player.pitch - ev.delta.y * 0.001).clamp(-0.5, 0.6);
            }
        }
    }

    match *mode {
        RelaxMode::Resting => {
            let a = (t * 1.15).sin();
            let b = (t * 0.78).cos();
            tf.translation = Vec3::new(-1.9, 1.12 + a * 0.018, 13.92 + b * 0.012);
            tf.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, b * 0.014);
        }
        RelaxMode::Walking => {
            let mut input = Vec2::ZERO;
            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                input.y += 1.0;
            }
            if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
                input.y -= 1.0;
            }
            if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                input.x += 1.0;
            }
            if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                input.x -= 1.0;
            }

            let sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            let speed = if sprint { 5.8 } else { 3.1 };

            if input.length_squared() > 0.001 {
                let fwd = Vec3::new(-player.yaw.sin(), 0.0, -player.yaw.cos());
                let right = Vec3::new(player.yaw.cos(), 0.0, -player.yaw.sin());
                let dir = (fwd * input.y + right * input.x).normalize();
                let step = dir * speed * dt;
                let mut next = tf.translation + step;

                next.x = next.x.clamp(-WORLD_X, WORLD_X);
                next.z = next.z.clamp(-WORLD_Z, WORLD_Z);
                if next.z.abs() < RIVER_EDGE {
                    next.z = if next.z < 0.0 { -RIVER_EDGE } else { RIVER_EDGE };
                }
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

fn sway_system(time: Res<Time>, mut q: Query<(&mut Transform, &Sway)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in q.iter_mut() {
        let a = (t * 1.15 + s.phase).sin();
        let b = (t * 0.78 + s.phase * 1.7).cos();
        tf.translation = s.base + Vec3::new(b * 0.01, a * 0.018, a * 0.05);
        tf.rotation = s.base_rot * Quat::from_rotation_x(a * 0.026) * Quat::from_rotation_z(b * 0.012);
    }
}

fn frond_system(time: Res<Time>, mut q: Query<(&mut Transform, &Frond)>) {
    let t = time.elapsed_secs();
    for (mut tf, f) in q.iter_mut() {
        let a = (t * 1.7 + f.phase).sin();
        let b = (t * 1.1 + f.phase * 0.6).cos();
        tf.rotation = f.base_rot * Quat::from_rotation_z(a * 0.055) * Quat::from_rotation_x(b * 0.03);
    }
}

fn water_system(time: Res<Time>, mut q: Query<&mut Transform, With<Water>>) {
    let t = time.elapsed_secs();
    for mut tf in q.iter_mut() {
        tf.translation.y = -0.05 + (t * 0.7).sin() * 0.012;
        tf.scale.z = 20.0 + (t * 0.35).sin() * 0.25;
    }
}

fn sheen_system(time: Res<Time>, mut q: Query<&mut Transform, With<WaterSheen>>) {
    let t = time.elapsed_secs();
    for mut tf in q.iter_mut() {
        let a = (t * 0.95 + 1.3).sin();
        tf.translation.y = -0.005 + a * 0.014;
        tf.scale.y = 0.03 * (1.0 + a * 0.4);
        tf.scale.x = 189.0 + (t * 0.4).cos() * 0.6;
    }
}

fn foam_system(time: Res<Time>, mut q: Query<(&mut Transform, &Foam)>) {
    let t = time.elapsed_secs();
    for (mut tf, f) in q.iter_mut() {
        let span = 150.0;
        let x = (t * f.speed + f.phase).rem_euclid(span) - span * 0.5;
        tf.translation = Vec3::new(
            x,
            0.012 + (t * 1.7 + f.phase).sin() * 0.008,
            f.lane + (t * 0.3 + f.phase).sin() * 0.35,
        );
        tf.scale.x = 9.0 * (0.7 + (t * 1.3 + f.phase).sin() * 0.3);
        tf.scale.z = 0.35 * (1.0 + (t * 0.9 + f.phase).cos() * 0.5);
    }
}

fn mote_system(time: Res<Time>, mut q: Query<(&mut Transform, &Mote)>) {
    let t = time.elapsed_secs();
    for (mut tf, m) in q.iter_mut() {
        let a = t * m.speed + m.phase;
        tf.translation = m.base
            + Vec3::new(
                a.sin() * m.radius,
                (a * 1.7).sin() * m.radius * 0.6,
                (a * 0.8).cos() * m.radius,
            );
    }
}

fn cloud_system(time: Res<Time>, mut q: Query<&mut Transform, With<CloudPuff>>) {
    let t = time.elapsed_secs();
    for mut tf in q.iter_mut() {
        let span = 560.0;
        tf.translation.x = (tf.translation.x + t * 0.4).rem_euclid(span) - span * 0.5;
    }
}

fn bird_system(time: Res<Time>, mut q: Query<(&mut Transform, &Bird)>) {
    let t = time.elapsed_secs();
    for (mut tf, b) in q.iter_mut() {
        let a = t * b.speed + b.phase;
        tf.translation = b.center
            + Vec3::new(
                a.cos() * b.radius,
                b.height + (a * 2.0).sin() * 1.1,
                a.sin() * b.radius * 0.55,
            );
        tf.rotation = Quat::from_rotation_y(-(a + PI * 0.5));
        tf.scale.y = 0.2 * (0.55 + (a * 6.0).sin().abs() * 0.7);
    }
}

fn drink_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<RelaxMode>,
    mut q: Query<(&mut Transform, &Drink)>,
    mut pose: ResMut<DrinkPose>,
) {
    let dt = time.delta_secs().min(0.05);
    let alpha = (dt * 7.5).min(1.0);
    let sipping = *mode == RelaxMode::Resting && keys.pressed(KeyCode::Space);

    let mut latest = (Vec3::ZERO, Quat::IDENTITY);

    for (mut tf, d) in q.iter_mut() {
        let base = if *mode == RelaxMode::Resting { d.rest } else { d.walk };
        let target = if sipping { d.mouth } else { base };
        tf.translation = tf.translation.lerp(target, alpha);

        let span = d.mouth.distance(base).max(0.001);
        let prog = (tf.translation.distance(base) / span).clamp(0.0, 1.0);
        tf.rotation = Quat::from_euler(EulerRot::ZYX, prog * 0.85, prog * 0.2, 0.0);
        tf.scale.y = 1.0 - prog * 0.08;

        latest = (tf.translation, tf.rotation);
    }

    pose.translation = latest.0;
    pose.rotation = latest.1;
}

fn accessory_system(pose: Res<DrinkPose>, mut q: Query<(&mut Transform, &Accessory)>) {
    for (mut tf, a) in q.iter_mut() {
        tf.translation = pose.translation + pose.rotation * a.offset;
        tf.rotation = pose.rotation * a.rot;
    }
}