//! Enveloppe Tauri (lot 1.1c) : une WebView sur `https://shiori.localhost/`, dont chaque requête
//! passe par le relais (`shiori-relay`), sur un fil à part pour ne jamais bloquer la WebView.

use std::sync::Arc;

use shiori_relay::Relay;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Arc::new(Relay::new(dir)));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("shiori", |ctx, request, responder| {
            let relay = ctx.app_handle().state::<Arc<Relay>>().inner().clone();
            std::thread::spawn(move || responder.respond(relay.handle(request)));
        })
        .run(tauri::generate_context!())
        .expect("lancement de l'application");
}
