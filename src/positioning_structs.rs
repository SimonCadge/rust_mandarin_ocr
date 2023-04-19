use std::{ops::{Sub, Add}, cmp::{min, max}};

use chinese_dictionary::query_by_chinese;
use wgpu_glyph::{ab_glyph::{self, Rect}, Text, FontId, OwnedSection, Section, Layout, OwnedText, GlyphBrush, GlyphCruncher};
use winit::dpi::{PhysicalPosition, Size, PhysicalSize};

use crate::{supported_languages::SupportedLanguages, screen_access::Vertex};

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
        let y = (self.y / (screen_max_point.y / 2.0)) - 1.0;
        [x, y]
    }
}

impl From<(f32, f32)> for PixelPoint {
    fn from(pos: (f32, f32)) -> Self {
        Self { x: pos.0, y: pos.1 }
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

impl PixelArea {
    pub fn new(min: PixelPoint, max: PixelPoint) -> Self {
        Self { min, max }
    }
}

#[derive(Debug, Clone)]
pub struct BboxWord {
    text: String,
    min: PixelPoint,
    max: PixelPoint,
    is_highlighted: bool,
    confidence: f32,
    language: SupportedLanguages,
}

impl Add<&BboxWord> for BboxWord {
    type Output = BboxWord;

    fn add(self, rhs: &BboxWord) -> Self::Output {
        BboxWord {
            text: self.text + &rhs.text,
            min: min(self.min, rhs.min),
            max: max(self.max, rhs.max),
            is_highlighted: self.is_highlighted || rhs.is_highlighted,
            confidence: self.confidence.min(rhs.confidence),
            language: self.language,
        }
    }
}

impl BboxWord {
    pub fn new(text: String, min: PixelPoint, max: PixelPoint, is_highlighted: bool, confidence: f32, language: SupportedLanguages) -> Self {
        Self { 
            text,
            min,
            max,
            is_highlighted,
            confidence,
            language,
        }
    }

    pub fn get_text(&self) -> &String {
        &self.text
    }

    pub fn get_min(&self) -> PixelPoint {
        self.min
    }

    pub fn set_min(&mut self, min: PixelPoint) {
        self.min = min;
    }

    pub fn get_max(&self) -> PixelPoint {
        self.max
    }

    pub fn set_max(&mut self, max: PixelPoint) {
        self.max = max;
    }

    pub fn is_within_bounds(&self, position: &PixelPoint) -> bool {
        let cursor_x: f32 = position.x as f32;
        let cursor_y: f32 = position.y as f32;
        return cursor_x > self.min.x && cursor_x <= self.max.x
            && cursor_y > self.min.y && cursor_y <= self.max.y;
    }

    pub fn is_highlighted(&self) -> bool {
        self.is_highlighted
    }

    pub fn set_highlighted(&mut self, is_highlighted: bool) {
        self.is_highlighted = is_highlighted;
    }

    fn to_text(&self, scale: f32) -> Text {
        return Text::default()
            .with_text(&self.text)
            .with_scale(scale)
            .with_color(self.get_colour())
            .with_font_id(if self.language == SupportedLanguages::Eng {FontId(0)} else {FontId(1)});
    }

    fn get_colour(&self) -> [f32; 4] {
        if self.is_highlighted {
            return [0.0, 1.0, 0.0, 1.0];
        } else if self.confidence < 90.0 {
            return [1.0, 0.0, 0.0, 1.0];
        } else {
            return [0.0, 0.0, 0.0, 1.0];
        }
    }

    pub fn generate_translation_section(&self, glyph_brush: &mut GlyphBrush<()>) -> (OwnedSection, Rect) {
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
                .with_font_id(FontId(1))
                .with_scale(24.0));
        }

        let section = Section::default()
            .to_owned()
            .with_text(translations_as_string);

        let bounds = glyph_brush.glyph_bounds(&section).unwrap();

        (section, bounds)
    }
}

pub struct BboxLine {
    words: Vec<BboxWord>,
}

impl BboxLine {
    pub fn new(words: Vec<BboxWord>) -> Self {
        return Self {
            words,
        }
    }

    pub fn get_words(&self) -> &Vec<BboxWord> {
        &self.words
    }
    
    pub fn get_mut_words(&mut self) -> &mut Vec<BboxWord> {
        &mut self.words
    }

    pub fn get_min(&self) -> PixelPoint {
        let smallest_x = self.words.iter().map(|word| word.min.x).min_by(|a, b| a.total_cmp(b)).unwrap();
        let smallest_y = self.words.iter().map(|word| word.min.y).min_by(|a, b| a.total_cmp(b)).unwrap();
        return PixelPoint::new(smallest_x, smallest_y);
    }

    pub fn get_max(&self) -> PixelPoint {
        let largest_x = self.words.iter().map(|word| word.max.x).max_by(|a, b| a.total_cmp(b)).unwrap();
        let largest_y = self.words.iter().map(|word| word.max.y).max_by(|a, b| a.total_cmp(b)).unwrap();
        return PixelPoint::new(largest_x, largest_y);
    }

    pub fn get_scale(&self) -> f32 {
        return self.get_max().y - self.get_min().y;
    }

    pub fn to_section(&self) -> OwnedSection {
        let text = self.words.iter().map(|word| word.to_text(self.get_scale())).collect();
        return Section::default()
            .with_screen_position((self.get_min().x, self.get_min().y))
            .with_layout(Layout::default())
            .with_text(text)
            .to_owned();
    }

    pub fn to_vertices(&self, screen_max_point: PixelPoint, offset: u32) -> (Vec<Vertex>, Vec<u32>) {
        let min = self.get_min();
        let max = self.get_max();
        let verticies = vec![
            Vertex { //top left
                position: min.to_normalized_coordinate(screen_max_point),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //top right
                position: PixelPoint::new(max.x, min.y).to_normalized_coordinate(screen_max_point),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //bottom left
                position: PixelPoint::new(min.x, max.y).to_normalized_coordinate(screen_max_point),
                color: [1.0, 1.0, 1.0],
            },
            Vertex { //bottom right
                position: max.to_normalized_coordinate(screen_max_point),
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