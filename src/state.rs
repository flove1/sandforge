use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    LoadingAssets,
    Menu,
    Setup,
    LevelInitialization,
    Splash,
    Game,
    GameOver,
}

pub fn state_auto_transition(
    app_state: Res<State<GameState>>,
    mut game_state: ResMut<NextState<GameState>>
) {
    match app_state.get() {
        GameState::Setup => {
            game_state.set(GameState::LevelInitialization);
        }
        GameState::LevelInitialization => {
            game_state.set(GameState::Splash);
        }
        _ => {}
    }
}
