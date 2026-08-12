mod chess;
mod engine;

use chess::AppState;
use engine::EngineState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState(Mutex::new(chess::Game::new())))
        .manage(EngineState::new())
        .invoke_handler(tauri::generate_handler![
            chess::get_state,
            chess::new_game,
            chess::make_move,
            chess::legal_moves,
            engine::engine_status,
            engine::evaluate_position,
            engine::get_hint,
            engine::engine_move
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
