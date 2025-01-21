use std::{ ffi::{ CStr, CString }, ptr::null_mut };

use flecs_ecs::{ core::{ SystemAPI, TermBuilderImpl, World }, macros::{ system, Component } };
use sdl3_sys::{
    self as sdl3,
    error::SDL_GetError,
    gpu::{
        SDL_AcquireGPUCommandBuffer,
        SDL_BeginGPURenderPass,
        SDL_ClaimWindowForGPUDevice,
        SDL_CreateGPUDevice,
        SDL_EndGPURenderPass,
        SDL_GPUColorTargetInfo,
        SDL_GPUDevice,
        SDL_GPUTexture,
        SDL_SubmitGPUCommandBuffer,
        SDL_WaitAndAcquireGPUSwapchainTexture,
        SDL_GPU_LOADOP_CLEAR,
        SDL_GPU_SHADERFORMAT_SPIRV,
        SDL_GPU_STOREOP_STORE,
    },
    pixels::SDL_FColor,
    video::SDL_Window,
};

#[derive(Debug, Component)]
pub struct Window(pub *mut SDL_Window);

#[derive(Debug, Component)]
pub struct GpuApi {
    pub gpu_device: *mut SDL_GPUDevice,
    pub color: (f32, f32, f32),
}

impl GpuApi {
    pub fn new(window: *mut SDL_Window) -> Self {
        unsafe {
            let gpu_device = SDL_CreateGPUDevice(SDL_GPU_SHADERFORMAT_SPIRV, false, null_mut());

            if gpu_device == null_mut() {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to create GPU device: {:?}", error);
            }

            if !SDL_ClaimWindowForGPUDevice(gpu_device, window) {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to claim window for GPU device: {:?}", error);
            }

            Self {
                gpu_device,
                color: (0.0, 1.0, 0.0),
            }
        }
    }

    pub fn draw(&self, window: *mut SDL_Window) {
        unsafe {
            let cmd_buf = SDL_AcquireGPUCommandBuffer(self.gpu_device);
            if cmd_buf == null_mut() {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to acquire GPU command buffer: {:?}", error);
            }

            let mut swapchain_texture: *mut SDL_GPUTexture = null_mut();
            if
                !SDL_WaitAndAcquireGPUSwapchainTexture(
                    cmd_buf,
                    window,
                    &mut swapchain_texture,
                    null_mut(),
                    null_mut()
                )
            {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to wait and acquire GPU swapchain texture: {:?}", error);
            }

            if swapchain_texture != null_mut() {
                let mut color_target_info = SDL_GPUColorTargetInfo::default();
                color_target_info.texture = swapchain_texture;
                color_target_info.clear_color = SDL_FColor {
                    r: self.color.0,
                    g: self.color.1,
                    b: self.color.2,
                    a: 1.0,
                };
                color_target_info.load_op = SDL_GPU_LOADOP_CLEAR;
                color_target_info.store_op = SDL_GPU_STOREOP_STORE;

                let render_pass = SDL_BeginGPURenderPass(
                    cmd_buf,
                    &color_target_info,
                    1,
                    null_mut()
                );
                SDL_EndGPURenderPass(render_pass);
            }

            SDL_SubmitGPUCommandBuffer(cmd_buf);
        }
    }

    pub fn set_color(&mut self, color: (f32, f32, f32)) {
        self.color = color;
    }
}

impl Window {
    pub fn new(title: &str, width: i32, height: i32) -> Self {
        unsafe {
            let window = sdl3::video::SDL_CreateWindow(
                CString::new(title).unwrap().as_ptr(),
                width,
                height,
                0
            );
            if window == null_mut() {
                panic!("Failed to create window");
            }
            Self(window)
        }
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

    let mut event = sdl3::events::SDL_Event::default();

    system!("draw_screen", world, &GpuApi, &Window)
        .singleton()
        .each_iter(|_it, _, (gpu_api, window)| {
            gpu_api.draw(window.0);
        });

    let start_time = std::time::Instant::now();

    'running: loop {
        while unsafe { sdl3::events::SDL_PollEvent(&mut event) } {
            match sdl3::events::SDL_EventType(unsafe { event.r#type }) {
                sdl3::events::SDL_EventType::QUIT => {
                    break 'running;
                }
                _ => {}
            }
        }

        let elapsed_time = start_time.elapsed().as_secs_f32();

        let red = elapsed_time.sin() * 127.0 + 128.0;
        let green = elapsed_time.cos() * 127.0 + 128.0;
        let blue = elapsed_time.cos() * 127.0 + 128.0;

        world.get::<&mut GpuApi>(|gpu_api| {
            gpu_api.set_color((red / 255.0, green / 255.0, blue / 255.0));
        });

        std::thread::sleep(std::time::Duration::from_millis(10));
        world.progress();
    }

    unsafe {
        sdl3::init::SDL_Quit();
    }

    Ok(())
}
