use std::{ffi::CString, ptr::null_mut};

use flecs_ecs::macros::Component;
use sdl3_sys::{self as sdl3, properties::*, video::*};

#[derive(Debug, Component)]
pub struct Window(pub *mut SDL_Window);

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    pub fn new(title: &str, width: i64, height: i64) -> Self {
        unsafe {
            let props: SDL_PropertiesID = SDL_CreateProperties();

            SDL_SetStringProperty(
                props,
                SDL_PROP_WINDOW_CREATE_TITLE_STRING,
                CString::new(title).unwrap().as_ptr(),
            );
            SDL_SetNumberProperty(
                props,
                SDL_PROP_WINDOW_CREATE_X_NUMBER,
                SDL_WINDOWPOS_CENTERED as i64,
            );
            SDL_SetNumberProperty(
                props,
                SDL_PROP_WINDOW_CREATE_Y_NUMBER,
                SDL_WINDOWPOS_CENTERED as i64,
            );
            SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_WIDTH_NUMBER, width);
            SDL_SetNumberProperty(props, SDL_PROP_WINDOW_CREATE_HEIGHT_NUMBER, height);
            SDL_SetBooleanProperty(props, SDL_PROP_WINDOW_CREATE_RESIZABLE_BOOLEAN, true);

            let window = sdl3::video::SDL_CreateWindowWithProperties(props);
            if window == null_mut() {
                panic!("Failed to create window");
            }
            //SDL_SetWindowSurfaceVSync(window, 0);
            Self(window)
        }
    }
}
