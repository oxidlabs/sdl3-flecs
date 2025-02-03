use camera::Camera;
use flecs_ecs::{
    core::{
        flecs::{ self, pipeline::{ OnStore, OnUpdate, PostUpdate, PreStore, PreUpdate } },
        QueryBuilderImpl,
        SystemAPI,
        TermBuilderImpl,
        World,
        WorldGet,
    },
    macros::{ observer, system, Component },
    prelude::*,
};

use glam::{ Mat4, Vec2, Vec3 };
use gpu::{ GpuApi, ShadersInitEvent };
use modules::sprites::{Sprite, SpritesBuffer, SpritesModule};
use sdl3_sys::{
    self as sdl3,
    error::SDL_GetError,
    gpu::*,
    iostream::SDL_LoadFile,
    pixels::{ SDL_FColor, SDL_PIXELFORMAT_ABGR8888, SDL_PIXELFORMAT_UNKNOWN },
    scancode::*,
    stdinc::{ SDL_free, SDL_memcpy, SDL_rand, SDL_strstr },
    surface::{ SDL_ConvertSurface, SDL_DestroySurface, SDL_LoadBMP, SDL_Surface },
};
use std::{
    ffi::{ c_char, c_void, CStr, CString },
    os::raw::c_int,
    ptr::null_mut,
    time::Instant,
    u8,
    usize,
};
use window::Window;

mod camera;
mod gpu;
mod window;
mod modules;

const BASE_PATH: &str = env!("CARGO_MANIFEST_DIR");

#[allow(unused_assignments)]
pub fn load_shader(
    gpu_device: *mut SDL_GPUDevice,
    file_name: &str,
    sampler_count: u32,
    uniform_buffer_count: u32,
    storage_buffer_count: u32,
    storage_text_count: u32
) -> Result<*mut SDL_GPUShader, String> {
    let file_name = CString::new(file_name).unwrap();
    unsafe {
        //let base_path = CString::new(BASE_PATH).unwrap(); /* SDL_GetBasePath(); */
        let mut stage = SDL_GPUShaderStage::default();
        if SDL_strstr(file_name.as_ptr(), CString::new(".vert").unwrap().as_ptr()) != null_mut() {
            stage = SDL_GPU_SHADERSTAGE_VERTEX;
        } else if
            SDL_strstr(file_name.as_ptr(), CString::new(".frag").unwrap().as_ptr()) != null_mut()
        {
            stage = SDL_GPU_SHADERSTAGE_FRAGMENT;
        } else {
            return Err("Invalid shader file extension".to_owned());
        }

        let mut full_path: *mut c_char = null_mut();
        let backend_formats = SDL_GetGPUShaderFormats(gpu_device);
        let mut format = SDL_GPU_SHADERFORMAT_INVALID;
        let mut entrypoint = CString::new("").unwrap();

        if (backend_formats & SDL_GPU_SHADERFORMAT_SPIRV) != 0 {
            full_path = CString::new(
                format!("{}/Shaders/Compiled/SPIRV/{}.spv", BASE_PATH, file_name.to_str().unwrap())
            )
                .unwrap()
                .into_raw();
            format = SDL_GPU_SHADERFORMAT_SPIRV;
            entrypoint = CString::new("main").unwrap();
        } else if (backend_formats & SDL_GPU_SHADERFORMAT_MSL) != 0 {
            full_path = CString::new(
                format!("{}/Shaders/Compiled/MSL/{}.msl", BASE_PATH, file_name.to_str().unwrap())
            )
                .unwrap()
                .into_raw();
            format = SDL_GPU_SHADERFORMAT_MSL;
            entrypoint = CString::new("main0").unwrap();
        } else if (backend_formats & SDL_GPU_SHADERFORMAT_DXIL) != 0 {
            full_path = CString::new(
                format!("{}/Shaders/Compiled/DXIL/{}.dxil", BASE_PATH, file_name.to_str().unwrap())
            )
                .unwrap()
                .into_raw();
            format = SDL_GPU_SHADERFORMAT_DXIL;
            entrypoint = CString::new("main").unwrap();
        } else {
            return Err("Unrecognized backend shader format!".to_owned());
        }

        let mut code_size: usize = 0;
        let code = SDL_LoadFile(full_path, &mut code_size) as *const u8;
        if code == null_mut() {
            return Err("Failed to load shader file".to_owned());
        }

        let shader_info = SDL_GPUShaderCreateInfo {
            code_size,
            code,
            entrypoint: entrypoint.as_ptr(),
            format,
            stage,
            num_samplers: sampler_count,
            num_uniform_buffers: uniform_buffer_count,
            num_storage_buffers: storage_buffer_count,
            num_storage_textures: storage_text_count,
            ..Default::default()
        };

        let shader = SDL_CreateGPUShader(gpu_device, &shader_info);
        if shader == null_mut() {
            SDL_free(code as *mut _);
            return Err("Failed to create shader".to_owned());
        }

        SDL_free(code as *mut _);
        return Ok(shader);
    }
}

#[allow(unused_assignments)]
pub fn load_image(file_name: &str, desired_channels: u32) -> *mut SDL_Surface {
    let mut full_path: *mut c_char = null_mut();
    let mut result: *mut SDL_Surface = null_mut();
    let mut pixel_format = SDL_PIXELFORMAT_UNKNOWN;

    full_path = CString::new(format!("{}/Images/{}", BASE_PATH, file_name)).unwrap().into_raw();

    result = unsafe { SDL_LoadBMP(full_path) };
    if result == null_mut() {
        panic!("Failed to load BMP");
    }

    if desired_channels == 4 {
        pixel_format = SDL_PIXELFORMAT_ABGR8888;
    } else {
        unsafe {
            SDL_DestroySurface(result);
            panic!("Unexpected desired_channels");
        }
    }

    unsafe {
        if (*result).format != pixel_format {
            let next = SDL_ConvertSurface(result, pixel_format);
            SDL_DestroySurface(result);
            result = next;
        }
    }

    return result;
}

#[derive(Component)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

fn main() -> Result<(), &'static str> {
    let world = World::new();

    let window_title = "Example window";

    unsafe {
        if
            !sdl3::init::SDL_SetAppMetadata(
                CString::new(window_title).unwrap().as_ptr(),
                CString::new("1.0").unwrap().as_ptr(),
                CString::new("example window with flecs").unwrap().as_ptr()
            )
        {
            return Err("Failed to set app metadata");
        }

        if !sdl3::init::SDL_Init(sdl3::init::SDL_INIT_VIDEO) {
            return Err("Failed to initialize SDL");
        }
    }

    world.component::<Window>();
    world.component::<GpuApi>();


    let window = Window::new("Example window", 800, 600);
    let renderer = GpuApi::new(window.0);
    
    world.set(window);
    world.set(renderer);
    world.set(Camera::new(0.0, 800.0, 600.0, 0.0, 0.0, -1.0));
    
    world.import::<SpritesModule>();

    // init the renderer get the world and the window
    world.get::<&GpuApi>(|renderer| {
        world.get::<&Window>(|window| {
            renderer.init(&world, window.0);
        });
    });

    
    let mut event = sdl3::events::SDL_Event::default();

    let mut count = 0;
    'running: loop {
        while (unsafe { sdl3::events::SDL_PollEvent(&mut event) }) {
            match sdl3::events::SDL_EventType(unsafe { event.r#type }) {
                sdl3::events::SDL_EventType::QUIT => {
                    break 'running;
                }
                _ => {}
            }
        }

        unsafe {
            sdl3::events::SDL_PumpEvents();
        }

        let mut numkeys: c_int = 0;
        let key_state_ptr = unsafe { sdl3::keyboard::SDL_GetKeyboardState(&mut numkeys) };

        // Convert the raw pointer into a slice.
        let key_states: &[bool] = unsafe {
            std::slice::from_raw_parts(key_state_ptr, numkeys as usize)
        };

        // For example, if you're using a world to store a Camera:
        if key_states[SDL_SCANCODE_W.0 as usize] {
            world.get::<&mut Camera>(|camera| {
                camera.translate(glam::Vec3::Y);
            });
        }

        if key_states[SDL_SCANCODE_A.0 as usize] {
            world.get::<&mut Camera>(|camera| {
                camera.translate(glam::Vec3::X);
            });
        }

        if key_states[SDL_SCANCODE_S.0 as usize] {
            world.get::<&mut Camera>(|camera| {
                camera.translate(glam::Vec3::NEG_Y);
            });
        }

        if key_states[SDL_SCANCODE_D.0 as usize] {
            world.get::<&mut Camera>(|camera| {
                camera.translate(glam::Vec3::NEG_X);
            });
        }

        if key_states[SDL_SCANCODE_P.0 as usize] {
            // For example, spawn sprites
            count += 100;
            world.get::<&mut SpritesBuffer>(|sprites_buffer| {
                for _ in 0..100 {
                    spawn_sprite(&world, sprites_buffer);
                }
            });
            println!("{}", count);
        }

        world.progress();
    }

    unsafe {
        sdl3::init::SDL_Quit();
    }

    Ok(())
}

fn spawn_sprite(world: &World, sprite_buffer: &mut SpritesBuffer) {
    unsafe {
        let x = SDL_rand(800) as f32;
        let y = SDL_rand(600) as f32;
        world
            .entity()
            .set(Uuid::new())
            .set(Sprite::new(Vec3::new(x, y, 0.0), sprite_buffer));
    }
}
