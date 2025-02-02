use std::{ffi::CStr, ptr::null_mut};

use flecs_ecs::{
    core::{flecs, World},
    macros::Component,
};
use sdl3_sys::{error::SDL_GetError, gpu::*, pixels::SDL_FColor, video::*};

#[derive(Debug, Component)]
pub struct GpuApi {
    pub gpu_device: *mut SDL_GPUDevice,
    pub color: (f32, f32, f32),
}

#[derive(Component)]
pub struct RenderEvent {
    pub command_buffer: *mut SDL_GPUCommandBuffer,
    pub render_pass: *mut SDL_GPURenderPass,
}

#[derive(Component)]
pub struct ShadersInitEvent {
    pub gpu_device: *mut SDL_GPUDevice,
    pub window: *mut SDL_Window,
}

unsafe impl Send for GpuApi {}
unsafe impl Sync for GpuApi {}

unsafe impl Send for RenderEvent {}
unsafe impl Sync for RenderEvent {}

unsafe impl Send for ShadersInitEvent {}
unsafe impl Sync for ShadersInitEvent {}

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

            SDL_SetGPUSwapchainParameters(
                gpu_device,
                window,
                SDL_GPUSwapchainComposition {
                    ..Default::default()
                },
                SDL_GPU_PRESENTMODE_IMMEDIATE,
            );

            Self {
                gpu_device,
                color: (0.2, 0.3, 0.3),
            }
        }
    }

    pub fn set_color(&mut self, color: (f32, f32, f32)) {
        self.color = color;
    }

    pub fn init(&self, world: &World, window: *mut SDL_Window) {
        let event = ShadersInitEvent {
            gpu_device: self.gpu_device,
            window,
        };
        world.event().entity(flecs::Any).emit(&event);
    }
}
