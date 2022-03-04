use {
    crossfont::{
        FontDesc, FontKey, GlyphKey, Rasterize, RasterizedGlyph, Rasterizer, Size, Slant, Style,
        Weight,
    },
    log::error,
    std::default::Default,
};

/// Description of the normal font.
#[derive(Debug, Default, Clone)]
pub struct FontDescription {
    pub family: String,
    pub style: Option<String>,
}

/// Description of the font.
pub struct Font {
    pub normal: FontDescription,
    pub size: Size,
    pub dpr: f32,

    /// Rasterizer for loading new glyphs.
    rasterizer: Rasterizer,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            normal: FontDescription {
                family: String::from("Source Han Mono"),
                style: Some(String::from("Regular")),
            },
            size: Size::new(14.0),
            dpr: 1.0,
            rasterizer: Rasterizer::new(1.0, true).unwrap(),
        }
    }
}

impl Font {
    pub fn new(dpr: f32, family: &str, size: i32) -> Self {
        Self {
            normal: FontDescription {
                family: String::from(family),
                style: Some(String::from("Regular")),
            },
            size: Size::new(size as _),
            dpr,
            rasterizer: Rasterizer::new(dpr, true).unwrap(),
        }
    }

    pub fn make_desc(desc: &FontDescription, slant: Slant, weight: Weight) -> FontDesc {
        let style = if let Some(ref spec) = desc.style {
            Style::Specific(spec.to_owned())
        } else {
            Style::Description { slant, weight }
        };
        FontDesc::new(desc.family.clone(), style)
    }

    pub fn compute_font_keys(&mut self) -> Result<FontKey, crossfont::Error> {
        let size = self.size;

        let regular_desc = Self::make_desc(&self.normal, Slant::Normal, Weight::Normal);
        let regular = self.load_regular_font(&regular_desc, size)?;

        Ok(regular)
    }

    pub fn load_regular_font(
        &mut self,
        description: &FontDesc,
        size: Size,
    ) -> Result<FontKey, crossfont::Error> {
        self.rasterizer.load_font(description, size).or_else(|e| {
            error!("load font {} error: {}", description, e);
            let fallback_desc =
                Self::make_desc(&Font::default().normal, Slant::Normal, Weight::Normal);
            self.rasterizer.load_font(&fallback_desc, size)
        })
    }

    /// Calculate font metrics without access to a glyph cache.
    pub fn metrics(&mut self) -> Result<crossfont::Metrics, crossfont::Error> {
        let regular_desc = Self::make_desc(&self.normal, Slant::Normal, Weight::Normal);
        let regular = self.load_regular_font(&regular_desc, self.size)?;
        self.rasterizer.metrics(regular, self.size)
    }

    pub fn get_glyph(&mut self, glyph_key: GlyphKey) -> Result<RasterizedGlyph, crossfont::Error> {
        self.rasterizer.get_glyph(glyph_key)
    }
}
