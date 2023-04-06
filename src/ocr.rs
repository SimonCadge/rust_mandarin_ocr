use core::fmt;
use std::time::Instant;

use clipboard::{ClipboardContext, ClipboardProvider};
use leptess::LepTess;
use graphicsmagick::{initialize, types::{FilterTypes}, wand::MagickWand};
use chinese_dictionary::{tokenize, query};

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

fn remove_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<String>()
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
    let mut start_time = Instant::now();
    initialize();
    let language = SupportedLanguages::Eng;
    let mut leptess = LepTess::new(None, &language.to_string()).unwrap();
    leptess.set_variable(leptess::Variable::TesseditPagesegMode, "6").unwrap();
    
    let mut wand = MagickWand::new();
    
    wand.read_image_blob(&image).unwrap();
    let image_width = wand.get_image_width();
    let image_height = wand.get_image_height();
    wand.resize_image(image_width * 5, image_height * 5, FilterTypes::MitchellFilter, 0.5).unwrap()
        .normalize_image().unwrap();

    wand.set_image_format("PNG").unwrap();

    println!("Processing image took {} ms", start_time.elapsed().as_millis());

    start_time = Instant::now();

    leptess.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap();

    println!("Pass image to leptess took {} ms", start_time.elapsed().as_millis());

    let final_ocr = leptess.get_utf8_text().unwrap();

    println!("OCR took {} ms", start_time.elapsed().as_millis());
    
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(final_ocr.clone()).unwrap();

    match language {
        SupportedLanguages::ChiTra => {
            let stripped_ocr = remove_whitespace(&final_ocr); //Remove all Whitespace
            let tokenized_text = tokenize(&stripped_ocr);
            println!("{} tokens", tokenized_text.len());
            for token in tokenized_text {
                let results = query(token).unwrap();
                for result in results {
                    println!("{} - {:?}", token, result.english);
                }
            }
            return stripped_ocr;
        }
        _ => {
            let stripped_ocr = final_ocr.trim();
            return stripped_ocr.to_owned();
        }
    }

}