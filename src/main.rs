use bevy::camera::{ClearColor, Hdr};
use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::f32::consts::PI;

#[derive(Component)]
struct Water;

#[derive(Component)]
struct Hammock;

#[derive(Component)]
struct Drink {
    original: Vec3,
    mouth: Vec3,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "River Relax".into(),
                    resolution: WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
        )
        .insert_resource(ClearColor(Color::srgb(0.49, 0.74, 0.93)))
        .add_systems(Startup, setup)
        .add_systems(Update, (animate_scene, drink_system))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("Hold SPACE to sip lemonade");

    // Камера
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Transform::from_xyz(-7.2, 2.1, 23.0)
            .looking_at(Vec3::new(0.0, 0.9, 14.0), Vec3::Y),
    ));

    // Солнце как directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 16_000.0,
            shadow_maps_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -PI / 3.1,
            PI / 4.2,
            0.0,
        )),
        Visibility::default(),
    ));

    // Видимое солнце
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.70),
        emissive: Color::srgb(1.0, 0.84, 0.38).into(),
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(4.0))),
        MeshMaterial3d(sun_material),
        Transform::from_xyz(55.0, 38.0, -55.0),
    ));

    // Река
    let water_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.26, 0.43),
        metallic: 0.18,
        perceptual_roughness: 0.10,
        emissive: Color::srgb(0.0, 0.02, 0.04).into(),
        ..default()
    });

    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(190.0, 0.06, 20.0))),
            MeshMaterial3d(water_material),
            Transform::from_xyz(0.0, -0.04, 0.0),
        ))
        .insert(Water);

    // Пляжи
    let sand_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.91, 0.82, 0.62),
        perceptual_roughness: 0.95,
        ..default()
    });

    for z in [-18.0, 18.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(190.0, 0.16, 16.0))),
            MeshMaterial3d(sand_material.clone()),
            Transform::from_xyz(0.0, 0.08, z),
        ));
    }

    // Дальние холмы
    let hill_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.45, 0.25),
        perceptual_roughness: 0.92,
        ..default()
    });

    for (x, y, z, sx, sy, sz) in [
        (-45.0, 4.0, -45.0, 44.0, 16.0, 36.0),
        (25.0, 5.0, -55.0, 60.0, 20.0, 44.0),
        (70.0, 3.0, -35.0, 40.0, 12.0, 32.0),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(sx, sy, sz))),
            MeshMaterial3d(hill_material.clone()),
            Transform::from_xyz(x, y, z),
        ));
    }

    // Деревья для гамака
    let left_tree = Vec3::new(-3.4, 0.0, 14.2);
    let right_tree = Vec3::new(3.4, 0.0, 14.2);

    spawn_tree(&mut commands, &mut meshes, &mut materials, left_tree);
    spawn_tree(&mut commands, &mut meshes, &mut materials, right_tree);

    // Гамак
    let hammock_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.28, 0.22),
        perceptual_roughness: 0.65,
        ..default()
    });

    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(5.9, 0.14, 0.9))),
            MeshMaterial3d(hammock_material),
            Transform::from_xyz(0.0, 0.62, 14.2),
        ))
        .insert(Hammock);

    // Верёвки гамака
    let rope_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.13, 0.10),
        perceptual_roughness: 0.85,
        ..default()
    });

    spawn_beam(
        &mut commands,
        &mut meshes,
        rope_material.clone(),
        left_tree + Vec3::Y * 2.45,
        Vec3::new(-2.75, 0.62, 14.2),
    );

    spawn_beam(
        &mut commands,
        &mut meshes,
        rope_material,
        right_tree + Vec3::Y * 2.45,
        Vec3::new(2.75, 0.62, 14.2),
    );

    // Персонаж
    let skin_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.76, 0.63),
        perceptual_roughness: 0.70,
        ..default()
    });

    let shirt_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.45, 0.78),
        perceptual_roughness: 0.75,
        ..default()
    });

    let shorts_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.24, 0.28),
        perceptual_roughness: 0.80,
        ..default()
    });

    // Голова
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.17))),
        MeshMaterial3d(skin_material.clone()),
        Transform::from_xyz(-1.75, 0.88, 14.2),
    ));

    // Туловище
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.1, 0.26, 0.36))),
        MeshMaterial3d(shirt_material),
        Transform::from_xyz(-0.85, 0.76, 14.2),
    ));

    // Ноги
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.1, 0.20, 0.28))),
        MeshMaterial3d(shorts_material),
        Transform::from_xyz(0.45, 0.72, 14.2),
    ));

    // Рука
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.56, 0.11, 0.11))),
        MeshMaterial3d(skin_material),
        Transform::from_xyz(-0.75, 0.82, 14.45)
            .with_rotation(Quat::from_rotation_y(0.35)),
    ));

    // Лимонад
    let drink_original = Vec3::new(2.0, 0.30, 14.70);
    let drink_mouth = Vec3::new(-1.42, 0.98, 14.25);

    let drink_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.25),
        emissive: Color::srgb(0.30, 0.25, 0.05).into(),
        perceptual_roughness: 0.15,
        ..default()
    });

    commands
        .spawn((
            Mesh3d(meshes.add(Cylinder::new(0.09, 0.28))),
            MeshMaterial3d(drink_material),
            Transform::from_translation(drink_original),
        ))
        .insert(Drink {
            original: drink_original,
            mouth: drink_mouth,
        });
}

fn spawn_tree(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
) {
    let trunk_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.22, 0.13),
        perceptual_roughness: 0.88,
        ..default()
    });

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.36, 4.2, 0.36))),
        MeshMaterial3d(trunk_material),
        Transform::from_xyz(pos.x, pos.y + 2.1, pos.z),
    ));

    let leaf_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.52, 0.22),
        perceptual_roughness: 0.78,
        ..default()
    });

    for (dx, dy, dz, r) in [
        (0.0, 4.1, 0.0, 1.25),
        (-0.65, 3.65, 0.35, 0.95),
        (0.70, 3.55, -0.30, 0.90),
        (0.10, 4.55, -0.55, 0.80),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(r))),
            MeshMaterial3d(leaf_material.clone()),
            Transform::from_xyz(pos.x + dx, pos.y + dy, pos.z + dz),
        ));
    }
}

fn spawn_beam(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<StandardMaterial>,
    start: Vec3,
    end: Vec3,
) {
    let dir = end - start;
    let len = dir.length();

    if len <= f32::EPSILON {
        return;
    }

    let mid = start + dir * 0.5;
    let rot = Quat::from_rotation_arc(Vec3::Y, dir.normalize());

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.07, len, 0.07))),
        MeshMaterial3d(material),
        Transform {
            translation: mid,
            rotation: rot,
            ..default()
        },
    ));
}

fn animate_scene(
    time: Res<Time>,
    mut water: Query<&mut Transform, (With<Water>, Without<Hammock>)>,
    mut hammock: Query<&mut Transform, (With<Hammock>, Without<Water>)>,
) {
    let t = time.elapsed_secs();

    let water_sway = (t * 0.65).sin();
    let hammock_sway = (t * 0.85).sin();

    for mut transform in water.iter_mut() {
        transform.translation.y = -0.04 + 0.003 * water_sway;
    }

    for mut transform in hammock.iter_mut() {
        transform.rotation = Quat::from_rotation_z(hammock_sway * 0.022)
            * Quat::from_rotation_x(hammock_sway * 0.012);
    }
}

fn drink_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut drinks: Query<(&mut Transform, &Drink)>,
) {
    let dt = time.delta_secs();
    let alpha = (dt * 9.0).min(1.0);
    let sipping = keys.pressed(KeyCode::Space);

    for (mut transform, drink) in drinks.iter_mut() {
        let target = if sipping { drink.mouth } else { drink.original };
        transform.translation = transform.translation.lerp(target, alpha);
    }
}