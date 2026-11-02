mod commands;
mod config;
mod imaging;
mod lockscreen;
mod models;
mod services;
mod state;
mod sysinfo;
mod wallpaper;

use tauri::Manager;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .setup(|app| {
            let state = state::AppState::new(app.handle())
                .map_err(|e| std::io::Error::other(e))?;
            app.manage(state);
            log::info!("Wallpaper Studio {} 已启动", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::search_photos,
            commands::preview_image,
            commands::pick_image,
            commands::pick_folder,
            commands::apply_background,
            commands::refresh_history,
            commands::remove_history,
            commands::save_settings,
            commands::screen_info,
            commands::reveal_path,
            commands::open_url,
            commands::clear_cache,
            commands::reset_wallpaper,
            commands::reset_lockscreen,
            commands::relaunch_admin,
        ])
        .run(tauri::generate_context!())
        .expect("Wallpaper Studio 启动失败");
}
