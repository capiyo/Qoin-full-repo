#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use qoin_vector_motion::{simulate, MotionConfig, ParticleInput, ParticleOutput};

#[tauri::command]
fn simulate_vector_motion(
    particles: Vec<ParticleInput>,
    config: MotionConfig,
) -> Result<Vec<ParticleOutput>, String> {
    simulate(&particles, config).map_err(|error| format!("Vector motion rejected: {error:?}"))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![simulate_vector_motion])
        .run(tauri::generate_context!())
        .expect("failed to start Qoin desktop");
}
