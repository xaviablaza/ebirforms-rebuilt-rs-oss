mod commands;
mod state;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::app_snapshot,
            commands::list_profiles,
            commands::create_profile,
            commands::update_settings,
            commands::lock_init,
            commands::unlock_check,
            commands::render_tax_form,
            commands::render_1601c,
            commands::package_tax_form,
            commands::package_1601c,
            commands::queue_tax_form_dry_run,
            commands::queue_1601c_dry_run,
            commands::list_jobs,
            commands::run_queue_dry_run,
            commands::list_submissions,
            commands::match_receipt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running eBIRForms Desktop");
}
