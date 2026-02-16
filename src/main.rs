use bevy::input::ButtonState;
use bevy::input::mouse::MouseButtonInput;
use bevy::prelude::*;
use rand::prelude::*;

const PLAYER_RADIUS: f32 = 10.0;
const ENEMY_RADIUS: f32 = 10.0;
const BULLET_RADIUS: f32 = 5.0;

#[derive(Resource)]
struct Score(u32);

#[derive(Component)]
struct ScoreText;

#[derive(Resource)]
struct EnemySpawnTimer(Timer);

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct Bullet {
    direction: Vec2,
    speed: f32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(EnemySpawnTimer(Timer::from_seconds(
            1.0,
            TimerMode::Repeating,
        )))
        .insert_resource(Score(0))
        .add_systems(Update, update_score_ui)
        .add_systems(Startup, setup)
        .add_systems(Update, move_player)
        .add_systems(Update, (shoot_bullet, bullet_movement_system))
        .add_systems(Update, spawn_enemies)
        .add_systems(Update, move_enemies_toward_player)
        .add_systems(Update, bullet_enemy_collision_system)
        .add_systems(Update, enemy_player_collision_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        Text2d::new("@"),
        TextFont {
            font_size: 20.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_translation(Vec3::ZERO),
        Player,
    ));

    // Score UI (screen space)
    commands.spawn((
        Text::new("Score: 0"),
        TextFont {
            font_size: 30.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        ScoreText,
    ));
}

fn move_player(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player_transform: Single<&mut Transform, With<Player>>,
) {
    let mut direction = Vec2::ZERO;
    if input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    if input.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }

    if direction != Vec2::ZERO {
        let speed = 300.0;
        let delta = direction.normalize() * speed * time.delta_secs();
        player_transform.translation.x += delta.x;
        player_transform.translation.y += delta.y;
    }
}

fn shoot_bullet(
    mut mousebtn_evr: MessageReader<MouseButtonInput>,
    mut commands: Commands,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return; // Player is dead, do nothing
    };

    let window = windows.single().unwrap();
    let (camera, cam_tf) = camera_q.single().unwrap();

    for ev in mousebtn_evr.read() {
        if ev.state == ButtonState::Pressed && ev.button == MouseButton::Left {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_pos) {
                    let dir = (world_pos - player_tf.translation.truncate()).normalize();

                    commands.spawn((
                        Text2d::new("*"),
                        TextFont {
                            font_size: 20.0,
                            font: default(),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        Transform::from_translation(player_tf.translation),
                        Bullet {
                            direction: dir,
                            speed: 600.0,
                        },
                    ));
                }
            }
        }
    }
}

fn bullet_movement_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &Bullet)>,
) {
    for (entity, mut tf, bullet) in q.iter_mut() {
        let delta = bullet.direction * bullet.speed * time.delta_secs();
        tf.translation.x += delta.x;
        tf.translation.y += delta.y;

        // Simple lifetime check
        if tf.translation.length() > 5000.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn bullet_enemy_collision_system(
    mut commands: Commands,
    mut score: ResMut<Score>,

    bullets: Query<(Entity, &Transform), With<Bullet>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    for (bullet_entity, bullet_tf) in &bullets {
        for (enemy_entity, enemy_tf) in &enemies {
            let distance = bullet_tf
                .translation
                .truncate()
                .distance(enemy_tf.translation.truncate());

            if distance < BULLET_RADIUS + ENEMY_RADIUS {
                // Despawn both
                commands.entity(bullet_entity).despawn();
                commands.entity(enemy_entity).despawn();

                score.0 += 1;

                break; // Bullet is gone, stop checking
            }
        }
    }
}

fn update_score_ui(score: Res<Score>, mut query: Query<&mut Text, With<ScoreText>>) {
    if score.is_changed() {
        if let Ok(mut text) = query.single_mut() {
            text.0 = format!("Score: {}", score.0);
        }
    }
}

fn enemy_player_collision_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
) {
    if let Ok((player_entity, player_tf)) = player_q.single() {
        for enemy_tf in &enemies {
            let distance = player_tf
                .translation
                .truncate()
                .distance(enemy_tf.translation.truncate());

            if distance < PLAYER_RADIUS + ENEMY_RADIUS {
                println!("Game Over!");
                commands.entity(player_entity).despawn();
            }
        }
    }
}

fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<EnemySpawnTimer>,
    player_q: Query<&Transform, With<Player>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let player = player_q.single().unwrap();

    let mut rng = thread_rng();
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let distance = rng.gen_range(300.0..500.0);

    let spawn_pos = Vec3::new(
        player.translation.x + angle.cos() * distance,
        player.translation.y + angle.sin() * distance,
        0.0,
    );

    commands.spawn((
        Enemy,
        Transform::from_translation(spawn_pos),
        GlobalTransform::default(),
        Text2d::new("E"),
        TextFont {
            font_size: 20.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn move_enemies_toward_player(
    time: Res<Time>,
    player: Single<&Transform, With<Player>>,
    mut enemies: Query<&mut Transform, (With<Enemy>, Without<Player>)>,
) {
    let player_pos = player.translation;

    for mut transform in &mut enemies {
        let direction = (player_pos - transform.translation).truncate();

        if direction != Vec2::ZERO {
            let speed = 150.0; // enemy speed
            let delta = direction.normalize() * speed * time.delta_secs();

            transform.translation.x += delta.x;
            transform.translation.y += delta.y;
        }
    }
}
