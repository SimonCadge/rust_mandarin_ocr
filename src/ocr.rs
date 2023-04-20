use std::{fs::File, io::Cursor};

use image::{ImageFormat, imageops::{BiLevel, dither}, EncodableLayout};
use screenshots::Screen;
use tesseract::{Tesseract, PageSegMode};
use tokio::{sync::{watch, mpsc}, task::yield_now};

use crate::{supported_languages::SupportedLanguages};

/* 
語言來看我用注音
convert x: -resize 300% -set density 300 \
+dither  -colors 2  -normalize \
$debug png:- | \
tesseract --dpi 300 --psm 6 stdin stdout
https://docs.rs/chinese_dictionary/latest/chinese_dictionary/
*/

pub async fn build_ocr_worker(mut receiver: watch::Receiver<(i32, i32, u32, u32)>, sender: mpsc::Sender<String>) {
    let mut window_position: Option<(i32, i32, u32, u32)> = None;
    loop {
        tokio::select! {
            biased;
            _ = receiver.changed() => {
                println!("Receiver Changed");
                window_position = Some(*receiver.borrow());
            }
            Some(parsed_text) = execute_ocr(window_position) => {
                println!("Parsed Text");
                sender.send(parsed_text).await.unwrap();
                window_position = None;
            }
        }
    }
}

async fn execute_ocr(t: Option<(i32, i32, u32, u32)>) -> Option<String> {
    match t {
        Some((x, y, width, height)) => {
            println!("Executing OCR");
            let screen = Screen::from_point(x, y).unwrap();
            let display_position = screen.display_info;
            let image = screen.capture_area(x - display_position.x, y - display_position.y, width, height).unwrap();
            let buffer = image.buffer();
            yield_now().await;

            let image = image::load_from_memory(buffer).unwrap();
      
            let language = SupportedLanguages::ChiTra;
                        
            let image_width = image.width();
            let image_height = image.height();

            yield_now().await;

            let image = image.resize(image_width * 4, image_height * 4, image::imageops::FilterType::Gaussian);
            yield_now().await;
            // let image = image.blur(0.9);
            // yield_now().await;
            let color_map = BiLevel;
            let mut image: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = image.to_luma8();
            yield_now().await;
            dither(&mut image, &color_map);

            yield_now().await;

            let mut tesseract = Tesseract::new(None, Some(&language.to_string())).unwrap();
            tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);
            // let tesseract = tesseract.set_variable("user_defined_dpi", "300").unwrap();

            let mut bytes: Vec<u8> = Vec::with_capacity(image.len());
            image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();

            println!("OCR Complete");
            return Some(tesseract.set_image_from_mem(&bytes).unwrap()
                .recognize().unwrap()
                .get_hocr_text(0).unwrap());
        },
        None => None,
    }

}