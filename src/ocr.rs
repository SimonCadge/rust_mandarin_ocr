use std::io::Cursor;


use abort_on_drop::ChildTask;
use image::{ImageFormat, imageops::{BiLevel, dither}};
use screenshots::Screen;
use tesseract::{Tesseract, PageSegMode};
use tokio::{sync::{watch, mpsc}, task::yield_now};

use crate::supported_languages::SupportedLanguages;

#[tokio::main]
pub async fn build_ocr_worker(mut receiver: watch::Receiver<(i32, i32, u32, u32)>, sender: mpsc::Sender<String>, language: SupportedLanguages) {
    let mut window_position: Option<(i32, i32, u32, u32)> = None;
    loop {
        tokio::select! {
            biased;
            _ = receiver.changed() => {
                window_position = Some(*receiver.borrow());
            }
            Ok(Some(parsed_text)) = ChildTask::from(tokio::spawn(execute_ocr(window_position, language))) => {
                sender.send(parsed_text).await.unwrap();
                window_position = None;
            }
        }
    }
}


async fn execute_ocr(t: Option<(i32, i32, u32, u32)>, language: SupportedLanguages) -> Option<String> {
    match t {
        Some((x, y, width, height)) => {
            let screen = Screen::from_point(x, y).unwrap();
            let display_position = screen.display_info;
            let image = screen.capture_area(x - display_position.x, y - display_position.y, width, height).unwrap();
            let buffer = image.buffer();
            yield_now().await;

            let image = image::load_from_memory(buffer).unwrap();
                        
            let image_width = image.width();
            let image_height = image.height();

            yield_now().await;

            let image = image.resize(image_width * 4, image_height * 4, image::imageops::FilterType::CatmullRom);
            yield_now().await;
            let image = image.blur(0.9);
            yield_now().await;
            let color_map = BiLevel;
            let mut image: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = image.to_luma8();
            dither(&mut image, &color_map);

            yield_now().await;

            let mut tesseract = Tesseract::new(None, Some(&language.to_string())).unwrap();
            tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);

            let mut bytes: Vec<u8> = Vec::with_capacity(image.len());
            image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();

            let mut tesseract = tesseract.set_image_from_mem(&bytes).unwrap();

            yield_now().await;

            let hocr_text = tesseract.get_hocr_text(0).unwrap();
            return Some(hocr_text);

        },
        None => None,
    }

}