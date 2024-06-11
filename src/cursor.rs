use bevy::{ prelude::*, window::PrimaryWindow };

use crate::{
    assets::SpriteAssetCollection, camera::TrackingCamera, constants::CHUNK_SIZE, simulation::{ chunk_manager::ChunkManager, materials::PhysicsType }, state::GameState
};

#[derive(Component)]
pub struct GameCursor;

#[derive(Component)]
pub struct MaterialName;

pub fn setup_cursor(mut commands: Commands, sprites: Res<SpriteAssetCollection>) {
    commands.spawn((
        Name::new("Cursor"),
        GameCursor,
        ImageBundle {
            image: sprites.cursor.clone().into(),
            style: Style {
                position_type: PositionType::Absolute,
                ..default()
            },
            z_index: ZIndex::Global(100),
            ..default()
        },
    ));
    
    commands.spawn((
        Name::new("Material Name"),
        MaterialName,
        TextBundle {
            text: Text::from_section("Material Name", TextStyle {
                font_size: 16.0,
                color: Color::Rgba { red: 1.0, green: 1.0, blue: 1.0, alpha: 0.75 },
                ..Default::default()
            }).with_no_wrap(),
            style: Style {
                position_type: PositionType::Absolute,
                ..default()
            },
            z_index: ZIndex::Global(100),
            ..default()
        },
    ));
}

pub fn move_cursor(
    mut cursor_q: Query<&mut Style, With<GameCursor>>,
    mut material_q: Query<(&mut Style, &mut Text), (With<MaterialName>, Without<GameCursor>)>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<TrackingCamera>>,
    chunk_manager: Res<ChunkManager>,
    ui_scale: Res<UiScale>,
    game_state: Res<State<GameState>>
) {
    let (camera, camera_transform) = camera_q.single();
    let Ok(window) = window_q.get_single() else {
        return;
    };

    let Ok(mut style) = cursor_q.get_single_mut() else {
        return;
    };

    let Ok((mut text_style, mut text)) = material_q.get_single_mut() else {
        return;
    };

    if let Some(position) = window.cursor_position() {
        style.left = Val::Px((position.x - window.scale_factor() * 7.5) / ui_scale.0);
        style.top = Val::Px((position.y - window.scale_factor() * 7.5) / ui_scale.0);
        text_style.left = Val::Px((position.x - window.scale_factor() * (7.5 - 32.0)) / ui_scale.0);
        text_style.top = Val::Px((position.y - window.scale_factor() * (7.5 - 32.0)) / ui_scale.0);
    }

    if *game_state != GameState::Game {
        text_style.display = Display::None;
        return;
    }

    match
        window_q
            .get_single()
            .ok()
            .map(|window| window.cursor_position())
            .filter(|position| position.is_some())
            .map(|cursor_position| {
                camera
                    .viewport_to_world(camera_transform, cursor_position.unwrap())
                    .map(|ray| ray.origin.truncate() * CHUNK_SIZE as f32)
                    .unwrap()
                    .as_ivec2()
            })
    {
        Some(result) => {
            let pixel = chunk_manager
                .get(result)
                .ok()
                .filter(|pixel| pixel.physics_type != PhysicsType::Air)
                .map(|pixel| pixel.material.ui_name.clone());

            if let Some(id) = pixel {
                text_style.display = Display::default();
                text.sections[0].value = id;
            } else {
                text_style.display = Display::None;
            }
        }
        None => {}
    }
}
