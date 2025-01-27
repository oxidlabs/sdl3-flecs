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
            let gpu_device = SDL_CreateGPUDevice(SDL_GPU_SHADERFORMAT_DXIL, false, null_mut());

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

    pub fn draw(&self, window: *mut SDL_Window, world: &World) {
        unsafe {
            let cmd_buf = SDL_AcquireGPUCommandBuffer(self.gpu_device);
            if cmd_buf == null_mut() {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to acquire GPU command buffer: {:?}", error);
            }

            let mut swapchain_texture: *mut SDL_GPUTexture = null_mut();
            if !SDL_WaitAndAcquireGPUSwapchainTexture(
                cmd_buf,
                window,
                &mut swapchain_texture,
                null_mut(),
                null_mut(),
            ) {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!(
                    "Failed to wait and acquire GPU swapchain texture: {:?}",
                    error
                );
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

                let render_pass =
                    SDL_BeginGPURenderPass(cmd_buf, &color_target_info, 1, null_mut());

                let render_event = RenderEvent {
                    render_pass,
                    command_buffer: cmd_buf,
                };

                world.event().entity(flecs::Any).emit(&render_event);

                SDL_EndGPURenderPass(render_pass);
            }

            SDL_SubmitGPUCommandBuffer(cmd_buf);
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
