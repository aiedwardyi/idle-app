pub mod contract;
pub mod engines;
pub mod ipc;
pub mod runner;
pub mod store;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            let store = store::Store::open(app_data.join("idle.db"))?;
            app.manage(ipc::AppState::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::list_tasks,
            ipc::add_task,
            ipc::update_task,
            ipc::delete_task,
            ipc::run_now,
            ipc::stop_run,
            ipc::list_runs,
            ipc::get_meters,
            ipc::get_engines,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn package_name() {
        assert_eq!(env!("CARGO_PKG_NAME"), "idle-app");
    }
}
