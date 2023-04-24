use std::{ops::{Sub, Add}, cmp::{min, max}};

use chinese_dictionary::query_by_chinese;
use wgpu_glyph::{FontId, ab_glyph::{self, Rect, PxScale}, OwnedSection, Section, OwnedText, GlyphBrush, GlyphCruncher};
use winit::dpi::{PhysicalPosition, Size, PhysicalSize};

use crate::screen_access::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PixelPoint {
    x: f32,
    y: f32,
}

impl PixelPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y
        }
    }

    pub fn get_x(&self) -> f32 {
        self.x
    }
    
    pub fn get_y(&self) -> f32 {
        self.y
    }

    pub fn to_normalized_coordinate(self, screen_max_point: PixelPoint) -> [f32; 2] {
        let x = (self.x / (screen_max_point.x / 2.0)) - 1.0;
        let y = 1.0 - (self.y / (screen_max_point.y / 2.0));
        [x, y]
    }
}

impl From<(f32, f32)> for PixelPoint {
    fn from(pos: (f32, f32)) -> Self {
        Self { x: pos.0, y: pos.1 }
    }
}

impl Into<(f32, f32)> for PixelPoint {
    fn into(self) -> (f32, f32) {
        (self.x, self.y)
    }
}

impl From<ab_glyph::Point> for PixelPoint {
    fn from(pos: ab_glyph::Point) -> Self {
        Self {
            x: pos.x,
            y: pos.y
        }
    }
}

impl From<&PhysicalPosition<f64>> for PixelPoint {
    fn from(pos: &PhysicalPosition<f64>) -> Self {
        Self {
            x: pos.x as f32,
            y: pos.y as f32,
        }
    }
}

impl Sub<PixelPoint> for PixelPoint {
    type Output = PixelPoint;

    fn sub(self, rhs: PixelPoint) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y
        }
    }
}

impl Ord for PixelPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let combined_self = self.x + self.y;
        let combined_other = other.x + other.y;
        return combined_self.total_cmp(&combined_other);
    }
}

impl Eq for PixelPoint {}

pub struct PixelArea {
    min: PixelPoint,
    max: PixelPoint,
}

impl Into<Size> for PixelArea {
    fn into(self) -> Size {
        let difference = self.max - self.min;
        PhysicalSize::new(difference.x, difference.y).into()
    }
}

#[derive(Debug, Clone)]
pub struct HocrWord {
    text: String,
    min: PixelPoint,
    max: PixelPoint,
    confidence: f32,
}

impl Add<&HocrWord> for HocrWord {
    type Output = HocrWord;

    fn add(self, rhs: &HocrWord) -> Self::Output {
        HocrWord {
            text: self.text + &rhs.text,
            min: min(self.min, rhs.min),
            max: max(self.max, rhs.max),
            confidence: self.confidence.min(rhs.confidence),
        }
    }
}

impl HocrWord {
    pub fn new(text: String, min: PixelPoint, max: PixelPoint, confidence: f32) -> Self {
        Self { 
            text,
            min,
            max,
            confidence,
        }
    }

    pub fn get_text(&self) -> &String {
        &self.text
    }

    pub fn get_min(&self) -> PixelPoint {
        self.min
    }

    fn get_scale(&self) -> f32 {
        self.max.y - self.min.y
    }
}

#[derive(Debug, Clone)]
pub struct PresentableWord {
    text: String,
    min: PixelPoint,
    confidence: f32,
    is_highlighted: bool,
}

impl PresentableWord {
    pub fn new(text: String, min: PixelPoint, confidence: f32) -> Self {
        Self { 
            text,
            min,
            confidence,
            is_highlighted: false
        }
    }

    pub fn get_min(&self) -> PixelPoint {
        self.min
    }

    pub fn is_within_bounds(&self, position: &PixelPoint, scale: PxScale) -> bool {
        let cursor_x: f32 = position.x as f32;
        let cursor_y: f32 = position.y as f32;
        return cursor_x > self.min.x && cursor_x <= self.min.x + (scale.x * self.text.chars().count() as f32)
            && cursor_y > self.min.y && cursor_y <= self.min.y + scale.y as f32;
    }

    pub fn is_highlighted(&self) -> bool {
        self.is_highlighted
    }

    pub fn set_highlighted(&mut self, is_highlighted: bool) -> bool {
        let was_highlighted = self.is_highlighted;
        self.is_highlighted = is_highlighted;
        return was_highlighted != is_highlighted; //return true if value has changed
    }

    fn to_text(&self, scale: PxScale) -> OwnedText {
        return OwnedText::default()
            .with_text(&self.text)
            .with_scale(scale)
            .with_color(self.get_colour())
            .with_font_id(FontId(0));
    }

    fn get_colour(&self) -> [f32; 4] {
        if self.is_highlighted {
            return [0.0, 1.0, 0.0, 1.0]; //green
        } else if self.confidence < 90.0 {
            return [1.0, 0.0, 0.0, 1.0]; //red
        } else {
            return [0.0, 0.0, 0.0, 1.0]; //black
        }
    }

    pub fn generate_translation_section(&self, glyph_brush: &mut GlyphBrush<()>) -> (OwnedSection, Option<Rect>) {
        let translations = query_by_chinese(&self.text);
        let mut translations_as_string = Vec::with_capacity(translations.len());
        for translation in translations {
            let mut translation_as_string = "".to_owned();
            translation_as_string.push_str(&translation.traditional);
            translation_as_string.push_str("(");
            translation_as_string.push_str(&translation.pinyin_marks);
            translation_as_string.push_str("): \t");
            translation_as_string.push_str(&translation.english.join("\n          "));
            translation_as_string.push_str("\n");
            translations_as_string.push(OwnedText::new(&translation_as_string)
                .with_scale(24.0));
        }

        let section = Section::default()
            .to_owned()
            .with_text(translations_as_string);

        let bounds = glyph_brush.glyph_bounds(&section);

        (section, bounds)
    }
}

pub struct PresentableLine {
    words: Vec<PresentableWord>,
    section: OwnedSection,
    min: PixelPoint,
    max: PixelPoint,
    scale: PxScale,
}

impl PresentableLine {
    pub fn from_hocr(hocr_words: Vec<HocrWord>, glyph_brush: &mut GlyphBrush<()>) -> Self {
        let scale = PxScale::from(hocr_words.iter()
            .filter(|word| !word.text.starts_with(|char: char| char.is_ascii_punctuation()))
            .map(|word| word.get_scale())
            .sum::<f32>() / hocr_words.len() as f32); //average scale of non-punctuation characters
        let min = hocr_words[0].get_min();
        let mut presentable_words = Vec::with_capacity(hocr_words.len());
        let mut accumulated_text = Vec::with_capacity(hocr_words.len());
        let mut offset = min;
        for hocr_word in hocr_words {
            let presentable_word = PresentableWord::new(hocr_word.text, offset, hocr_word.confidence);
            let text = presentable_word.clone().to_text(scale);
            presentable_words.push(presentable_word);
            let word_bounds = glyph_brush.glyph_bounds(&OwnedSection::<()>::default().with_text(vec![text.clone()]).with_screen_position(offset)).unwrap();
            accumulated_text.push(text);
            offset = PixelPoint::new(word_bounds.max.x, word_bounds.min.y);
        }
        let section = OwnedSection::<()>::default()
                .with_screen_position(min)
                .with_text(accumulated_text);

        let line_bounds = glyph_brush.glyph_bounds(&section).unwrap();
        let max: PixelPoint = PixelPoint::from(line_bounds.max);

        return Self {
            words: presentable_words,
            section,
            min,
            max,
            scale,
        }
    }

    pub fn get_words(&self) -> &Vec<PresentableWord> {
        &self.words
    }

    fn get_mut_words(&mut self) -> &mut Vec<PresentableWord> {
        &mut self.words
    }

    pub fn handle_cursor(&mut self, cursor_position: &PixelPoint) {
        let scale = self.scale;
        let mut is_changed = false;
        for word in self.get_mut_words() {
            if word.is_within_bounds(cursor_position, scale) {
                is_changed = word.set_highlighted(true) || is_changed;
            } else {
                is_changed = word.set_highlighted(false) || is_changed;
            }
        }
        if is_changed {
            let text = self.words.iter().map(|word| word.to_text(scale)).collect();
            self.section = OwnedSection::<()>::default()
                .with_screen_position(self.min)
                .with_text(text);
        }
    }

    pub fn get_min(&self) -> PixelPoint {
        self.min
    }
    
    pub fn get_max(&self) -> PixelPoint {
        self.max
    }

    pub fn get_scale(&self) -> PxScale {
        self.scale
    }

    pub fn get_section(&self) -> &OwnedSection {
        &self.section
    }

    pub fn generate_bounding_vertices(&self, screen_max_point: PixelPoint, offset: u32) -> (Vec<Vertex>, Vec<u32>) {
        let min = self.get_min().to_normalized_coordinate(screen_max_point);
        let max = self.get_max().to_normalized_coordinate(screen_max_point);

        let verticies = vec![
            Vertex { //top left
                position: min.clone(),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //top right
                position: [max[0], min[1]].clone(),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //bottom left
                position: [min[0], max[1]].clone(),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //bottom right
                position: max.clone(),
                color: [1.0, 1.0, 1.0],
            },
        ];
        let indices = vec![
            offset + 0, offset + 1, offset + 2,
            offset + 2, offset + 1, offset + 3
        ];
        return (verticies, indices);
    }
}