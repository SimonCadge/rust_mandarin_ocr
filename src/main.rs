use clipboard::{ClipboardContext, ClipboardProvider};
use leptess::LepTess;
use graphicsmagick::{initialize, types::{FilterTypes, ColorspaceType}, wand::MagickWand};

fn log(wand: &mut MagickWand, leptess: &mut LepTess, stage: &str) {
    wand.write_image(stage.to_owned() + ".png").unwrap();
    leptess.set_image_from_mem(&wand.write_image_blob().unwrap()).unwrap();
    println!("{} - \t {}", stage, leptess.get_utf8_text().unwrap());
}

fn main() {
    initialize();
    let mut leptess = LepTess::new(None, "chi_tra").unwrap();
    leptess.set_variable(leptess::Variable::TesseditPagesegMode, "6").unwrap();
    
    let mut wand = MagickWand::new();

    wand.read_image("x:").unwrap();
    log(&mut wand, &mut leptess, "init");

    wand.negate_image(0).unwrap();
    log(&mut wand, &mut leptess, "nega");

    let image_width = wand.get_image_width();
    let image_height = wand.get_image_height();
    wand.resize_image(image_width * 5, image_height * 5, FilterTypes::BoxFilter, 0.5).unwrap();
    log(&mut wand, &mut leptess, "resi");
    
    wand.quantize_image(2, ColorspaceType::RGBColorspace, 0, 1, 0).unwrap();
    log(&mut wand, &mut leptess, "quan");
    
    wand.normalize_image().unwrap();
    log(&mut wand, &mut leptess, "norm");
    
    wand.set_image_colorspace(ColorspaceType::GRAYColorspace).unwrap();
    log(&mut wand, &mut leptess, "gray");

    wand.sharpen_image(0.0, 1.0).unwrap();
    log(&mut wand, &mut leptess, "shar");
    
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
    ctx.set_contents(leptess.get_utf8_text().unwrap()).unwrap();

}

/* 
語言來看我用注音
convert x: -resize 300% -set density 300 \
+dither  -colors 2  -normalize \
$debug png:- | \
tesseract --dpi 300 --psm 6 stdin stdout
*/