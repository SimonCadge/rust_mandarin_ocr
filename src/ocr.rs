use core::fmt;

use graphicsmagick::{initialize, types::{FilterTypes}, wand::MagickWand};
use tesseract::{Tesseract, PageSegMode};

#[derive(PartialEq)]
enum SupportedLanguages {
    Eng,
    ChiTra,
}

impl fmt::Display for SupportedLanguages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SupportedLanguages::Eng => write!(f, "eng"),
            SupportedLanguages::ChiTra => write!(f, "chi_tra"),
        }
    }
}

/* 
語言來看我用注音
convert x: -resize 300% -set density 300 \
+dither  -colors 2  -normalize \
$debug png:- | \
tesseract --dpi 300 --psm 6 stdin stdout
https://docs.rs/chinese_dictionary/latest/chinese_dictionary/
*/

pub fn execute_ocr(image: &Vec<u8>) -> String {
    initialize();
    let language = SupportedLanguages::ChiTra;
    
    let mut wand = MagickWand::new();
    
    wand.read_image_blob(&image).unwrap();
    let image_width = wand.get_image_width();
    let image_height = wand.get_image_height();
    wand.resize_image(image_width * 5, image_height * 5, FilterTypes::MitchellFilter, 0.5).unwrap()
        .normalize_image().unwrap();

    wand.set_image_format("PNG").unwrap();
    //TODO: Set dpi
    //TODO: Ensure that background is white and text is black
    //TODO: Remove alpha channel
    // https://github.com/tesseract-ocr/tessdoc/blob/main/ImproveQuality.md#inverting-images

    let mut tesseract = Tesseract::new_with_oem(None, Some(&language.to_string()), 
        tesseract::OcrEngineMode::TesseractLstmCombined).unwrap();
    tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);
    return tesseract.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap()
        .recognize().unwrap()
        .get_hocr_text(0).unwrap();

}