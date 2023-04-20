use std::sync::mpsc::{Receiver, Sender};

use graphicsmagick::{initialize, types::{FilterTypes}, wand::{MagickWand, PixelWand}};
use screenshots::Screen;
use tesseract::{Tesseract, PageSegMode};

use crate::{supported_languages::SupportedLanguages};

/* 
語言來看我用注音
convert x: -resize 300% -set density 300 \
+dither  -colors 2  -normalize \
$debug png:- | \
tesseract --dpi 300 --psm 6 stdin stdout
https://docs.rs/chinese_dictionary/latest/chinese_dictionary/
*/

pub fn build_ocr_worker(receiver: Receiver<(i32, i32, u32, u32)>, sender: Sender<String>) {
    loop {
        if let Ok((mut x, mut y, mut width, mut height)) = receiver.recv() {
            let mut reached_end = false;
            while !reached_end {
                if let Ok((new_x, new_y, new_width, new_height)) = receiver.try_recv() { //Exhaust 
                    x = new_x;
                    y = new_y;
                    width = new_width;
                    height = new_height;
                } else {
                    reached_end = true;
                }
            }
            println!("Received Job");
            let parsed_string = execute_ocr(x, y, width, height);
            println!("Finished Job");
            sender.send(parsed_string).unwrap();
        };
    }
}

fn execute_ocr(x: i32, y: i32, width: u32, height: u32) -> String {
    let screen = Screen::from_point(x, y).unwrap();
    let display_position = screen.display_info;
    let image = screen.capture_area(x - display_position.x, y - display_position.y, width, height).unwrap();
    let buffer = image.buffer();

    initialize();
    let language = SupportedLanguages::ChiTra;
    
    let mut wand = MagickWand::new();
    
    wand.read_image_blob(&buffer).unwrap();
    let image_width = wand.get_image_width();
    let image_height = wand.get_image_height();
    let mut black = PixelWand::new();
    black.set_color("black");
    wand.resize_image(image_width * 4, image_height * 4, FilterTypes::MitchellFilter, 0.9).unwrap()
        .normalize_image().unwrap()
        .quantize_image(2, graphicsmagick::types::ColorspaceType::GRAYColorspace, 0, 1, 0).unwrap();

    wand.set_image_format("PNG").unwrap();
    wand.write_image("input_image.PNG").unwrap();
    //TODO: Set dpi
    //TODO: Ensure that background is white and text is black
    //TODO: Remove alpha channel
    // https://github.com/tesseract-ocr/tessdoc/blob/main/ImproveQuality.md#inverting-images

    let mut tesseract = Tesseract::new(None, Some(&language.to_string())).unwrap();
    tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);
    // let tesseract = tesseract.set_variable("user_defined_dpi", "300").unwrap();
    return tesseract.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap()
        .recognize().unwrap()
        .get_hocr_text(0).unwrap();

}