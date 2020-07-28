use log::{debug, error};
use std::collections::HashMap;

use std::default::Default;
use std::mem;
use std::path::PathBuf;
use std::ptr;
use std::{fs, io};

use crossfont::{BitmapBuffer, FontKey, GlyphKey, RasterizedGlyph};

use crate::font::Font;

pub mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use gl::types::*;

/// Set OpenGL symbol loader. This call MUST be after window.make_current on windows.
pub fn setup_opengl<F>(loader: F)
where
    F: FnMut(&'static str) -> *const GLvoid,
{
    gl::load_with(loader);
}

#[derive(Debug)]
pub enum CreationError {
    Io(io::Error),
    Compile(PathBuf, String),
    Link(String),
    Font(crossfont::Error),
}

impl From<io::Error> for CreationError {
    fn from(val: io::Error) -> Self {
        Self::Io(val)
    }
}

impl From<crossfont::Error> for CreationError {
    fn from(err: crossfont::Error) -> Self {
        Self::Font(err)
    }
}

pub struct TextShader {
    program: GLuint,
    u_cell_size: GLint,
    u_draw_flag: GLint,

    cells: Vec<Vec<Cell>>,
    pub cell_width: f32,
    pub cell_height: f32,
    pub cell_descent: f32,

    glyph_cache: GlyphCache,

    /// Regular font.
    font_key: FontKey,
    // Bold font.
    // bold_key: FontKey,

    // Italic font.
    // italic_key: FontKey,

    // Bold italic font.
    // bold_italic_key: FontKey,
}

impl TextShader {
    pub fn new(dpr: f32, font_family: &str, font_size: i32) -> Result<TextShader, CreationError> {
        let vertex_shader = create_shader(gl::VERTEX_SHADER, TEXT_SHADER_V_PATH, TEXT_SHADER_V)?;
        let fragment_shader =
            create_shader(gl::FRAGMENT_SHADER, TEXT_SHADER_F_PATH, TEXT_SHADER_F)?;
        let program = create_program(vertex_shader, fragment_shader)?;

        let mut ft = Font::new(dpr, font_family, font_size);
        let font_key = ft.compute_font_keys()?;

        let (cell_descent, cell_width, cell_height) = Self::compute_cell_size(&mut ft)?;
        debug!(
            "cell_descent: {} cell_width: {} cell_height: {}",
            cell_descent, cell_width, cell_height
        );

        let glyph_cache = GlyphCache::new(ft)?;

        let mut u_cell_size: GLint = 0;
        let mut u_draw_flag: GLint = 0;
        unsafe {
            u_cell_size = gl::GetUniformLocation(program, b"cellSize\0".as_ptr() as *const _);
            u_draw_flag = gl::GetUniformLocation(program, b"drawFlag\0".as_ptr() as *const _);
        }

        let shader = Self {
            program,
            u_cell_size,
            u_draw_flag,
            cell_width,
            cell_height,
            cell_descent,
            font_key,
            glyph_cache,
            cells: Vec::new(),
        };

        Ok(shader)
    }

    pub fn draw_frame(&self) {
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::UseProgram(self.program);
        }

        for line in &self.cells {
            for cell in line {
                unsafe { gl::Uniform1i(self.u_draw_flag, 0) };
                cell.draw(0);
                unsafe { gl::Uniform1i(self.u_draw_flag, 1) };
                cell.draw(1);
                unsafe { gl::Uniform1i(self.u_draw_flag, 2) };
                cell.draw(2);
                unsafe { gl::Uniform1i(self.u_draw_flag, 3) };
                cell.draw(3);
            }
        }
    }

    pub fn set_text(&mut self, row: usize, col: usize, ch: char) {
        let glyph = self.glyph_cache.get(GlyphKey {
            font_key: self.font_key,
            c: ch,
            size: self.glyph_cache.font.size,
        });

        debug!(
            "ch: {} font descent: {} glyph: {:?}",
            ch, self.cell_descent, glyph
        );

        self.cells[row][col].set_text(ch, &glyph);
    }

    pub fn resize(&self, width: u32, height: u32) {
        unsafe { gl::Viewport(0, 0, width as _, height as _) };
    }

    pub fn set_size(&mut self, lines: usize, columns: usize) {
        if lines == 0 || columns == 0 {
            error!("Lines and columns must > 0");
            return;
        }

        let (delta_x, delta_y) = (2.0 / (columns as f32), 2.0 / (lines as f32));
        debug!("delta_x: {} delta_y: {}", delta_x, delta_y);

        unsafe {
            gl::UseProgram(self.program);
            gl::Uniform2f(self.u_cell_size, delta_x, delta_y);
        }

        let mut cells = Vec::new();
        for y in 0..lines {
            let mut row = Vec::new();
            for x in 0..columns {
                let cell = Cell::new(y, x);
                row.push(cell);
            }
            cells.push(row);
        }

        self.cells = cells;

        unsafe { gl::UseProgram(0) };
    }

    fn compute_cell_size(font: &mut Font) -> Result<(f32, f32, f32), CreationError> {
        let metrics = font.metrics()?;

        let offset_x = f64::from(0.0);
        let offset_y = f64::from(0.0);

        Ok((
            metrics.descent,
            ((metrics.average_advance + offset_x) as f32)
                .floor()
                .max(1.),
            ((metrics.line_height + offset_y) as f32).floor().max(1.),
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Cell {
    vao: GLuint,
    vbo: GLuint,
    ebo: GLuint,
    texture: GLuint,

    ch: char,
    gl_instance_attr: GlInstanceAttr,
}

/// GlInstanceAttr describes the instance properties passed to opengl shader.
/// Note that the fields here are in strict order, any modifications to it MUST
/// be synchronized with gl::VertexAttribPointer and GLSL scripts.
#[derive(Debug, Clone, Default)]
struct GlInstanceAttr {
    // gridCoords
    col: f32, // x
    row: f32, // y

    // bounding box size
    uv_width: f32,
    uv_height: f32,

    // bounding origin point
    uv_offset_x: f32,
    uv_offset_y: f32,

    // glyph baseline
    baseline: f32,
}

impl Cell {
    pub fn new(row: usize, col: usize) -> Cell {
        let mut vao: GLuint = 0;
        let mut vbo: GLuint = 0;
        let mut ebo: GLuint = 0;
        unsafe {
            // 创建 1 组 VAO 数据，将 ID 记入 vao。这里可以创建多个
            gl::GenVertexArrays(1, &mut vao);
            // 将 vao 绑定成为当前 VAO
            gl::BindVertexArray(vao);

            // 创建 1 个 BO，将 ID 记入 ebo
            gl::GenBuffers(1, &mut ebo);
            // 将 ebo 绑定成为当前 EBO
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

            // 为指定 target 绑定的 EBO 赋予数据，也就是把数据 copy 到 GPU memory
            let indices: [u32; 6] = [0, 1, 2, 3, 0, 2];
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER, // 指定 target 为 ELEMENT_ARRAY_BUFFER，即当前 EBO
                (mem::size_of::<u32>() * indices.len()) as _,
                indices.as_ptr() as _, // 数据位置
                gl::STATIC_DRAW,       // 数据内容不常变化
            );

            // 创建 1 个 BO，将 ID 记入 vbo
            gl::GenBuffers(1, &mut vbo);
            // 将 vbo 绑定到 ARRAY_BUFFER（专用于放置顶点数据）这个 target 上，成为当前 VBO
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            // 为指定 target 绑定的 VBO 分配内存，因为这里没有提供指针，所以并不拷贝数据
            gl::BufferData(
                gl::ARRAY_BUFFER, // 指定 target 为 ARRAY_BUFFER，即当前 VBO
                (mem::size_of::<GlInstanceAttr>()) as _,
                ptr::null(),     // 数据位置
                gl::STATIC_DRAW, // 数据内容不常变化
            );

            let sizeof_attr = mem::size_of::<GlInstanceAttr>();
            let sizeof_nf32 = |n| n * mem::size_of::<f32>();

            // 解释当前 VBO 里的数据，据此为当前 VAO 定义属性。这部分调用可以重复
            gl::VertexAttribPointer(
                0,                // 本次定义顶点的第 1 个属性（即 in vec2 gridCoords）
                2,                // 本属性由 2 个...
                gl::FLOAT,        //                 float32 来描述，也就是说是一个 vec2
                gl::FALSE,        // 是否对顶点数据进行归一化
                sizeof_attr as _, // 同一个属性在相邻两个实例之间的字节数
                ptr::null(),      // 本属性在每组顶点属性数据中的偏移量
            );
            // 启用当前 VAO 的第 1 个属性，同样，可以重复
            gl::EnableVertexAttribArray(0);
            // 说明当前 VAO 的第 1 个属性是个实例数组，仅在 1 个实例后进行更新，不按顶点更新
            gl::VertexAttribDivisor(0, 1);

            // 解释当前 VBO 里的数据，据此为当前 VAO 定义属性。这部分调用可以重复
            gl::VertexAttribPointer(
                1,                   // 本次定义顶点的第 2 个属性（即 in vec4 uvAttr）
                4,                   // 本属性由 4 个...
                gl::FLOAT,           //                 float32 来描述，也就是说是一个 vec4
                gl::FALSE,           // 是否对顶点数据进行归一化
                sizeof_attr as _,    // 同一个属性在相邻两个实例之间的字节数
                sizeof_nf32(2) as _, // 本属性在每组顶点属性数据中的偏移量
            );
            // 启用当前 VAO 的第 2 个属性，同样，可以重复
            gl::EnableVertexAttribArray(1);
            // 说明当前 VAO 的第 2 个属性是个实例数组，仅在 1 个实例后进行更新，不按顶点更新
            gl::VertexAttribDivisor(1, 1);

            // 解释当前 VBO 里的数据，据此为当前 VAO 定义属性。这部分调用可以重复
            gl::VertexAttribPointer(
                2,                   // 本次定义顶点的第 3 个属性（即 in float baseline）
                1,                   // 本属性由 1 个...
                gl::FLOAT,           //                 float32 来描述，也就是说是一个 float
                gl::FALSE,           // 是否对顶点数据进行归一化
                sizeof_attr as _,    // 同一个属性在相邻两个实例之间的字节数
                sizeof_nf32(6) as _, // 本属性在每组顶点属性数据中的偏移量
            );
            // 启用当前 VAO 的第 2 个属性，同样，可以重复
            gl::EnableVertexAttribArray(2);
            // 说明当前 VAO 的第 2 个属性是个实例数组，仅在 1 个实例后进行更新，不按顶点更新
            gl::VertexAttribDivisor(2, 1);
        }

        Self {
            vao: vao,
            vbo: vbo,
            ebo: ebo,
            gl_instance_attr: GlInstanceAttr {
                row: row as _,
                col: col as _,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn set_text(&mut self, ch: char, glyph: &Glyph) {
        self.ch = ch;
        self.texture = glyph.texture;
        self.gl_instance_attr.uv_width = glyph.uv_width;
        self.gl_instance_attr.uv_height = glyph.uv_height;
        self.gl_instance_attr.uv_offset_x = glyph.uv_left;
        self.gl_instance_attr.uv_offset_y = glyph.uv_bot;
        self.gl_instance_attr.baseline = glyph.uv_height - glyph.uv_ascent;
    }

    fn draw(&self, draw_flag: i8) {
        debug!("draw {}: {:?}", self.ch, self.gl_instance_attr);
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                (mem::size_of::<GlInstanceAttr>()) as _, // 同一个属性在相邻两个实例之间的字节数
                mem::transmute(&self.gl_instance_attr),
            );
            match draw_flag {
                0 => {
                    gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
                    gl::DrawElementsInstanced(gl::TRIANGLES, 6, gl::UNSIGNED_INT, ptr::null(), 1);
                }
                1 | 2 => {
                    gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
                    gl::DrawElementsInstanced(gl::LINE_LOOP, 4, gl::UNSIGNED_INT, ptr::null(), 1);
                }
                _ => {
                    gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
                    gl::DrawElementsInstanced(gl::LINE_LOOP, 4, gl::UNSIGNED_INT, ptr::null(), 1);
                }
            }
            gl::BindVertexArray(0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }
}

fn gl_get_info_log(kind: GLenum, obj: GLuint) -> String {
    let mut max_length: GLint = 0;

    let len_func = match kind {
        gl::PROGRAM => gl::GetProgramiv,
        gl::SHADER => gl::GetShaderiv,
        _ => return String::new(),
    };

    let log_func = match kind {
        gl::PROGRAM => gl::GetProgramInfoLog,
        gl::SHADER => gl::GetShaderInfoLog,
        _ => return String::new(),
    };

    unsafe {
        len_func(obj, gl::INFO_LOG_LENGTH, &mut max_length);
    }

    let mut actual_length: GLint = 0;
    let mut buf: Vec<u8> = Vec::with_capacity(max_length as _);

    unsafe {
        log_func(
            obj,
            max_length,
            &mut actual_length,
            buf.as_mut_ptr() as *mut _,
        );
        buf.set_len(actual_length as _);
    }

    String::from_utf8(buf).unwrap()
}

static TEXT_SHADER_V_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.v.glsl");
static TEXT_SHADER_F_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.f.glsl");

static TEXT_SHADER_V: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.v.glsl"));
static TEXT_SHADER_F: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.f.glsl"));

fn create_shader(kind: GLenum, path: &str, source: &str) -> Result<GLuint, CreationError> {
    let source = if let Ok(string) = fs::read_to_string(path) {
        string
    } else {
        String::from(source)
    };
    let len: [GLint; 1] = [source.len() as _];

    unsafe {
        let shader = gl::CreateShader(kind);
        gl::ShaderSource(shader, 1, &(source.as_ptr() as *const _), len.as_ptr());
        gl::CompileShader(shader);

        let mut success: GLint = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);

        if success == GLint::from(gl::TRUE) {
            Ok(shader)
        } else {
            let log = gl_get_info_log(gl::SHADER, shader);
            gl::DeleteShader(shader);
            Err(CreationError::Compile(PathBuf::from(path), log))
        }
    }
}

fn create_program(vertex: GLuint, fragment: GLuint) -> Result<GLuint, CreationError> {
    unsafe {
        let program = gl::CreateProgram();
        gl::AttachShader(program, vertex);
        gl::AttachShader(program, fragment);
        gl::LinkProgram(program);
        gl::DetachShader(program, vertex);
        gl::DetachShader(program, fragment);
        gl::DeleteShader(vertex);
        gl::DeleteShader(fragment);

        let mut success: GLint = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);

        if success == GLint::from(gl::TRUE) {
            gl::UseProgram(program);
            Ok(program)
        } else {
            let log = gl_get_info_log(gl::PROGRAM, program);
            gl::DeleteProgram(program);
            Err(CreationError::Link(log))
        }
    }
}

#[derive(Copy, Debug, Clone, Default)]
pub struct Glyph {
    pub texture: GLuint,
    pub colored: bool,
    pub top: f32,
    pub left: f32,
    pub width: f32,
    pub height: f32,
    pub uv_bot: f32,
    pub uv_left: f32,
    pub uv_width: f32,
    pub uv_height: f32,
    pub uv_ascent: f32,
}

pub struct GlyphCache {
    /// Cache of buffered glyphs.
    cache: HashMap<GlyphKey, Glyph>,

    /// Cache of buffered cursor glyphs.
    // cursor_cache: HashMap<CursorKey, Glyph>,
    font: Font,

    /// Glyph offset.
    // glyph_offset: Delta<i8>,

    /// Font metrics.
    metrics: crossfont::Metrics,
}

impl GlyphCache {
    pub fn new(mut font: Font) -> Result<GlyphCache, crossfont::Error> {
        let metrics = font.metrics()?;

        let cache = Self {
            cache: HashMap::default(),
            font: font,
            // font_key: regular,
            metrics,
        };

        Ok(cache)
    }

    pub fn get(&mut self, glyph_key: GlyphKey) -> Glyph {
        if let Some(glyph) = self.cache.get(&glyph_key) {
            return glyph.clone();
        }

        let rasterized = self
            .font
            .get_glyph(glyph_key)
            .unwrap_or_else(|_| Default::default());

        let glyph = self.load_glyph(&rasterized);
        self.cache.insert(glyph_key, glyph);

        return glyph;
    }

    pub fn load_glyph(&self, glyph: &RasterizedGlyph) -> Glyph {
        let colored = false;

        let mut texture: GLuint = 0; // 用来存放 Texture 的 ID
        unsafe {
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::GenTextures(1, &mut texture);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            // gl.BindTexture(gl.TEXTURE_2D_MULTISAMPLE, texture)
            // defer gl.BindTexture(gl.TEXTURE_2D_MULTISAMPLE, 0)

            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

            // Load data into OpenGL.
            let (colored, format, buf) = match &glyph.buf {
                BitmapBuffer::RGB(buf) => (false, gl::RGB, buf),
                BitmapBuffer::RGBA(buf) => (true, gl::RGBA, buf),
            };

            // TODO: gl::TexSubImage2D
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                format as _,
                glyph.width,
                glyph.height,
                0,
                format,
                gl::UNSIGNED_BYTE,
                buf.as_ptr() as *const _,
            );

            /*
                gl.TexImage2DMultisample(
                    gl.TEXTURE_2D_MULTISAMPLE, 0, gl.RGBA,
                    int32(rgba.Rect.Dx()), int32(rgba.Rect.Dy()),
                    0, gl.RGBA, gl.UNSIGNED_BYTE, gl.Ptr(rgba.Pix),
                )
            */

            gl::BindTexture(gl::TEXTURE_2D, 0);
        }

        // Generate UV coordinates.
        // let uv_bot = offset_y as f32 / 525.0;
        // let uv_left = offset_x as f32 / 240.0;

        Glyph {
            texture,
            colored,
            top: glyph.top as _,
            left: glyph.left as _,
            width: glyph.width as _,
            height: glyph.height as _,
            uv_bot: (-504 + 122 + glyph.top) as f32 * 2.0 / 1050.0,
            uv_left: (glyph.left as f32) * 2.0 / 2400.0,
            uv_width: (glyph.width as f32) * 2.0 / 2400.0,
            uv_height: (glyph.height as f32) * 2.0 / 1050.0,
            uv_ascent: glyph.top as f32 * 2.0 / 1050.0,
        }
    }
}
