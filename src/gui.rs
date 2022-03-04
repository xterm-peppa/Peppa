use {
    crate::shader,
    glutin::{
        self,
        dpi::{PhysicalPosition, PhysicalSize},
        event_loop::EventLoop,
        window::{CursorIcon, Fullscreen, WindowBuilder},
        ContextBuilder, PossiblyCurrent, WindowedContext,
    },
    log::info,
};

#[derive(Debug)]
pub enum Error {
    Glutin(glutin::CreationError),
    Shader(shader::CreationError),
    Other(String),
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

#[derive(Debug)]
pub struct Size {
    pub lines: usize,
    pub columns: usize,
}

pub struct Screen {
    title: String,

    size: Size,

    pub wc: WindowedContext<PossiblyCurrent>,
    pub shader: shader::TextShader,
}

impl Screen {
    pub fn new(el: &EventLoop<()>, font_family: &str, font_size: i32) -> Result<Screen, Error> {
        let title = String::from("Peppa");
        let wb = WindowBuilder::new();
        let wc = ContextBuilder::new().build_windowed(wb, el)?;
        let wc = unsafe { wc.make_current().unwrap() };
        let win = wc.window();

        shader::setup_opengl(|symbol| wc.get_proc_address(symbol) as *const _);

        win.set_title(title.as_str());
        win.set_cursor_icon(CursorIcon::Text);
        win.set_visible(true);

        info!(
            "Pixel format of the window's GL context: {:?}",
            wc.get_pixel_format()
        );

        let dpr = win.current_monitor().scale_factor();
        info!("Device pixel ratio: {}", dpr);

        let shader = shader::TextShader::new(dpr as _, font_family, font_size)?;
        let size = Size {
            lines: 25,
            columns: 80,
        };

        Ok(Self {
            title,
            wc,
            size,
            shader,
        })
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
        self.wc.window().set_title(title);
    }

    pub fn toggle_fullscreen(&mut self) {
        self.wc
            .window()
            .set_fullscreen(self.wc.window().fullscreen().map_or(
                Some(Fullscreen::Borderless(self.wc.window().current_monitor())),
                |_| None,
            ));
    }

    pub fn resize(&mut self) {
        let window_size = self.wc.window().inner_size();

        let size = Size {
            columns: (window_size.width as f32 / self.shader.cell_width).floor() as usize,
            lines: (window_size.height as f32 / self.shader.cell_height).floor() as usize,
        };

        info!(
            "window_width: {}, window_height: {}, columns: {}, lines: {}",
            window_size.width, window_size.height, size.columns, size.lines
        );

        self.wc.resize(window_size);
        self.wc.window().request_redraw();
        info!("term size: {:?}, windows size: {:?}", size, window_size);

        self.shader.resize(window_size.width, window_size.height);
        self.shader.set_size(size.lines, size.columns);
        self.size = size;
    }

    pub fn draw_frame(&self) {
        self.shader.draw_frame();
        self.wc.swap_buffers().unwrap();
    }

    pub fn set_line(&mut self, row: usize, s: &str) {
        if row >= self.size.lines {
            return;
        }

        for (i, ch) in s.chars().take(self.size.columns).enumerate() {
            self.shader.set_text(row, i, ch);
        }
    }
}
