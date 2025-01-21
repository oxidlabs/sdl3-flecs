use std::{ ffi::{CStr, CString}, ptr::null_mut };

use flecs_ecs::{ core::{IdOperations, World}, macros::Component };
use sdl3_sys::{ self as sdl3, error::SDL_GetError, render::SDL_Renderer, video::SDL_Window };

#[derive(Debug, Component)]
pub struct Window(pub *mut SDL_Window);

#[derive(Debug, Component)]
pub struct Renderer(pub *mut SDL_Renderer);

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

impl Renderer {
    pub fn new(window: *mut SDL_Window) -> Self {
        unsafe {
            let renderer = sdl3::render::SDL_CreateRenderer(
                window,
                null_mut(),
            );
            if renderer == null_mut() {
                let error = CStr::from_ptr(SDL_GetError()).to_str().unwrap();
                panic!("Failed to create renderer: {:?}", error);
            }
            Self(renderer)
        }
    }

    pub fn redraw(&self) {
        unsafe {
            sdl3::render::SDL_RenderClear(self.0);
            sdl3::render::SDL_RenderPresent(self.0);
        }
    }

    pub fn set_background(&self, red: u8, green: u8, blue: u8) {
        unsafe {
            sdl3::render::SDL_SetRenderDrawColor(self.0, red, green, blue, 255);
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
    world.component::<Renderer>();

    let window = Window::new("Example window", 800, 600);
    let renderer = Renderer::new(window.0);

    world.set(window);
    world.set(renderer);

    world.get::<&Renderer>(|renderer| {
        renderer.set_background(0, 255, 0);
        renderer.redraw();
    });

    let mut event = sdl3::events::SDL_Event::default();

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

        let red = (elapsed_time.sin() * 127.0 + 128.0) as u8;
        let green = (elapsed_time.cos() * 127.0 + 128.0) as u8;
        let blue = (elapsed_time.cos() * 127.0 + 128.0) as u8;

        world.get::<&Renderer>(|renderer| {
            renderer.set_background(red, green, blue);
            renderer.redraw();
        });

        std::thread::sleep(std::time::Duration::from_millis(10));
        world.progress();
    }

    unsafe {
        sdl3::init::SDL_Quit();
    }

    Ok(())
}
