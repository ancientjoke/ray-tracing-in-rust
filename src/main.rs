use crate::vector::Vec3f;

mod app;
mod bvh;
mod image;
mod loader;
mod log;
mod ray;
mod renderer;
mod scene;
mod texture;
mod vector;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const SAMPLE_COUNT: usize = 50;
const MAX_BOUNCES: usize = 3;
const DEBUG_BVH: bool = false;
const OBJ_PATH: &str = "C:/Users/marce/Downloads/rust_ray_tracing-main/res/170320.obj";

fn main() {
    log_info!("System logical cores: {}\n", rayon::current_num_threads());

    log_info!("Parameters");
    log_info!("- Width:        {}", WIDTH);
    log_info!("- Height:       {}", HEIGHT);
    log_info!("- Sample count: {}", SAMPLE_COUNT);
    log_info!("- Max bounces:  {}", MAX_BOUNCES);
    log_info!("- BVH debug:    {}", DEBUG_BVH);
    log_info!("- Input file:   {}", OBJ_PATH);

    log_info!("\nStarting application renderer...");
    log_info!("Controls:");
    log_info!("- Arrow Keys / WASD: Rotate camera");
    log_info!("- Q/E: Zoom in/out");
    log_info!("- Space: Toggle auto-rotation");
    log_info!("- P: Save current frame to output.ppm");
    log_info!("- ESC: Exit");
    log_info!("- Use UI sliders for precise control\n");

    let app = app::App::new(
        WIDTH,
        HEIGHT,
        SAMPLE_COUNT,
        MAX_BOUNCES,
        DEBUG_BVH,
        OBJ_PATH.to_string(),
    );

    app.run();
}
