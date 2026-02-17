use bevy::input::ButtonState;
use bevy::input::mouse::MouseButtonInput;
use bevy::prelude::*;
use rand::prelude::*;

const PLAYER_RADIUS: f32 = 10.0;
const ENEMY_RADIUS: f32 = 10.0;
const BULLET_RADIUS: f32 = 5.0;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Playing,
    GameOver,
}

#[derive(Component)]
struct GameOverText;

#[derive(Resource)]
struct Score(u32);

#[derive(Component)]
struct InGameEntity;

#[derive(Component)]
struct ScoreText;

#[derive(Resource)]
struct EnemySpawnTimer(Timer);

#[derive(Component)]
struct Player;

#[derive(Component, Clone, Copy)]
enum EnemyType {
    Basic,
    Fast,
    Tank,
}

#[derive(Component)]
struct Enemy {
    kind: EnemyType,
    health: i32,
}

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
        .insert_state(GameState::Playing)
        .insert_resource(Score(0))
        .add_systems(Update, update_score_ui)
        .add_systems(OnEnter(GameState::GameOver), cleanup_ingame_entities)
        .add_systems(Startup, setup)
        .add_systems(Update, move_player.run_if(in_state(GameState::Playing)))
        .add_systems(Update, shoot_bullet.run_if(in_state(GameState::Playing)))
        .add_systems(Update, bullet_movement_system)
        .add_systems(Update, spawn_enemies.run_if(in_state(GameState::Playing)))
        .add_systems(
            Update,
            move_enemies_toward_player.run_if(in_state(GameState::Playing)),
        )
        .add_systems(OnEnter(GameState::GameOver), spawn_game_over_text)
        .add_systems(Update, restart_on_r.run_if(in_state(GameState::GameOver)))
        .add_systems(Update, bullet_enemy_collision_system)
        .add_systems(Update, enemy_player_collision_system)
        .add_systems(OnEnter(GameState::Playing), setup_new_game)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    // Score UI (screen space)
    commands.spawn((
        Text::new("Score: 0"),
        TextFont {
            font_size: 28.0,
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
    if input.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    if input.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if input.pressed(KeyCode::KeyS) {
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
                        InGameEntity,
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
    mut enemies: Query<(Entity, &Transform, &mut Enemy)>,
) {
    for (bullet_entity, bullet_tf) in &bullets {
        for (enemy_entity, enemy_tf, mut enemy) in &mut enemies {
            let distance = bullet_tf
                .translation
                .truncate()
                .distance(enemy_tf.translation.truncate());

            if distance < BULLET_RADIUS + ENEMY_RADIUS {
                commands.entity(bullet_entity).despawn();

                enemy.health -= 1;

                if enemy.health <= 0 {
                    score.0 += match enemy.kind {
                        EnemyType::Basic => 1,
                        EnemyType::Fast => 2,
                        EnemyType::Tank => 5,
                    };

                    commands.entity(enemy_entity).despawn();
                }

                break;
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
    mut next_state: ResMut<NextState<GameState>>,
    player_q: Query<(Entity, &Transform), With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
) {
    let Ok((_player_entity, player_tf)) = player_q.single() else {
        return;
    };

    for enemy_tf in &enemies {
        let distance = player_tf
            .translation
            .truncate()
            .distance(enemy_tf.translation.truncate());

        if distance < PLAYER_RADIUS + ENEMY_RADIUS {
            println!("Game Over!");
            next_state.set(GameState::GameOver);
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

    let Ok(player) = player_q.single() else {
        return;
    };

    let mut rng = thread_rng();
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let distance = rng.gen_range(300.0..500.0);

    let spawn_pos = Vec3::new(
        player.translation.x + angle.cos() * distance,
        player.translation.y + angle.sin() * distance,
        0.0,
    );

    // Random enemy type
    let enemy_type = match rng.gen_range(0..3) {
        0 => EnemyType::Basic,
        1 => EnemyType::Fast,
        _ => EnemyType::Tank,
    };

    let (symbol, health, color) = match enemy_type {
        EnemyType::Basic => ("E", 1, Color::WHITE),
        EnemyType::Fast => ("e", 1, Color::WHITE),
        EnemyType::Tank => ("EE", 3, Color::WHITE),
    };

    commands.spawn((
        Enemy {
            kind: enemy_type,
            health,
        },
        Transform::from_translation(spawn_pos),
        GlobalTransform::default(),
        Text2d::new(symbol),
        TextFont {
            font_size: 20.0,
            font: default(),
            ..default()
        },
        TextColor(color),
        InGameEntity,
    ));
}

fn move_enemies_toward_player(
    time: Res<Time>,
    player: Single<&Transform, With<Player>>,
    mut enemies: Query<(&mut Transform, &Enemy), Without<Player>>,
) {
    let player_pos = player.translation;

    for (mut transform, enemy) in &mut enemies {
        let direction = (player_pos - transform.translation).truncate();

        if direction != Vec2::ZERO {
            let speed = match enemy.kind {
                EnemyType::Basic => 150.0,
                EnemyType::Fast => 300.0,
                EnemyType::Tank => 75.0,
            };

            let delta = direction.normalize() * speed * time.delta_secs();

            transform.translation.x += delta.x;
            transform.translation.y += delta.y;
        }
    }
}

fn cleanup_ingame_entities(mut commands: Commands, query: Query<Entity, With<InGameEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

fn restart_on_r(input: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<GameState>>) {
    if input.just_pressed(KeyCode::KeyR) {
        next_state.set(GameState::Playing);
    }
}

fn spawn_game_over_text(mut commands: Commands) {
    commands.spawn((
        Text::new("GAME OVER\nPress R to Restart"),
        TextFont {
            font_size: 50.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(40.0),
            left: Val::Percent(25.0),
            ..default()
        },
        GameOverText,
    ));
}

fn setup_new_game(
    mut commands: Commands,
    mut score: ResMut<Score>,
    game_over_text: Query<Entity, With<GameOverText>>,
) {
    // Reset score
    score.0 = 0;

    // Remove game over text
    for entity in &game_over_text {
        commands.entity(entity).despawn();
    }

    // Respawn player
    commands.spawn((
        Player,
        InGameEntity,
        Text2d::new("@"),
        TextFont {
            font_size: 20.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_translation(Vec3::ZERO),
    ));
}
