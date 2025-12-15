use crate::ray::Ray;
use crate::vector::Vec3f;
use crate::{image::Image, log_info, scene::Scene};
use rayon::prelude::*;

#[derive(Clone)]
pub struct Renderer {
    pub parameters: Parameters,
}

impl Renderer {
    pub fn new(parameters: Parameters) -> Self {
        return Self { parameters };
    }

    pub fn render_to_image(&self, scene: &Scene, image: &mut Image) {
        let block_size = (image.width * image.height) / rayon::current_num_threads();
        image.bytes = (0..image.width * image.height)
            .into_par_iter()
            .by_uniform_blocks(block_size)
            .map(|index: usize| {
                let mut rng_state: u32 =
                    987612486u32.wrapping_mul((index as u32).wrapping_add(87636354u32));
                let mut final_color = Vec3f::new(0.0, 0.0, 0.0);
                let x: usize = index % image.width;
                let y: usize = image.height - (index / image.width);
                let screen_x = (((x as f32 / image.width as f32) * 2.0) - 1.0)
                    * (image.width as f32 / image.height as f32);
                let screen_y = ((y as f32 / image.height as f32) * 2.0) - 1.0;

                for _ in 0..self.parameters.samples {
                    let forward = (self.parameters.camera_target - self.parameters.camera_pos).normalized();
                    let right = Vec3f::cross(self.parameters.camera_up, forward).normalized();
                    let up = Vec3f::cross(forward, right);
                    
                    let direction = (forward + right * screen_x + up * screen_y).normalized();
                    
                    let mut ray = Ray::new(
                        self.parameters.camera_pos,
                        Vec3f::new(
                            direction.data[0] + (Vec3f::rand_f32(&mut rng_state) * 2.0 - 1.0) * 0.0005,
                            direction.data[1] + (Vec3f::rand_f32(&mut rng_state) * 2.0 - 1.0) * 0.0005,
                            direction.data[2],
                        )
                        .normalized(),
                    );

                    final_color += Ray::trace(
                        &mut ray,
                        self.parameters.max_ray_depth,
                        &scene,
                        &mut rng_state,
                        self.parameters.debug_mode,
                    );

                    // Only one sample is needed for BVH visualization
                    if self.parameters.debug_mode {
                        break;
                    }
                }

                if !self.parameters.debug_mode {
                    final_color /= self.parameters.samples as f32;
                }
                final_color = Vec3f::linear_to_gamma(final_color);

                return final_color.into();
            })
            .collect::<Vec<[u8; 3]>>()
            .into_flattened();
    }
}

impl Default for Renderer {
    fn default() -> Self {
        log_info!("Using default renderer\n");
        return Self {
            parameters: Parameters::default(),
        };
    }
}

pub struct Parameters {
    pub samples: usize,
    pub max_ray_depth: usize,
    pub debug_mode: bool,
    pub camera_pos: Vec3f,
    pub camera_target: Vec3f,
    pub camera_up: Vec3f,
}

impl Clone for Parameters {
    fn clone(&self) -> Self {
        Self {
            samples: self.samples,
            max_ray_depth: self.max_ray_depth,
            debug_mode: self.debug_mode,
            camera_pos: self.camera_pos,
            camera_target: self.camera_target,
            camera_up: self.camera_up,
        }
    }
}

impl Default for Parameters {
    fn default() -> Self {
        return Self {
            samples: 1,
            max_ray_depth: 6,
            debug_mode: false,
            camera_pos: Vec3f::new(0.0, 0.0, 8.0),
            camera_target: Vec3f::new(0.0, 0.0, 0.0),
            camera_up: Vec3f::new(0.0, 1.0, 0.0),
        };
    }
}
