//! Peppa is a GPU enhanced terminal emulator.
//! It can also be used to play MUD, act as a SSH client, connect to BBS, etc.

#![warn(missing_docs, unused, dead_code)]
#![cfg_attr(debug_assertions, allow(unused, dead_code))]

mod font;
mod gui;
mod shader;

use {
    crate::gui::{Screen, Size},
    glutin::{
        dpi::PhysicalSize,
        event::{ElementState, Event, KeyboardInput, ModifiersState, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    log::info,
    std::env,
};

#[derive(Debug)]
enum Error {
    Gui(gui::Error),
}

impl From<gui::Error> for Error {
    fn from(err: gui::Error) -> Self {
        Error::Gui(err)
    }
}

fn main() -> Result<(), Error> {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Usage: {} <font> <size> <string1> [<stringN> ...]", args[0]);
        return Ok(());
    }

    let font_family = &args[1];
    let font_size = &args[2];

    let el = EventLoop::new();
    let (size, dpr) = el
        .available_monitors()
        .next()
        .map(|m| (m.size(), m.scale_factor()))
        .unwrap_or((
            PhysicalSize {
                width: 1024,
                height: 768,
            },
            1.0,
        ));

    info!(
        "Monitor physical size: {:?} Device pixel ratio: {}",
        size, dpr
    );

    let mut screen = Screen::new(&el, font_family, font_size.parse::<i32>().unwrap())?;
    screen.set_title("Peppa");
    screen.resize();

    redraw(&mut screen);

    run(screen, el);

    Ok(())
}

fn run(mut screen: Screen, el: EventLoop<()>) {
    let mut modifiers_state = Default::default();
    el.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => screen.resize(),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::ModifiersChanged(state) => modifiers_state = state,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Return),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    if modifiers_state.logo() {
                        screen.toggle_fullscreen();
                    }
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                redraw(&mut screen);
                screen.draw_frame();
            }
            Event::LoopDestroyed => {}
            _ => (),
        }
    });
}

fn redraw(screen: &mut Screen) {
    let args: Vec<String> = env::args().collect();
    let strs = &args[3..];

    for (i, s) in strs.iter().enumerate() {
        screen.set_line(i, s);
    }
}
