use camera::Camera;
use flecs_ecs::{
    core::{
        flecs::{
            self,
            pipeline::{OnStore, OnUpdate, PostUpdate, PreStore, PreUpdate},
        },
        QueryBuilderImpl, SystemAPI, TermBuilderImpl, World, WorldGet,
    },
    macros::{observer, system, Component},
    prelude::*,
};

use glam::{Mat4, Vec2, Vec3};
use gpu::{GpuApi, ShadersInitEvent};
use sdl3_sys::{
    self as sdl3,
    error::SDL_GetError,
    gpu::*,
    iostream::SDL_LoadFile,
    pixels::{SDL_FColor, SDL_PIXELFORMAT_ABGR8888, SDL_PIXELFORMAT_UNKNOWN},
    scancode::*,
    stdinc::{SDL_free, SDL_memcpy, SDL_rand, SDL_strstr},
    surface::{SDL_ConvertSurface, SDL_DestroySurface, SDL_LoadBMP, SDL_Surface},
};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    os::raw::c_int,
    ptr::null_mut,
    time::Instant,
    u8, usize,
};
use window::Window;

mod camera;
mod gpu;
mod window;

const BASE_PATH: &str = env!("CARGO_MANIFEST_DIR");

#[allow(unused_assignments)]
pub fn load_shader(
    gpu_device: *mut SDL_GPUDevice,
    file_name: &str,
    sampler_count: u32,
    uniform_buffer_count: u32,
    storage_buffer_count: u32,
    storage_text_count: u32,
) -> Result<*mut SDL_GPUShader, String> {
    let file_name = CString::new(file_name).unwrap();
    unsafe {
        //let base_path = CString::new(BASE_PATH).unwrap(); /* SDL_GetBasePath(); */
        let mut stage = SDL_GPUShaderStage::default();
        if SDL_strstr(file_name.as_ptr(), CString::new(".vert").unwrap().as_ptr()) != null_mut() {
            stage = SDL_GPU_SHADERSTAGE_VERTEX;
        } else if SDL_strstr(file_name.as_ptr(), CString::new(".frag").unwrap().as_ptr())
            != null_mut()
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
            full_path = CString::new(format!(
                "{}/Shaders/Compiled/SPIRV/{}.spv",
                BASE_PATH,
                file_name.to_str().unwrap()
            ))
            .unwrap()
            .into_raw();
            format = SDL_GPU_SHADERFORMAT_SPIRV;
            entrypoint = CString::new("main").unwrap();
        } else if (backend_formats & SDL_GPU_SHADERFORMAT_MSL) != 0 {
            full_path = CString::new(format!(
                "{}/Shaders/Compiled/MSL/{}.msl",
                BASE_PATH,
                file_name.to_str().unwrap()
            ))
            .unwrap()
            .into_raw();
            format = SDL_GPU_SHADERFORMAT_MSL;
            entrypoint = CString::new("main0").unwrap();
        } else if (backend_formats & SDL_GPU_SHADERFORMAT_DXIL) != 0 {
            full_path = CString::new(format!(
                "{}/Shaders/Compiled/DXIL/{}.dxil",
                BASE_PATH,
                file_name.to_str().unwrap()
            ))
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

    full_path = CString::new(format!("{}/Images/{}", BASE_PATH, file_name))
        .unwrap()
        .into_raw();

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
pub struct TexturePipeline(pub *mut SDL_GPUGraphicsPipeline);

unsafe impl Send for TexturePipeline {}
unsafe impl Sync for TexturePipeline {}

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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PointTexture {
    pub point: Vec3,
    pub uv: Vec2,
}

#[derive(Component)]
pub struct Renderer {
    pub command_buffer: *mut SDL_GPUCommandBuffer,
    pub render_pass: *mut SDL_GPURenderPass,
}

unsafe impl Send for Renderer {}
unsafe impl Sync for Renderer {}

unsafe impl Send for Sprite {}
unsafe impl Sync for Sprite {}

unsafe impl Send for SpritesBuffer {}
unsafe impl Sync for SpritesBuffer {}

#[derive(Component)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

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
                }),
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
                }),
            );

            let texture_transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &(SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: ((*image).w * (*image).h * 4) as u32,
                    ..Default::default()
                }),
            );

            let texture_transfer_ptr =
                SDL_MapGPUTransferBuffer(gpu_device, texture_transfer_buffer, false);
            SDL_memcpy(
                texture_transfer_ptr,
                (*image).pixels,
                ((*image).w * (*image).h * 4) as usize,
            );
            SDL_UnmapGPUTransferBuffer(gpu_device, texture_transfer_buffer);

            let transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &(SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: (100000 * size_of::<Sprite>()) as u32,
                    ..Default::default()
                }),
            );

            let data_buffer = SDL_CreateGPUBuffer(
                gpu_device,
                &(SDL_GPUBufferCreateInfo {
                    usage: SDL_GPU_BUFFERUSAGE_GRAPHICS_STORAGE_READ,
                    size: (100000 * size_of::<Sprite>()) as u32,
                    ..Default::default()
                }),
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
                false,
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
                    }),
                );

                self.data_buffer = SDL_CreateGPUBuffer(
                    gpu_device,
                    &(SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_GRAPHICS_STORAGE_READ,
                        size: (self.size * size_of::<Sprite>()) as u32,
                        ..Default::default()
                    }),
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

fn main() -> Result<(), &'static str> {
    let world = World::new();

    let window_title = "Example window";

    unsafe {
        if !sdl3::init::SDL_SetAppMetadata(
            CString::new(window_title).unwrap().as_ptr(),
            CString::new("1.0").unwrap().as_ptr(),
            CString::new("example window with flecs").unwrap().as_ptr(),
        ) {
            return Err("Failed to set app metadata");
        }

        if !sdl3::init::SDL_Init(sdl3::init::SDL_INIT_VIDEO) {
            return Err("Failed to initialize SDL");
        }
    }

    world.component::<Window>();
    world.component::<GpuApi>();
    world.component::<TexturePipeline>();
    world.component::<Sprite>();
    world.component::<Renderer>();

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

    let window = Window::new("Example window", 800, 600);
    let renderer = GpuApi::new(window.0);
    renderer.init(&world, window.0);
    let sprites_buffer = SpritesBuffer::new("ravioli.bmp", renderer.gpu_device);

    world.set(window);
    world.set(renderer);
    world.set(Renderer {
        command_buffer: null_mut(),
        render_pass: null_mut(),
    });
    world.set(sprites_buffer);
    world.set(Camera::new(0.0, 800.0, 600.0, 0.0, 0.0, -1.0));

    let mut event = sdl3::events::SDL_Event::default();

    system!("sprite_render_pipeline", world, &mut Renderer($), &GpuApi($), &Window($))
        .kind::<PreUpdate>()
        .each_iter(|_it, _, (renderer, gpu_api, window)| unsafe {
            let gpu_device = gpu_api.gpu_device;
            let color = gpu_api.color;
            let window = window.0;

            let cmd_buf = SDL_AcquireGPUCommandBuffer(gpu_device);
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
                    r: color.0,
                    g: color.1,
                    b: color.2,
                    a: 1.0,
                };
                color_target_info.load_op = SDL_GPU_LOADOP_CLEAR;
                color_target_info.store_op = SDL_GPU_STOREOP_STORE;

                let render_pass =
                    SDL_BeginGPURenderPass(cmd_buf, &color_target_info, 1, null_mut());

                renderer.render_pass = render_pass;
            }

            renderer.command_buffer = cmd_buf;
        });

    let sprites_query = world.query::<&Sprite>().set_cached().build();

    system!("draw_sprites", world, &Renderer($), &SpritesBuffer($), &GpuApi($), &mut Camera($), &TexturePipeline($))
        .kind::<OnUpdate>()
        .each_iter(move |_it, _, (renderer, sprite_buffer, gpu_api, camera, pipeline)| unsafe {
            let gpu_device = gpu_api.gpu_device;
            let render_pass = renderer.render_pass;
            let command_buffer = renderer.command_buffer;

            if sprites_query.count() == 0 {
                return;
            }

            let data_ptr= SDL_MapGPUTransferBuffer(
                gpu_device,
                sprite_buffer.transfer_buffer,
                false
            );

            sprites_query.run(|mut it| {
                while it.next() {
                    let s = &it.field::<Sprite>(0).unwrap()[..];
                    SDL_memcpy(data_ptr, s.as_ptr() as *const c_void, s.len() * size_of::<Sprite>());

                    SDL_UnmapGPUTransferBuffer(gpu_device, sprite_buffer.transfer_buffer);

                    let copy_pass = SDL_BeginGPUCopyPass(command_buffer);
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
                    SDL_BindGPUVertexStorageBuffers(render_pass, 0, &sprite_buffer.data_buffer, 1);
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
                        command_buffer,
                        0,
                        &mut camera.0 as *mut _ as *mut c_void,
                        size_of::<Mat4>() as u32
                    );
                    SDL_DrawGPUPrimitives(render_pass, (s.len() * 6) as u32, 1, 0, 0);
                }
            });
        });

    system!("sprite_submit_buffer", world, &mut Renderer($))
        .kind::<PostUpdate>()
        .each_iter(|_it, _, renderer| unsafe {
            if renderer.render_pass != null_mut() {
                SDL_EndGPURenderPass(renderer.render_pass);
                renderer.render_pass = null_mut();
            }
            SDL_SubmitGPUCommandBuffer(renderer.command_buffer);
            renderer.command_buffer = null_mut();
        });

    system!("resize_sprite_buffer", world, &GpuApi($), &mut SpritesBuffer)
        .kind::<PreStore>()
        .each(|(gpu_api, sprites_buffer)| {
            sprites_buffer.resize(gpu_api.gpu_device);
        });

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
        let key_states: &[bool] =
            unsafe { std::slice::from_raw_parts(key_state_ptr, numkeys as usize) };

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
