mod gl {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

use {
    crate::font::Font,
    crossfont::{BitmapBuffer, FontKey, GlyphKey, RasterizedGlyph},
    gl::types::*,
    log::{debug, error},
    std::{collections::HashMap, default::Default, fs, io, mem, path::PathBuf, ptr},
};

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
    u_window_size: GLint,
    u_draw_flag: GLint,

    cells: Vec<Vec<Cell>>,

    dpr: f32,
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

static TEXT_SHADER_V_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.v.glsl");
static TEXT_SHADER_F_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.f.glsl");

static TEXT_SHADER_V: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.v.glsl"));
static TEXT_SHADER_F: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/text.f.glsl"));

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

        let (u_cell_size, u_window_size, u_draw_flag) = unsafe {
            (
                gl::GetUniformLocation(program, b"cellSize\0".as_ptr() as *const _),
                gl::GetUniformLocation(program, b"windowSize\0".as_ptr() as *const _),
                gl::GetUniformLocation(program, b"drawFlag\0".as_ptr() as *const _),
            )
        };

        let shader = Self {
            program,
            u_cell_size,
            u_window_size,
            u_draw_flag,
            dpr,
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
        unsafe {
            gl::Viewport(0, 0, width as _, height as _);
            gl::Uniform2f(self.u_window_size, width as f32, height as f32);
        };
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

        let offset_x = 0.0;
        let offset_y = 0.0;

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
            // Create VAO.
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            // Create EBO.
            gl::GenBuffers(1, &mut ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);

            // Set EBO.
            // NOTE that the vertex index vector is cleverly set here.
            // We can either use all 6 indices to draw the texture,
            // or we can use only the first 4 indices to draw the bounding box.
            let indices: [u32; 6] = [0, 1, 2, 3, 0, 2];
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (mem::size_of::<u32>() * indices.len()) as _,
                indices.as_ptr() as _,
                gl::STATIC_DRAW,
            );

            // Create VBO.
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            // Just allocate GPU memory to the VBO here.
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (mem::size_of::<GlInstanceAttr>()) as _,
                ptr::null(),
                gl::STATIC_DRAW,
            );

            let sizeof_attr = mem::size_of::<GlInstanceAttr>();
            let define_vertex_attrib = |idx, n, offset| {
                // Define vertex attrib pointer
                gl::VertexAttribPointer(
                    idx,              // Attrib index.
                    n,                // Attrib size, in gl::FLOAT.
                    gl::FLOAT,        // Attrib type.
                    gl::FALSE,        // Don't be normalized.
                    sizeof_attr as _, // Attrib stride.
                    offset as _,      // Attrib pointer, offset of GlInstanceAttr.
                );
                // Enable it.
                gl::EnableVertexAttribArray(idx);
                // Vertex attributes are changed only when the instance changes.
                gl::VertexAttribDivisor(idx, 1);
                (idx + 1, offset + n * (mem::size_of::<f32>() as i32))
            };

            // Define vertex attributes.

            let (idx, offset) = (0, 0);
            // in vec2 gridCoords
            let (idx, offset) = define_vertex_attrib(idx, 2, offset);
            // in vec4 uvAttr
            let (idx, offset) = define_vertex_attrib(idx, 4, offset);
            // in float baseline
            let (idx, offset) = define_vertex_attrib(idx, 1, offset);

            // Just for make linter happy.
            let (_, _) = (idx, offset);
        }

        Self {
            vao,
            vbo,
            ebo,
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

        let (cell_descent, dpr) = (36.352074, 2.0); // FIXME: hard-code
        self.gl_instance_attr.uv_width = glyph.width * dpr;
        self.gl_instance_attr.uv_height = glyph.height * dpr;
        self.gl_instance_attr.uv_offset_x = glyph.left * dpr;
        self.gl_instance_attr.uv_offset_y = (glyph.top + cell_descent) * dpr;
        self.gl_instance_attr.baseline = cell_descent * dpr;
    }

    fn draw(&self, draw_flag: i8) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.ebo);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BufferSubData(
                gl::ARRAY_BUFFER,
                0,
                (mem::size_of::<GlInstanceAttr>()) as _,
                mem::transmute(&self.gl_instance_attr),
            );
            match draw_flag {
                // draw texture(0)
                0 => {
                    debug!("draw {}: {:?}", self.ch, self.gl_instance_attr);
                    gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
                    gl::DrawElementsInstanced(gl::TRIANGLES, 6, gl::UNSIGNED_INT, ptr::null(), 1);
                }
                // draw bounding box(1) or cell box(2)
                1 | 2 => {
                    gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
                    gl::DrawElementsInstanced(gl::LINE_LOOP, 4, gl::UNSIGNED_INT, ptr::null(), 1);
                }
                // draw baseline(3)
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

#[derive(Copy, Debug, Clone, Default)]
pub struct Glyph {
    pub texture: GLuint,
    pub top: f32,
    pub left: f32,
    pub width: f32,
    pub height: f32,
}

pub struct GlyphCache {
    /// Cache of buffered glyphs.
    cache: HashMap<GlyphKey, Glyph>,

    font: Font,
}

impl GlyphCache {
    pub fn new(font: Font) -> Result<GlyphCache, crossfont::Error> {
        let cache = Self {
            cache: HashMap::default(),
            font,
        };

        Ok(cache)
    }

    pub fn get(&mut self, glyph_key: GlyphKey) -> Glyph {
        if let Some(glyph) = self.cache.get(&glyph_key) {
            return *glyph;
        }

        let rasterized = self
            .font
            .get_glyph(glyph_key)
            .unwrap_or_else(|_| Default::default());

        let glyph = self.load_glyph(&rasterized);
        self.cache.insert(glyph_key, glyph);

        glyph
    }

    pub fn load_glyph(&self, glyph: &RasterizedGlyph) -> Glyph {
        let mut texture: GLuint = 0;
        unsafe {
            // Create texture object.
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::GenTextures(1, &mut texture);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            // Set texture parameter.
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

            // Parse glyph buffer.
            let (format, buf) = match &glyph.buf {
                BitmapBuffer::RGB(buf) => (gl::RGB, buf),
                BitmapBuffer::RGBA(buf) => (gl::RGBA, buf),
            };

            // Load data into OpenGL.
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

            // All done.
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }

        Glyph {
            texture,
            top: glyph.top as _,
            left: glyph.left as _,
            width: glyph.width as _,
            height: glyph.height as _,
        }
    }
}

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
