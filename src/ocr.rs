use core::fmt;

use clipboard::{ClipboardContext, ClipboardProvider};
use leptess::LepTess;
use graphicsmagick::{initialize, types::{FilterTypes, ColorspaceType}, wand::MagickWand};
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

fn log(wand: &mut MagickWand, leptess: &mut LepTess, stage: &str) {
    wand.write_image(stage.to_owned() + ".png").unwrap();
    leptess.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap();
    println!("{} - \t {}", stage, leptess.get_utf8_text().unwrap());
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

pub fn execute_ocr() {
    initialize();
    let language = SupportedLanguages::ChiTra;
    let mut leptess = LepTess::new(None, &language.to_string()).unwrap();
    leptess.set_variable(leptess::Variable::TesseditPagesegMode, "6").unwrap();
    
    let mut wand = MagickWand::new();

    // wand.read_image("x:").unwrap();
    // log(&mut wand, &mut leptess, "init");

    // wand.negate_image(0).unwrap();
    // log(&mut wand, &mut leptess, "nega");

    // let image_width = wand.get_image_width();
    // let image_height = wand.get_image_height();
    // wand.resize_image(image_width * 5, image_height * 5, FilterTypes::BoxFilter, 0.5).unwrap();
    // log(&mut wand, &mut leptess, "resi");
    
    // wand.quantize_image(2, ColorspaceType::RGBColorspace, 0, 1, 0).unwrap();
    // log(&mut wand, &mut leptess, "quan");
    
    // wand.normalize_image().unwrap();
    // log(&mut wand, &mut leptess, "norm");
    
    // wand.set_image_colorspace(ColorspaceType::GRAYColorspace).unwrap();
    // log(&mut wand, &mut leptess, "gray");

    // wand.sharpen_image(0.0, 1.0).unwrap();
    // log(&mut wand, &mut leptess, "shar");

    wand.read_image("x:").unwrap()
        .negate_image(0).unwrap();
    let image_width = wand.get_image_width();
    let image_height = wand.get_image_height();
    wand.resize_image(image_width * 5, image_height * 5, FilterTypes::BoxFilter, 0.5).unwrap()
        .quantize_image(2, ColorspaceType::RGBColorspace, 0, 1, 0).unwrap()
        .normalize_image().unwrap()
        .set_image_colorspace(ColorspaceType::GRAYColorspace).unwrap()
        .sharpen_image(0.0, 1.0).unwrap();

    wand.set_image_format("PNG").unwrap();

    leptess.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap();

    let final_ocr = leptess.get_utf8_text().unwrap();
    
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(final_ocr.clone()).unwrap();

    match language {
        SupportedLanguages::ChiTra => {
            let stripped_ocr = remove_whitespace(&final_ocr); //Remove all Whitespace
            println!("Final OCR - {}", stripped_ocr);
            let tokenized_text = tokenize(&stripped_ocr);
            println!("{} tokens", tokenized_text.len());
            for token in tokenized_text {
                let results = query(token).unwrap();
                for result in results {
                    println!("{} - {:?}", token, result.english);
                }
            }
        }
        _ => {
            let stripped_ocr = final_ocr.trim();
            println!("Final OCR - {}", stripped_ocr);
        }
    }

}