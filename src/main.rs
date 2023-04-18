use pollster::FutureExt;

mod ocr;
mod screen_access;
mod supported_languages;
mod positioning_structs;

fn main() {
    screen_access::screen_entry().block_on();
}