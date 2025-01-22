use std::{ ffi::{ c_char, CStr, CString }, ptr::null_mut };

use flecs_ecs::{ core::{ SystemAPI, TermBuilderImpl, World }, macros::{ system, Component } };
use sdl3_sys::{
    self as sdl3,
    error::SDL_GetError,
    gpu::{
        SDL_AcquireGPUCommandBuffer, SDL_BeginGPUCopyPass, SDL_BeginGPURenderPass, SDL_BindGPUGraphicsPipeline, SDL_BindGPUVertexBuffers, SDL_ClaimWindowForGPUDevice, SDL_CreateGPUBuffer, SDL_CreateGPUDevice, SDL_CreateGPUGraphicsPipeline, SDL_CreateGPUShader, SDL_CreateGPUTransferBuffer, SDL_DrawGPUPrimitives, SDL_EndGPUCopyPass, SDL_EndGPURenderPass, SDL_GPUBuffer, SDL_GPUBufferBinding, SDL_GPUBufferCreateInfo, SDL_GPUBufferRegion, SDL_GPUColorTargetDescription, SDL_GPUColorTargetInfo, SDL_GPUDevice, SDL_GPUGraphicsPipeline, SDL_GPUGraphicsPipelineCreateInfo, SDL_GPUGraphicsPipelineTargetInfo, SDL_GPUShader, SDL_GPUShaderCreateInfo, SDL_GPUShaderStage, SDL_GPUTexture, SDL_GPUTransferBufferCreateInfo, SDL_GPUTransferBufferLocation, SDL_GPUVertexAttribute, SDL_GPUVertexBufferDescription, SDL_GPUVertexInputState, SDL_GetGPUShaderFormats, SDL_GetGPUSwapchainTextureFormat, SDL_MapGPUTransferBuffer, SDL_ReleaseGPUShader, SDL_ReleaseGPUTransferBuffer, SDL_SubmitGPUCommandBuffer, SDL_UnmapGPUTransferBuffer, SDL_UploadToGPUBuffer, SDL_WaitAndAcquireGPUSwapchainTexture, SDL_GPU_BUFFERUSAGE_VERTEX, SDL_GPU_LOADOP_CLEAR, SDL_GPU_PRIMITIVETYPE_TRIANGLELIST, SDL_GPU_SHADERFORMAT_DXIL, SDL_GPU_SHADERFORMAT_INVALID, SDL_GPU_SHADERFORMAT_MSL, SDL_GPU_SHADERFORMAT_SPIRV, SDL_GPU_SHADERSTAGE_FRAGMENT, SDL_GPU_SHADERSTAGE_VERTEX, SDL_GPU_STOREOP_STORE, SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD, SDL_GPU_VERTEXELEMENTFORMAT_FLOAT3, SDL_GPU_VERTEXELEMENTFORMAT_UBYTE4, SDL_GPU_VERTEXELEMENTFORMAT_UBYTE4_NORM, SDL_GPU_VERTEXINPUTRATE_VERTEX
    },
    iostream::SDL_LoadFile,
    pixels::SDL_FColor,
    stdinc::{ SDL_free, SDL_strstr },
    video::SDL_Window,
};

/* static mut LINE_PIPELINE: *mut SDL_GPUGraphicsPipeline = null_mut();
static mut FILL_PIPELINE: *mut SDL_GPUGraphicsPipeline = null_mut(); */
static mut PIPELINE: *mut SDL_GPUGraphicsPipeline = null_mut();
static mut VERTEX_BUFFER: *mut SDL_GPUBuffer = null_mut();

#[derive(Debug, Component)]
pub struct Window(pub *mut SDL_Window);

#[derive(Debug, Component)]
pub struct GpuApi {
    pub gpu_device: *mut SDL_GPUDevice,
    pub color: (f32, f32, f32),
}

#[derive(Debug, Component)]
pub struct Rect {
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Component)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct PositionColorVertex {
    pub position: Vec3,
    pub color: Color,
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
                color: (0.0, 0.0, 0.0),
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

                SDL_BindGPUGraphicsPipeline(render_pass, PIPELINE);
                SDL_BindGPUVertexBuffers(
                    render_pass,
                    0,
                    &(SDL_GPUBufferBinding {
                        buffer: VERTEX_BUFFER,
                        offset: 0,
                    }),
                    1
                );
                SDL_DrawGPUPrimitives(render_pass, 3, 1, 0, 0);

                SDL_EndGPURenderPass(render_pass);
            }

            SDL_SubmitGPUCommandBuffer(cmd_buf);
        }
    }

    pub fn set_color(&mut self, color: (f32, f32, f32)) {
        self.color = color;
    }

    #[allow(unused_assignments)]
    pub fn load_shader(
        &self,
        file_name: &str,
        sampler_count: u32,
        uniform_buffer_count: u32,
        storage_buffer_count: u32,
        storage_text_count: u32
    ) -> Result<*mut SDL_GPUShader, String> {
        let file_name = CString::new(file_name).unwrap();
        unsafe {
            let base_path = CString::new(
                env!("CARGO_MANIFEST_DIR")
            ).unwrap(); /* SDL_GetBasePath(); */
            let mut stage = SDL_GPUShaderStage::default();
            if
                SDL_strstr(file_name.as_ptr(), CString::new(".vert").unwrap().as_ptr()) !=
                null_mut()
            {
                stage = SDL_GPU_SHADERSTAGE_VERTEX;
            } else if
                SDL_strstr(file_name.as_ptr(), CString::new(".frag").unwrap().as_ptr()) !=
                null_mut()
            {
                stage = SDL_GPU_SHADERSTAGE_FRAGMENT;
            } else {
                return Err("Invalid shader file extension".to_owned());
            }

            let mut full_path: *mut c_char = null_mut();
            let backend_formats = SDL_GetGPUShaderFormats(self.gpu_device);
            let mut format = SDL_GPU_SHADERFORMAT_INVALID;
            let mut entrypoint = CString::new("").unwrap();

            if (backend_formats & SDL_GPU_SHADERFORMAT_SPIRV) != 0 {
                full_path = CString::new(
                    format!(
                        "{}/Shaders/Compiled/SPIRV/{}.spv",
                        base_path.to_str().unwrap(),
                        file_name.to_str().unwrap()
                    )
                )
                    .unwrap()
                    .into_raw();
                format = SDL_GPU_SHADERFORMAT_SPIRV;
                entrypoint = CString::new("main").unwrap();
            } else if (backend_formats & SDL_GPU_SHADERFORMAT_MSL) != 0 {
                full_path = CString::new(
                    format!(
                        "{}/Shaders/Compiled/SPIRV/{}.msl",
                        base_path.to_str().unwrap(),
                        file_name.to_str().unwrap()
                    )
                )
                    .unwrap()
                    .into_raw();
                format = SDL_GPU_SHADERFORMAT_MSL;
                entrypoint = CString::new("main0").unwrap();
            } else if (backend_formats & SDL_GPU_SHADERFORMAT_DXIL) != 0 {
                full_path = CString::new(
                    format!(
                        "{}/Shaders/Compiled/SPIRV/{}.dxil",
                        base_path.to_str().unwrap(),
                        file_name.to_str().unwrap()
                    )
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

            let shader = SDL_CreateGPUShader(self.gpu_device, &shader_info);
            if shader == null_mut() {
                SDL_free(code as *mut _);
                return Err("Failed to create shader".to_owned());
            }

            SDL_free(code as *mut _);
            return Ok(shader);
        }
    }

    pub fn init(&self, window: *mut SDL_Window) -> Result<(), String> {
        unsafe {
            let vertex_shader = self.load_shader("PositionColor.vert", 0, 0, 0, 0)?;
            let fragment_shader = self.load_shader("SolidColor.frag", 0, 0, 0, 0)?;

            let pipeline_create_info = SDL_GPUGraphicsPipelineCreateInfo {
                target_info: SDL_GPUGraphicsPipelineTargetInfo {
                    num_color_targets: 1,
                    color_target_descriptions: &(SDL_GPUColorTargetDescription {
                        format: SDL_GetGPUSwapchainTextureFormat(self.gpu_device, window),
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
                        pitch: size_of::<PositionColorVertex>() as u32,
                    }),
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
                            format: SDL_GPU_VERTEXELEMENTFORMAT_UBYTE4_NORM,
                            location: 1,
                            offset: (size_of::<f32>() * 3) as u32,
                        },
                    ].as_ptr(),
                },
                primitive_type: SDL_GPU_PRIMITIVETYPE_TRIANGLELIST,
                vertex_shader,
                fragment_shader,
                ..Default::default()
            };

            /* pipeline_create_info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            FILL_PIPELINE = SDL_CreateGPUGraphicsPipeline(self.gpu_device, &pipeline_create_info);
            if FILL_PIPELINE == null_mut() {
                return Err("Failed to create fill graphics pipeline".to_owned());
            }

            pipeline_create_info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_LINE;
            LINE_PIPELINE = SDL_CreateGPUGraphicsPipeline(self.gpu_device, &pipeline_create_info);
            if LINE_PIPELINE == null_mut() {
                return Err("Failed to create line graphics pipeline".to_owned());
            } */
            PIPELINE = SDL_CreateGPUGraphicsPipeline(self.gpu_device, &pipeline_create_info);
            if PIPELINE == null_mut() {
                return Err("Failed to create graphics pipeline".to_owned());
            }

            SDL_ReleaseGPUShader(self.gpu_device, vertex_shader);
            SDL_ReleaseGPUShader(self.gpu_device, fragment_shader);

            VERTEX_BUFFER = SDL_CreateGPUBuffer(
                self.gpu_device,
                &(SDL_GPUBufferCreateInfo {
                    usage: SDL_GPU_BUFFERUSAGE_VERTEX,
                    size: 3 * (size_of::<PositionColorVertex>() as u32),
                    ..Default::default()
                })
            );

            let transfer_buffer = SDL_CreateGPUTransferBuffer(
                self.gpu_device,
                &(SDL_GPUTransferBufferCreateInfo {
                    usage: SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD,
                    size: 3 * (size_of::<PositionColorVertex>() as u32),
                    ..Default::default()
                })
            );

            let transfer_data: *mut PositionColorVertex = SDL_MapGPUTransferBuffer(
                self.gpu_device,
                transfer_buffer,
                false
            ) as *mut _;

            let transfer_data_slice = std::slice::from_raw_parts_mut(transfer_data, 3);

            transfer_data_slice[0] = PositionColorVertex {
                position: Vec3 { x: -1., y: -1.0, z: 0.0 },
                color: Color { r: 255, g: 0, b: 0, a: 255 },
            };
            transfer_data_slice[1] = PositionColorVertex {
                position: Vec3 { x: 1.0, y: -1.0, z: 0.0 },
                color: Color { r: 0, g: 255, b: 0, a: 255 },
            };
            transfer_data_slice[2] = PositionColorVertex {
                position: Vec3 { x: 0.0, y: 1.0, z: 0.0 },
                color: Color { r: 0, g: 0, b: 255, a: 255 },
            };

            SDL_UnmapGPUTransferBuffer(self.gpu_device, transfer_buffer);

            let upload_cmd_buf = SDL_AcquireGPUCommandBuffer(self.gpu_device);
            let copy_pass = SDL_BeginGPUCopyPass(upload_cmd_buf);

            SDL_UploadToGPUBuffer(
                copy_pass,
                &(SDL_GPUTransferBufferLocation {
                    transfer_buffer,
                    offset: 0,
                }),
                &(SDL_GPUBufferRegion {
                    buffer: VERTEX_BUFFER,
                    offset: 0,
                    size: 3 * (size_of::<PositionColorVertex>() as u32),
                }),
                false
            );

            SDL_EndGPUCopyPass(copy_pass);
            SDL_SubmitGPUCommandBuffer(upload_cmd_buf);
            SDL_ReleaseGPUTransferBuffer(self.gpu_device, transfer_buffer);

            Ok(())
        }
    }
    /* pub fn create_shader_pipeline(&self) -> Result<(), String> {
        unsafe {
            let vertex_shader = SDL_CreateGPUShader(
                self.gpu_device,
                &mut (SDL_GPUShaderCreateInfo {
                    code_size: vert_shader.len() as usize,
                    code: vert_shader.as_ptr() as *const _,
                    entrypoint: CString::new("main").unwrap().as_ptr(),
                    format: SDL_GPU_SHADERFORMAT_SPIRV,
                    stage: SDL_GPUShaderStage::VERTEX,
                    ..Default::default()
                })
            );

            if vertex_shader == null_mut() {
                return Err("Failed to create vertex shader".to_string());
            }

            let fragment_shader = SDL_CreateGPUShader(
                self.gpu_device,
                &mut (SDL_GPUShaderCreateInfo {
                    code_size: frag_shader.len() as usize,
                    code: frag_shader.as_ptr() as *const _,
                    entrypoint: CString::new("main").unwrap().as_ptr(),
                    format: SDL_GPU_SHADERFORMAT_SPIRV,
                    stage: SDL_GPUShaderStage::FRAGMENT,
                    ..Default::default()
                })
            );

            // Pipeline configuration
            let pipeline_config = SDL_CreateGPUGraphicsPipeline(
                self.gpu_device,
                &mut (SDL_GPUGraphicsPipelineCreateInfo {
                    vertex_shader,
                    fragment_shader,
                    ..Default::default()
                })
            );

            if pipeline_config == null_mut() {
                return Err("Failed to create graphics pipeline".to_string());
            }

            Ok(())
        }
    } */
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
    /* world.component::<Rect>();
    world.component::<Position>(); */

    let window = Window::new("Example window", 800, 600);
    let renderer = GpuApi::new(window.0);
    renderer.init(window.0).unwrap();
    //renderer.create_shader_pipeline().unwrap();

    /* let _bob = world
        .entity_named("bob")
        .set(Rect { w: 100.0, h: 100.0 })
        .set(Position { x: 0.0, y: 0.0 }); */

    world.set(window);
    world.set(renderer);

    let mut event = sdl3::events::SDL_Event::default();

    system!("draw_screen", world, &GpuApi, &Window)
        .singleton()
        .each_iter(|_it, _, (gpu_api, window)| {
            gpu_api.draw(window.0);
            /* let world = _it.world(); */

            /* world.entity().get::<(&Position, &Rect)>(|(position, rect)| {
                gpu_api.draw_vertex_buffer(window.0, position, rect, (1.0, 1.0, 1.0));
            }); */
        });

    //let start_time = std::time::Instant::now();
    println!("Starting loop");

    'running: loop {
        while unsafe { sdl3::events::SDL_PollEvent(&mut event) } {
            match sdl3::events::SDL_EventType(unsafe { event.r#type }) {
                sdl3::events::SDL_EventType::QUIT => {
                    break 'running;
                }
                _ => {}
            }
        }

        /* let elapsed_time = start_time.elapsed().as_secs_f32();

        let red = elapsed_time.sin() * 127.0 + 128.0;
        let green = elapsed_time.cos() * 127.0 + 128.0;
        let blue = elapsed_time.cos() * 127.0 + 128.0; */

        /* world.get::<&mut GpuApi>(|gpu_api| {
            gpu_api.set_color((red / 255.0, green / 255.0, blue / 255.0));
        }); */

        std::thread::sleep(std::time::Duration::from_millis(10));
        world.progress();
    }

    unsafe {
        sdl3::init::SDL_Quit();
    }

    Ok(())
}
