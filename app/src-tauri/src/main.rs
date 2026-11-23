// Bureau (essais seulement) : sous Android, Tauri appelle `run` directement.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shiori_app_lib::run()
}
