#![cfg_attr(debug_assertions, allow(dead_code, unused))]

use std::env;

use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use log::info;
use pretty_env_logger;

mod font;
mod gui;
mod shader;

use crate::gui::*;

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
    if args.len() < 5 {
        println!("Usage: {} <font> <size> <string1> <string2>", args[0]);
        return Ok(());
    }

    let font_family = &args[1];
    let font_size = &args[2];
    let string1 = &args[3];
    let string2 = &args[4];

    let el = EventLoop::new();
    let dpr = el
        .available_monitors()
        .next()
        .map(|m| m.scale_factor())
        .unwrap_or(1.);
    info!("Device pixel ratio: {}", dpr);

    let mut screen = Screen::new(&el, &font_family, font_size.parse::<i32>().unwrap())?;
    screen.set_title("Peppa");

    screen.set_line(0, string1);
    screen.set_line(1, string2);

    run(screen, el);

    Ok(())
}

fn run(screen: Screen, el: EventLoop<()>) {
    el.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(physical_size) => screen.resize(physical_size),
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::RedrawRequested(_) => {
                screen.draw_frame();
            }
            Event::LoopDestroyed => return,
            _ => (),
        }
    });
}
