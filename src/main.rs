use flecs_ecs::{
    core::{
        flecs, EntityView, EntityViewGet, QueryBuilderImpl, SystemAPI, TermBuilderImpl, World,
        WorldGet,
    },
    macros::{observer, system, Component},
    prelude::*,
};

use glam::{Mat4, Vec2, Vec3};
use gpu::{GpuApi, RenderEvent, ShadersInitEvent};
use sdl3_sys::{
    self as sdl3,
    gpu::*,
    iostream::SDL_LoadFile,
    pixels::{SDL_PIXELFORMAT_ABGR8888, SDL_PIXELFORMAT_UNKNOWN},
    stdinc::{SDL_free, SDL_memcpy, SDL_strstr},
    surface::{SDL_ConvertSurface, SDL_DestroySurface, SDL_LoadBMP, SDL_Surface},
};
use std::{
    ffi::{c_char, c_void, CString},
    ptr::null_mut,
};
use window::Window;

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
pub struct Pipeline(pub *mut SDL_GPUGraphicsPipeline);

#[derive(Component)]
pub struct TexturePipeline(pub *mut SDL_GPUGraphicsPipeline);

unsafe impl Send for Pipeline {}
unsafe impl Sync for Pipeline {}

unsafe impl Send for TexturePipeline {}
unsafe impl Sync for TexturePipeline {}

#[derive(Component)]
pub struct Triangle {
    pub points: [Vec3; 3],
    pub vertex_buffer: *mut SDL_GPUBuffer,
    pub index_buffer: *mut SDL_GPUBuffer,
}

#[derive(Component)]
pub struct TriangleWithTexture {
    pub points: [PointTexture; 3],
    pub vertex_buffer: *mut SDL_GPUBuffer,
    pub index_buffer: *mut SDL_GPUBuffer,
    pub texture: *mut SDL_GPUTexture,
    pub sampler: *mut SDL_GPUSampler,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PointTexture {
    pub point: Vec3,
    pub uv: Vec2,
}

unsafe impl Send for Triangle {}
unsafe impl Sync for Triangle {}

unsafe impl Send for TriangleWithTexture {}
unsafe impl Sync for TriangleWithTexture {}

#[derive(Component)]
pub struct Uuid(pub uuid::Uuid);

impl Uuid {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Triangle {
    fn new(gpu_device: *mut SDL_GPUDevice, x: Vec3, y: Vec3, z: Vec3) -> Self {
        unsafe {
            let triangle = Triangle {
                points: [x, y, z],
                vertex_buffer: SDL_CreateGPUBuffer(
                    gpu_device,
                    &SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_VERTEX,
                        size: (size_of::<Vec3>() * 3) as u32,
                        ..Default::default()
                    },
                ),
                index_buffer: SDL_CreateGPUBuffer(
                    gpu_device,
                    &SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_INDEX,
                        size: (size_of::<u16>() * 3) as u32,
                        ..Default::default()
                    },
                ),
            };

            let transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: ((size_of::<Vec3>() * 3) + (size_of::<u16>() * 3)) as u32,
                    ..Default::default()
                },
            );

            let transfer_data: *mut Vec3 =
                SDL_MapGPUTransferBuffer(gpu_device, transfer_buffer, false) as *mut _;

            *transfer_data.add(0) = triangle.points[0];
            *transfer_data.add(1) = triangle.points[1];
            *transfer_data.add(2) = triangle.points[2];

            let index_data = transfer_data.add(3) as *mut u16;

            *index_data.add(0) = 0;
            *index_data.add(1) = 1;
            *index_data.add(2) = 2;

            SDL_UnmapGPUTransferBuffer(gpu_device, transfer_buffer);

            let command_buffer = SDL_AcquireGPUCommandBuffer(gpu_device);
            let copy_pass = SDL_BeginGPUCopyPass(command_buffer);

            SDL_UploadToGPUBuffer(
                copy_pass,
                &SDL_GPUTransferBufferLocation {
                    transfer_buffer,
                    offset: 0,
                },
                &SDL_GPUBufferRegion {
                    buffer: triangle.vertex_buffer,
                    offset: 0,
                    size: (size_of::<Vec3>() * 3) as u32,
                },
                false,
            );

            SDL_UploadToGPUBuffer(
                copy_pass,
                &SDL_GPUTransferBufferLocation {
                    transfer_buffer,
                    offset: (size_of::<Vec3>() * 3) as u32,
                },
                &SDL_GPUBufferRegion {
                    buffer: triangle.index_buffer,
                    offset: 0,
                    size: (size_of::<u16>() * 3) as u32,
                },
                false,
            );

            SDL_EndGPUCopyPass(copy_pass);
            SDL_SubmitGPUCommandBuffer(command_buffer);
            SDL_ReleaseGPUTransferBuffer(gpu_device, transfer_buffer);

            triangle
        }
    }

    fn rotate(&mut self, angle_radians: f32) {
        // Calculate centroid (average position)
        let center = (self.points[0] + self.points[1] + self.points[2]) / 3.0;

        // Create rotation matrix for Z-axis rotation (assuming we're rotating in XY plane)
        let cos_theta = angle_radians.cos();
        let sin_theta = angle_radians.sin();

        // Rotate each point around the centroid
        let mut rotated_points = self.points;
        for point in &mut rotated_points {
            // Translate point to origin-centered coordinates
            let translated = *point - center;

            // Apply rotation (Z-axis)
            let x_new = translated.x * cos_theta - translated.y * sin_theta;
            let y_new = translated.x * sin_theta + translated.y * cos_theta;

            // Translate back and update point
            *point = Vec3::new(x_new, y_new, translated.z) + center;
        }

        self.points = rotated_points;
    }
}

impl TriangleWithTexture {
    pub fn new(gpu_device: *mut SDL_GPUDevice, points: [Vec3; 3], file_name: &str) -> Self {
        let image = load_image(file_name, 4);
        unsafe {
            let triangle = TriangleWithTexture {
                points: [
                    PointTexture {
                        point: points[0],
                        uv: Vec2::new(0.0, 0.0),
                    },
                    PointTexture {
                        point: points[1],
                        uv: Vec2::new(1.0, 0.0),
                    },
                    PointTexture {
                        point: points[2],
                        uv: Vec2::new(0.5, 1.0),
                    },
                ],
                vertex_buffer: SDL_CreateGPUBuffer(
                    gpu_device,
                    &SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_VERTEX,
                        size: (size_of::<PointTexture>() * 3) as u32,
                        ..Default::default()
                    },
                ),
                index_buffer: SDL_CreateGPUBuffer(
                    gpu_device,
                    &SDL_GPUBufferCreateInfo {
                        usage: SDL_GPU_BUFFERUSAGE_INDEX,
                        size: (size_of::<u16>() * 3) as u32,
                        ..Default::default()
                    },
                ),
                texture: SDL_CreateGPUTexture(
                    gpu_device,
                    &SDL_GPUTextureCreateInfo {
                        r#type: SDL_GPU_TEXTURETYPE_2D,
                        format: SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM,
                        width: (*image).w as u32,
                        height: (*image).h as u32,
                        layer_count_or_depth: 1,
                        num_levels: 1,
                        usage: SDL_GPU_TEXTUREUSAGE_SAMPLER,
                        ..Default::default()
                    },
                ),
                sampler: SDL_CreateGPUSampler(
                    gpu_device,
                    &SDL_GPUSamplerCreateInfo {
                        min_filter: SDL_GPU_FILTER_NEAREST,
                        mag_filter: SDL_GPU_FILTER_NEAREST,
                        mipmap_mode: SDL_GPU_SAMPLERMIPMAPMODE_NEAREST,
                        address_mode_u: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                        address_mode_v: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                        address_mode_w: SDL_GPU_SAMPLERADDRESSMODE_REPEAT,
                        //enable_anisotropy: true,
                        //max_anisotropy: 4.,
                        ..Default::default()
                    },
                ),
            };

            let transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: ((size_of::<PointTexture>() * 3) + (size_of::<u16>() * 3)) as u32,
                    ..Default::default()
                },
            );

            let transfer_data: *mut PointTexture =
                SDL_MapGPUTransferBuffer(gpu_device, transfer_buffer, false) as *mut _;

            *transfer_data.add(0) = triangle.points[0];
            *transfer_data.add(1) = triangle.points[1];
            *transfer_data.add(2) = triangle.points[2];

            let index_data = transfer_data.add(3) as *mut u16;

            *index_data.add(0) = 0;
            *index_data.add(1) = 1;
            *index_data.add(2) = 2;

            SDL_UnmapGPUTransferBuffer(gpu_device, transfer_buffer);

            let texture_transfer_buffer = SDL_CreateGPUTransferBuffer(
                gpu_device,
                &SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: ((*image).w * (*image).h * 4) as u32,
                    ..Default::default()
                },
            );

            let texture_transfer_ptr =
                SDL_MapGPUTransferBuffer(gpu_device, texture_transfer_buffer, false);
            SDL_memcpy(
                texture_transfer_ptr,
                (*image).pixels,
                ((*image).w * (*image).h * 4) as usize,
            );
            SDL_UnmapGPUTransferBuffer(gpu_device, texture_transfer_buffer);

            let command_buffer = SDL_AcquireGPUCommandBuffer(gpu_device);
            let copy_pass = SDL_BeginGPUCopyPass(command_buffer);

            SDL_UploadToGPUBuffer(
                copy_pass,
                &SDL_GPUTransferBufferLocation {
                    transfer_buffer,
                    offset: 0,
                },
                &SDL_GPUBufferRegion {
                    buffer: triangle.vertex_buffer,
                    offset: 0,
                    size: (size_of::<PointTexture>() * 3) as u32,
                },
                false,
            );

            SDL_UploadToGPUBuffer(
                copy_pass,
                &SDL_GPUTransferBufferLocation {
                    transfer_buffer,
                    offset: (size_of::<PointTexture>() * 3) as u32,
                },
                &SDL_GPUBufferRegion {
                    buffer: triangle.index_buffer,
                    offset: 0,
                    size: (size_of::<u16>() * 3) as u32,
                },
                false,
            );

            SDL_UploadToGPUTexture(
                copy_pass,
                &SDL_GPUTextureTransferInfo {
                    transfer_buffer: texture_transfer_buffer,
                    offset: 0,
                    ..Default::default()
                },
                &SDL_GPUTextureRegion {
                    texture: triangle.texture,
                    w: (*image).w as u32,
                    h: (*image).h as u32,
                    d: 1,
                    ..Default::default()
                },
                false,
            );

            SDL_EndGPUCopyPass(copy_pass);
            SDL_SubmitGPUCommandBuffer(command_buffer);
            SDL_DestroySurface(image);
            SDL_ReleaseGPUTransferBuffer(gpu_device, transfer_buffer);
            SDL_ReleaseGPUTransferBuffer(gpu_device, texture_transfer_buffer);

            triangle
        }
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
    world.component::<Pipeline>();
    world.component::<TexturePipeline>();
    world.component::<Triangle>();
    world.component::<TriangleWithTexture>();

    // Init Shaders and Graphics Pipeline
    observer!("init_vertex_shader", world, ShadersInitEvent, flecs::Any).each_iter(|it, _, _| {
        let event = &*it.param();
        let world = it.world();
        let gpu_device = event.gpu_device;
        let window = event.window;
        unsafe {
            let vertex_shader = load_shader(gpu_device, "example.vert", 0, 0, 0, 0).unwrap();
            let fragment_shader = load_shader(gpu_device, "example.frag", 0, 0, 0, 0).unwrap();

            let pipeline_create_info = SDL_GPUGraphicsPipelineCreateInfo {
                target_info: SDL_GPUGraphicsPipelineTargetInfo {
                    num_color_targets: 1,
                    color_target_descriptions: &(SDL_GPUColorTargetDescription {
                        format: SDL_GetGPUSwapchainTextureFormat(gpu_device, window),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                vertex_input_state: SDL_GPUVertexInputState {
                    num_vertex_buffers: 1,
                    vertex_buffer_descriptions: &(SDL_GPUVertexBufferDescription {
                        slot: 0,
                        input_rate: SDL_GPU_VERTEXINPUTRATE_VERTEX,
                        instance_step_rate: 0,
                        pitch: size_of::<Vec3>() as u32,
                    }),
                    num_vertex_attributes: 1,
                    vertex_attributes: [SDL_GPUVertexAttribute {
                        buffer_slot: 0,
                        format: SDL_GPU_VERTEXELEMENTFORMAT_FLOAT3,
                        location: 0,
                        offset: 0,
                    }]
                    .as_ptr(),
                },
                primitive_type: SDL_GPU_PRIMITIVETYPE_TRIANGLELIST,
                vertex_shader,
                fragment_shader,
                ..Default::default()
            };

            let pipeline = SDL_CreateGPUGraphicsPipeline(gpu_device, &pipeline_create_info);
            if pipeline == null_mut() {
                panic!("Failed to create graphics pipeline");
            }

            world.set(Pipeline(pipeline));

            SDL_ReleaseGPUShader(gpu_device, vertex_shader);
            SDL_ReleaseGPUShader(gpu_device, fragment_shader);

            println!("Setting Triangle");
        }
    });

    observer!("init_texture_shader", world, ShadersInitEvent, flecs::Any).each_iter(|it, _, _| {
        let event = &*it.param();
        let world = it.world();
        let gpu_device = event.gpu_device;
        let window = event.window;

        let vertex_shader = load_shader(gpu_device, "texture.vert", 0, 0, 0, 0).unwrap();
        let fragment_shader = load_shader(gpu_device, "texture.frag", 1, 0, 0, 0).unwrap();

        unsafe {
            let pipeline_create_info = SDL_GPUGraphicsPipelineCreateInfo {
                target_info: SDL_GPUGraphicsPipelineTargetInfo {
                    num_color_targets: 1,
                    color_target_descriptions: &SDL_GPUColorTargetDescription {
                        format: SDL_GetGPUSwapchainTextureFormat(gpu_device, window),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                vertex_input_state: SDL_GPUVertexInputState {
                    num_vertex_buffers: 1,
                    vertex_buffer_descriptions: &SDL_GPUVertexBufferDescription {
                        slot: 0,
                        input_rate: SDL_GPU_VERTEXINPUTRATE_VERTEX,
                        instance_step_rate: 0,
                        pitch: size_of::<PointTexture>() as u32,
                        ..Default::default()
                    },
                    num_vertex_attributes: 2,
                    vertex_attributes: [
                        SDL_GPUVertexAttribute {
                            buffer_slot: 0,
                            format: SDL_GPU_VERTEXELEMENTFORMAT_FLOAT3,
                            location: 0,
                            offset: 0,
                        },
                        SDL_GPUVertexAttribute {
                            buffer_slot: 0,
                            format: SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2,
                            location: 1,
                            offset: (size_of::<f32>() * 3) as u32,
                        },
                    ]
                    .as_ptr(),
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

    world.entity().set(Uuid::new()).set(Triangle::new(
        renderer.gpu_device,
        Vec3::new(-1.0, -1.0, 0.),
        Vec3::new(-0.5, -1.0, 0.),
        Vec3::new(-0.75, -0.5, 0.),
    ));
    world
        .entity()
        .set(Uuid::new())
        .set(TriangleWithTexture::new(
            renderer.gpu_device,
            [
                Vec3::new(-0.5, -0.5, 0.),
                Vec3::new(0.5, -0.5, 0.),
                Vec3::new(0.0, 0.5, 0.),
            ],
            "wall.bmp",
        ));
    world.set(window);
    world.set(renderer);

    let mut event = sdl3::events::SDL_Event::default();

    observer!("draw_texture_buffer", world, RenderEvent, flecs::Any).each_iter(|it, _, _| unsafe {
        let render_event = &*it.param();
        let render_pass = render_event.render_pass;
        let world = it.world();

        world.get::<&TexturePipeline>(|pipeline| {
            world.get::<&GpuApi>(|gpu_api| {
                let gpu_device = gpu_api.gpu_device;
                let triangle_query = world.query::<&TriangleWithTexture>().build();
                let graphics_pipeline = pipeline.0;
                triangle_query.each(|triangle| {
                    SDL_BindGPUGraphicsPipeline(render_pass, graphics_pipeline);
                    SDL_BindGPUVertexBuffers(
                        render_pass,
                        0,
                        &SDL_GPUBufferBinding {
                            buffer: triangle.vertex_buffer,
                            offset: 0,
                        },
                        1,
                    );
                    SDL_BindGPUIndexBuffer(
                        render_pass,
                        &SDL_GPUBufferBinding {
                            buffer: triangle.index_buffer,
                            offset: 0,
                        },
                        SDL_GPU_INDEXELEMENTSIZE_16BIT,
                    );
                    SDL_BindGPUFragmentSamplers(
                        render_pass,
                        0,
                        &SDL_GPUTextureSamplerBinding {
                            texture: triangle.texture,
                            sampler: triangle.sampler,
                        },
                        1,
                    );
                    SDL_DrawGPUIndexedPrimitives(render_pass, 6, 1, 0, 0, 0);
                });
            });
        });
    });

    observer!("draw_vertex_buffer", world, RenderEvent, flecs::Any).each_iter(|it, _, _| unsafe {
        let render_event = &*it.param();
        let render_pass = render_event.render_pass;
        let world = it.world();

        world.get::<&Pipeline>(|pipeline| {
            world.get::<&GpuApi>(|gpu_api| {
                let gpu_device = gpu_api.gpu_device;
                let triangle_query = world.query::<&Triangle>().build();
                triangle_query.each(|triangle| {
                    let graphics_pipeline = pipeline.0;

                    SDL_BindGPUGraphicsPipeline(render_pass, graphics_pipeline);
                    SDL_BindGPUVertexBuffers(
                        render_pass,
                        0,
                        &SDL_GPUBufferBinding {
                            buffer: triangle.vertex_buffer,
                            offset: 0,
                        },
                        1,
                    );
                    SDL_BindGPUIndexBuffer(
                        render_pass,
                        &SDL_GPUBufferBinding {
                            buffer: triangle.index_buffer,
                            offset: 0,
                        },
                        SDL_GPU_INDEXELEMENTSIZE_16BIT,
                    );

                    let transfer_buffer = SDL_CreateGPUTransferBuffer(
                        gpu_device,
                        &SDL_GPUTransferBufferCreateInfo {
                            usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                            size: (size_of::<Vec3>() * 3) as u32,
                            ..Default::default()
                        },
                    );

                    let transfer_data: *mut Vec3 =
                        SDL_MapGPUTransferBuffer(gpu_device, transfer_buffer, false) as *mut _;

                    *transfer_data.add(0) = triangle.points[0];
                    *transfer_data.add(1) = triangle.points[1];
                    *transfer_data.add(2) = triangle.points[2];

                    SDL_UnmapGPUTransferBuffer(gpu_device, transfer_buffer);

                    let command_buffer = SDL_AcquireGPUCommandBuffer(gpu_device);
                    let copy_pass = SDL_BeginGPUCopyPass(command_buffer);

                    SDL_UploadToGPUBuffer(
                        copy_pass,
                        &SDL_GPUTransferBufferLocation {
                            transfer_buffer,
                            offset: 0,
                        },
                        &SDL_GPUBufferRegion {
                            buffer: triangle.vertex_buffer,
                            offset: 0,
                            size: (size_of::<Vec3>() * 3) as u32,
                        },
                        false,
                    );

                    SDL_EndGPUCopyPass(copy_pass);
                    SDL_SubmitGPUCommandBuffer(command_buffer);
                    SDL_ReleaseGPUTransferBuffer(gpu_device, transfer_buffer);

                    SDL_DrawGPUIndexedPrimitives(
                        render_pass,
                        triangle.points.len() as u32,
                        1,
                        0,
                        0,
                        0,
                    );
                });
            });
        });
    });

    system!("draw_screen", world, &GpuApi, &Window)
        .singleton()
        .each_iter(|_it, _, (gpu_api, window)| {
            gpu_api.draw(window.0, &_it.world());
        });

    system!("update_triangle", world, &mut Triangle).each_iter(|it, _, triangle| {
        triangle.rotate(1.0 * it.delta_time());
    });

    'running: loop {
        while unsafe { sdl3::events::SDL_PollEvent(&mut event) } {
            match sdl3::events::SDL_EventType(unsafe { event.r#type }) {
                sdl3::events::SDL_EventType::QUIT => {
                    break 'running;
                }
                _ => {}
            }
        }

        world.progress();
    }

    unsafe {
        sdl3::init::SDL_Quit();
    }

    Ok(())
}
