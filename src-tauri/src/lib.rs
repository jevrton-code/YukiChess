mod chess;

use chess::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState(Mutex::new(chess::Game::new())))
        .invoke_handler(tauri::generate_handler![
            chess::get_state,
            chess::new_game,
            chess::make_move,
            chess::legal_moves
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
