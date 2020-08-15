//! Peppa is a GPU enhanced terminal emulator.
//! It can also be used to play MUD, act as a SSH client, connect to BBS, etc.

#![deny(clippy::all, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)]
#![warn(missing_docs, unused, dead_code)]
#![cfg_attr(debug_assertions, allow(unused, dead_code))]

mod font;
mod gui;
mod shader;

use {
    crate::gui::Screen,
    glutin::{
        dpi::PhysicalSize,
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
    },
    log::info,
    std::env,
};

#[derive(Debug)]
enum Error {
    GUI(gui::Error),
}

impl From<gui::Error> for Error {
    fn from(err: gui::Error) -> Self {
        Error::GUI(err)
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
    let dpr = el
        .available_monitors()
        .next()
        .map(|m| m.scale_factor())
        .unwrap_or(1.);
    info!("Device pixel ratio: {}", dpr);

    let mut screen = Screen::new(&el, &font_family, font_size.parse::<i32>().unwrap())?;
    screen.set_title("Peppa");
    screen.resize(PhysicalSize {
        width: 1600,
        height: 1200,
    });

    redraw(&mut screen);

    run(screen, el);

    Ok(())
}

fn run(mut screen: Screen, el: EventLoop<()>) {
    el.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => screen.resize(physical_size),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
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
