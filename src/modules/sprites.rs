use std::{ ffi::{ c_void, CStr }, ptr::null_mut };

use flecs_ecs::{
    core::{ flecs::{ self, pipeline::{ PreStore, PreUpdate } }, TermBuilderImpl, WorldGet },
    macros::{ observer, system, Component },
    prelude::{ Builder, Module, QueryAPI, QueryBuilderImpl, SystemAPI },
};
use glam::{ Mat4, Vec2, Vec3 };
use sdl3_sys::{
    error::SDL_GetError,
    gpu::*,
    pixels::SDL_FColor,
    stdinc::SDL_memcpy,
    surface::SDL_DestroySurface,
};

use crate::{
    camera::Camera,
    gpu::{ GpuApi, ShadersInitEvent },
    load_image,
    load_shader,
    window::Window,
};

#[derive(Component)]
pub struct SpritesModule;

#[repr(C)]
#[derive(Component, Clone, Copy)]
pub struct Sprite {
    pub position: Vec3,
    pub rotation: f32,
    pub scale: Vec2,
    pub padding: Vec2,
    pub texture: Texture,
    pub color: ColorRgba,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Texture {
    pub u: f32,
    pub v: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Component)]
pub struct SpritesBuffer {
    pub data_buffer: *mut SDL_GPUBuffer,
    pub transfer_buffer: *mut SDL_GPUTransferBuffer,
    pub texture: *mut SDL_GPUTexture,
    pub sampler: *mut SDL_GPUSampler,
    pub count: usize,
    pub size: usize,
}

#[derive(Component)]
pub struct TexturePipeline(pub *mut SDL_GPUGraphicsPipeline);

unsafe impl Send for TexturePipeline {}
unsafe impl Sync for TexturePipeline {}

unsafe impl Send for Sprite {}
unsafe impl Sync for Sprite {}

unsafe impl Send for SpritesBuffer {}
unsafe impl Sync for SpritesBuffer {}

impl SpritesBuffer {
    pub fn new(file_name: &str, gpu_device: *mut SDL_GPUDevice) -> Self {
        let image = load_image(file_name, 4);

        unsafe {
            let texture = SDL_CreateGPUTexture(
                gpu_device,
                &(SDL_GPUTextureCreateInfo {
                    r#type: SDL_GPU_TEXTURETYPE_2D,
                    format: SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM,
                    width: (*image).w as u32,
                    height: (*image).h as u32,
                    layer_count_or_depth: 1,
                    num_levels: 1,
                    usage: SDL_GPU_TEXTUREUSAGE_SAMPLER,
                    ..Default::default()
                })
            );

            let sampler = SDL_CreateGPUSampler(
                gpu_device,
                &(SDL_GPUSamplerCreateInfo {
                    min_filter: SDL_GPU_FILTER_NEAREST,
                    mag_filter: SDL_GPU_FILTER_NEAREST,
                    mipmap_mode: SDL_GPU_SAMPLERMIPMAPMODE_NEAREST,
                    address_mode_u: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                    address_mode_v: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                    address_mode_w: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                    //enable_anisotropy: true,
                    //max_anisotropy: 4.,
                    ..Default::default()
                })
            );

            let texture_transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &(SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: ((*image).w * (*image).h * 4) as u32,
                    ..Default::default()
                })
            );

            let texture_transfer_ptr = SDL_MapGPUTransferBuffer(
                gpu_device,
                texture_transfer_buffer,
                false
            );
            SDL_memcpy(
                texture_transfer_ptr,
                (*image).pixels,
                ((*image).w * (*image).h * 4) as usize
            );
            SDL_UnmapGPUTransferBuffer(gpu_device, texture_transfer_buffer);

            let transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &(SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: (100000 * size_of::<Sprite>()) as u32,
                    ..Default::default()
                })
            );

            let data_buffer = SDL_CreateGPUBuffer(
                gpu_device,
                &(SDL_GPUBufferCreateInfo {
                    usage: SDL_GPU_BUFFERUSAGE_GRAPHICS_STORAGE_READ,
                    size: (100000 * size_of::<Sprite>()) as u32,
                    ..Default::default()
                })
            );

            let command_buffer = SDL_AcquireGPUCommandBuffer(gpu_device);
            let copy_pass = SDL_BeginGPUCopyPass(command_buffer);

            SDL_UploadToGPUTexture(
                copy_pass,
                &(SDL_GPUTextureTransferInfo {
                    transfer_buffer: texture_transfer_buffer,
                    offset: 0,
                    ..Default::default()
                }),
                &(SDL_GPUTextureRegion {
                    texture,
                    w: (*image).w as u32,
                    h: (*image).h as u32,
                    d: 1,
                    ..Default::default()
                }),
                false
            );

            SDL_EndGPUCopyPass(copy_pass);
            SDL_SubmitGPUCommandBuffer(command_buffer);
            SDL_DestroySurface(image);
            SDL_ReleaseGPUTransferBuffer(gpu_device, texture_transfer_buffer);

            Self {
                transfer_buffer,
                data_buffer,
                texture,
                sampler,
                count: 0,
                size: 100000,
            }
        }
    }

    pub fn resize(&mut self, gpu_device: *mut SDL_GPUDevice) {
        if self.count == self.size - 10000 {
            self.size += 50000;
            unsafe {
                self.transfer_buffer = SDL_CreateGPUTransferBuffer(
                    gpu_device,
                    &(SDL_GPUTransferBufferCreateInfo {
                        usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                        size: (self.size * size_of::<Sprite>()) as u32,
                        ..Default::default()
                    })
                );

                self.data_buffer = SDL_CreateGPUBuffer(
                    gpu_device,
                    &(SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_GRAPHICS_STORAGE_READ,
                        size: (self.size * size_of::<Sprite>()) as u32,
                        ..Default::default()
                    })
                );
            }
        }
    }
}

impl Sprite {
    pub fn new(position: Vec3, sprites_buffer: &mut SpritesBuffer) -> Self {
        let sprite = Sprite {
            position,
            rotation: 0.0,
            scale: Vec2::new(32.0, 32.0),
            padding: Vec2::new(0.0, 0.0),
            texture: Texture {
                u: 0.0,
                v: 0.0,
                w: 1.0,
                h: 1.0,
            },
            color: ColorRgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        };

        sprites_buffer.count += 1;

        sprite
    }
}

impl Module for SpritesModule {
    fn module(world: &flecs_ecs::prelude::World) {
        world.component::<Sprite>();
        world.component::<SpritesBuffer>();
        world.component::<TexturePipeline>();

        world.get::<&GpuApi>(|gpu_api| {
            let gpu_device = gpu_api.gpu_device;
            let sprites_buffer = SpritesBuffer::new("ravioli.bmp", gpu_device);
            world.set(sprites_buffer);
        });

        let sprites_query = world.query::<&Sprite>().set_cached().build();
        observer!("init_texture_shader", world, ShadersInitEvent, flecs::Any).each_iter(|it, _, _| {
            let event = &*it.param();
            let world = it.world();
            let gpu_device = event.gpu_device;
            let window = event.window;

            let vertex_shader = load_shader(gpu_device, "texture.vert", 0, 1, 1, 0).unwrap();
            let fragment_shader = load_shader(gpu_device, "texture.frag", 1, 0, 0, 0).unwrap();

            unsafe {
                let pipeline_create_info = SDL_GPUGraphicsPipelineCreateInfo {
                    target_info: SDL_GPUGraphicsPipelineTargetInfo {
                        num_color_targets: 1,
                        color_target_descriptions: &(SDL_GPUColorTargetDescription {
                            format: SDL_GetGPUSwapchainTextureFormat(gpu_device, window),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    primitive_type: SDL_GPU_PRIMITIVETYPE_TRIANGLELIST,
                    vertex_shader,
                    fragment_shader,
                    ..Default::default()
                };

                let pipeline = SDL_CreateGPUGraphicsPipeline(gpu_device, &pipeline_create_info);
                if pipeline == null_mut() {
                    panic!("Failed to create Texture pipeline");
                }

                world.set(TexturePipeline(pipeline));

                SDL_ReleaseGPUShader(gpu_device, vertex_shader);
                SDL_ReleaseGPUShader(gpu_device, fragment_shader);

                println!("Setting Texture Pipeline");
            }
        });

        system!("sprite_render_pipeline", world, &GpuApi($), &Window($), &mut Camera($), &TexturePipeline($), &SpritesBuffer($))
            .kind::<PreUpdate>()
            .each_iter(move |_it, _, (gpu_api, window, camera, pipeline, sprite_buffer)| unsafe {
                let gpu_device = gpu_api.gpu_device;
                let color = gpu_api.color;
                let window = window.0;

                let cmd_buf = SDL_AcquireGPUCommandBuffer(gpu_device);
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
                        r: color.0,
                        g: color.1,
                        b: color.2,
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
                    if sprites_query.count() != 0 {
                        let data_ptr = SDL_MapGPUTransferBuffer(
                            gpu_device,
                            sprite_buffer.transfer_buffer,
                            false
                        );

                        sprites_query.run(|mut it| {
                            while it.next() {
                                let s = &it.field::<Sprite>(0).unwrap()[..];
                                SDL_memcpy(
                                    data_ptr,
                                    s.as_ptr() as *const c_void,
                                    s.len() * size_of::<Sprite>()
                                );

                                SDL_UnmapGPUTransferBuffer(
                                    gpu_device,
                                    sprite_buffer.transfer_buffer
                                );

                                let copy_pass = SDL_BeginGPUCopyPass(cmd_buf);
                                SDL_UploadToGPUBuffer(
                                    copy_pass,
                                    &(SDL_GPUTransferBufferLocation {
                                        transfer_buffer: sprite_buffer.transfer_buffer,
                                        offset: 0,
                                    }),
                                    &(SDL_GPUBufferRegion {
                                        buffer: sprite_buffer.data_buffer,
                                        offset: 0,
                                        size: (s.len() * size_of::<Sprite>()) as u32,
                                    }),
                                    true
                                );
                                SDL_EndGPUCopyPass(copy_pass);

                                SDL_BindGPUGraphicsPipeline(render_pass, pipeline.0);
                                SDL_BindGPUVertexStorageBuffers(
                                    render_pass,
                                    0,
                                    &sprite_buffer.data_buffer,
                                    1
                                );
                                SDL_BindGPUFragmentSamplers(
                                    render_pass,
                                    0,
                                    &(SDL_GPUTextureSamplerBinding {
                                        texture: sprite_buffer.texture,
                                        sampler: sprite_buffer.sampler,
                                    }),
                                    1
                                );
                                SDL_PushGPUVertexUniformData(
                                    cmd_buf,
                                    0,
                                    &mut camera.0 as *mut _ as *mut c_void,
                                    size_of::<Mat4>() as u32
                                );
                                SDL_DrawGPUPrimitives(render_pass, (s.len() * 6) as u32, 1, 0, 0);
                            }
                        });
                    }

                    SDL_EndGPURenderPass(render_pass);
                }

                SDL_SubmitGPUCommandBuffer(cmd_buf);
            });

        system!("resize_sprite_buffer", world, &GpuApi($), &mut SpritesBuffer)
            .kind::<PreStore>()
            .each(|(gpu_api, sprites_buffer)| {
                sprites_buffer.resize(gpu_api.gpu_device);
            });
    }
}
