use std::{fs::File, io::Cursor, time::Duration};

use abort_on_drop::ChildTask;
use image::{ImageFormat, imageops::{BiLevel, dither}, EncodableLayout};
use screenshots::Screen;
use tesseract::{Tesseract, PageSegMode};
use tokio::{sync::{watch, mpsc}, task::{yield_now, JoinHandle}, runtime::{self, Runtime}};

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
    let tesseract_runtime = runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap();
    let mut running_job: Option<JoinHandle<String>> = None;
    loop {
        println!("Wake");
        if receiver.has_changed().unwrap() {
            if let Some(handle) = running_job.as_ref() {
                handle.abort();
                handle.await;
                println!("Job Aborted");
            }
            let (x, y, width, height) = *receiver.borrow_and_update();
            running_job = Some(tesseract_runtime.spawn(execute_ocr(x, y, width, height )));
            println!("Job Triggered");
        }
        if running_job.is_some() {
            let handle = running_job.take().unwrap();
            if handle.is_finished() {
                println!("Job Finished");
                let text = handle.await.unwrap();
                sender.send(text).await.unwrap();
            } else {
                running_job = Some(handle);
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn execute_ocr(x: i32, y: i32, width: u32, height: u32) -> String {
    println!("Executing OCR");
    let screen = Screen::from_point(x, y).unwrap();
    let display_position = screen.display_info;
    let image = screen.capture_area(x - display_position.x, y - display_position.y, width, height).unwrap();
    let buffer = image.buffer();

    let image = image::load_from_memory(buffer).unwrap();

    let language = SupportedLanguages::ChiTra;
                
    let image_width = image.width();
    let image_height = image.height();

    let image = image.resize(image_width * 4, image_height * 4, image::imageops::FilterType::CatmullRom);
    let image = image.blur(0.9);
    let color_map = BiLevel;
    let mut image: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = image.to_luma8();
    dither(&mut image, &color_map);

    let mut tesseract = Tesseract::new(None, Some(&language.to_string())).unwrap();
    tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);

    let mut bytes: Vec<u8> = Vec::with_capacity(image.len());
    image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();

    let mut tesseract = tesseract.set_image_from_mem(&bytes).unwrap();

    println!("About to OCR");
    let hocr_text = tesseract.get_hocr_text(0).unwrap();
    println!("OCR Complete");
    return hocr_text;
}

// pub async fn build_ocr_worker(mut receiver: watch::Receiver<(i32, i32, u32, u32)>, sender: mpsc::Sender<String>) {
//     let mut window_position: Option<(i32, i32, u32, u32)> = None;
//     let tesseract_runtime = runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap();
//     loop {
//         tokio::select! {
//             biased;
//             _ = receiver.changed() => {
//                 println!("Receiver Changed");
//                 window_position = Some(*receiver.borrow());
//             }
//             Some(parsed_text) = execute_ocr(window_position, &tesseract_runtime) => {
//                 println!("Parsed Text");
//                 sender.send(parsed_text).await.unwrap();
//                 window_position = None;
//             }
//         }
//     }
// }


// async fn execute_ocr(t: Option<(i32, i32, u32, u32)>, tesseract_runtime: &Runtime) -> Option<String> {
//     match t {
//         Some((x, y, width, height)) => {
//             println!("Executing OCR");
//             let screen = Screen::from_point(x, y).unwrap();
//             let display_position = screen.display_info;
//             let image = screen.capture_area(x - display_position.x, y - display_position.y, width, height).unwrap();
//             let buffer = image.buffer();
//             yield_now().await;

//             let image = image::load_from_memory(buffer).unwrap();
      
//             let language = SupportedLanguages::ChiTra;
                        
//             let image_width = image.width();
//             let image_height = image.height();

//             yield_now().await;

//             let image = image.resize(image_width * 4, image_height * 4, image::imageops::FilterType::CatmullRom);
//             yield_now().await;
//             let image = image.blur(0.9);
//             yield_now().await;
//             let color_map = BiLevel;
//             let mut image: image::ImageBuffer<image::Luma<u8>, Vec<u8>> = image.to_luma8();
//             dither(&mut image, &color_map);

//             yield_now().await;

//             let join_handle: ChildTask<Option<String>> = tesseract_runtime.spawn(async move {
//                 let mut tesseract = Tesseract::new(None, Some(&language.to_string())).unwrap();
//                 tesseract.set_page_seg_mode(PageSegMode::PsmSingleBlock);
    
//                 let mut bytes: Vec<u8> = Vec::with_capacity(image.len());
//                 image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).unwrap();
    
//                 let mut tesseract = tesseract.set_image_from_mem(&bytes).unwrap();
    
//                 println!("About to OCR");
//                 let hocr_text = tesseract.get_hocr_text(0).unwrap();
//                 println!("OCR Complete");
//                 return Some(hocr_text);
//             }).into();

//             while !join_handle.is_finished() {
//                 println!("Not finished yet");
//                 tokio::time::sleep(Duration::from_millis(500)).await;
//                 yield_now().await;
//             }

//             return join_handle.await.unwrap();

//         },
//         None => None,
//     }

// }