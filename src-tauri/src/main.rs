#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use tauri::Manager;

#[tauri::command]
async fn request(
    state: tauri::State<'_, japread_sources::Application>,
    op: String,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let application = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || application.dispatch(&op, payload))
        .await
        .map_err(|e| e.to_string())?
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = japread_sources::Application::open(
                &dir.join("japread.sqlite3"),
                &dir.join("sources"),
                "desktop",
            )
            .map_err(std::io::Error::other)?;
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![request])
        .run(tauri::generate_context!())
        .expect("JapRead 启动失败");
}
