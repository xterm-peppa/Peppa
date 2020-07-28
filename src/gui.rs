use log::{debug, info};

use glutin::dpi::{PhysicalPosition, PhysicalSize};
use glutin::event_loop::EventLoop;
use glutin::window::{CursorIcon, WindowBuilder};
use glutin::{self, ContextBuilder, PossiblyCurrent, WindowedContext};

use crate::shader;

#[derive(Debug)]
pub enum Error {
    Glutin(glutin::CreationError),
    Shader(shader::CreationError),
}

impl From<glutin::CreationError> for Error {
    fn from(err: glutin::CreationError) -> Self {
        Self::Glutin(err)
    }
}

impl From<shader::CreationError> for Error {
    fn from(err: shader::CreationError) -> Self {
        Self::Shader(err)
    }
}

pub struct Screen {
    title: String,

    lines: usize,
    columns: usize,

    width: f32,
    height: f32,

    pub wc: WindowedContext<PossiblyCurrent>,
    pub shader: shader::TextShader,
}

impl Screen {
    pub fn new(el: &EventLoop<()>, font_family: &str, font_size: i32) -> Result<Screen, Error> {
        let title = String::from("Peppa");
        let wb = WindowBuilder::new();
        let wc = ContextBuilder::new().build_windowed(wb, el)?;
        let wc = unsafe { wc.make_current().unwrap() };

        shader::setup_opengl(|symbol| wc.get_proc_address(symbol) as *const _);

        wc.window().set_title(title.as_str());
        wc.window().set_cursor_icon(CursorIcon::Text);

        info!(
            "Pixel format of the window's GL context: {:?}",
            wc.get_pixel_format()
        );

        let dpr = wc.window().current_monitor().scale_factor();
        info!("Device pixel ratio: {}", dpr);

        let mut shader = shader::TextShader::new(dpr as _, font_family, font_size)?;

        let (columns, lines) = (10, 2);
        let (screen_width, screen_height) = (
            shader.cell_width * columns as f32,
            shader.cell_height * lines as f32,
        );

        shader.set_size(lines, columns);

        debug!(
            "screen_width: {}, screen_height: {}",
            screen_width, screen_height
        );

        wc.window()
            .set_inner_size(PhysicalSize::new(screen_width, screen_height));

        Ok(Self {
            title: title,
            wc: wc,
            lines: lines,
            columns: columns,
            width: screen_width,
            height: screen_height,
            shader: shader,
        })
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
        self.wc.window().set_title(title);
    }

    pub fn resize(&self, physical_size: PhysicalSize<u32>) {
        self.wc.resize(physical_size);
        let full_size = self.wc.window().current_monitor().size();
        self.wc.window().set_outer_position(PhysicalPosition {
            x: (full_size.width - physical_size.width) / 2,
            y: (full_size.height - physical_size.height) / 2,
        });
        self.shader
            .resize(physical_size.width, physical_size.height);
    }

    pub fn draw_frame(&self) {
        self.shader.draw_frame();
        self.wc.swap_buffers().unwrap();
    }

    pub fn set_line(&mut self, row: usize, s: &str) {
        for (i, ch) in s.chars().enumerate() {
            self.shader.set_text(row, i, ch);
        }
    }
}
