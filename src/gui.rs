use {
    crate::shader,
    glutin::{
        self,
        dpi::{PhysicalPosition, PhysicalSize},
        event_loop::EventLoop,
        window::{CursorIcon, WindowBuilder},
        ContextBuilder, PossiblyCurrent, WindowedContext,
    },
    log::info,
};

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

        let shader = shader::TextShader::new(dpr as _, font_family, font_size)?;

        Ok(Self {
            title,
            wc,
            lines: 0,
            columns: 0,
            shader,
        })
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
        self.wc.window().set_title(title);
    }

    pub fn resize(&mut self, window_size: PhysicalSize<u32>) {
        let full_size = self.wc.window().current_monitor().size();

        self.columns = (window_size.width as f32 / self.shader.cell_width).floor() as usize;
        self.lines = (window_size.height as f32 / self.shader.cell_height).floor() as usize;

        info!(
            "window_width: {}, window_height: {}, columns: {}, lines: {}",
            window_size.width, window_size.height, self.columns, self.lines
        );

        self.wc.resize(window_size);
        self.wc.window().set_outer_position(PhysicalPosition {
            x: (full_size.width - window_size.width) / 2,
            y: (full_size.height - window_size.height) / 2,
        });

        self.shader.resize(window_size.width, window_size.height);
        self.shader.set_size(self.lines, self.columns);
    }

    pub fn draw_frame(&self) {
        self.shader.draw_frame();
        self.wc.swap_buffers().unwrap();
    }

    pub fn set_line(&mut self, row: usize, s: &str) {
        if row >= self.lines {
            return;
        }

        for (i, ch) in s.chars().take(self.columns).enumerate() {
            self.shader.set_text(row, i, ch);
        }
    }
}
