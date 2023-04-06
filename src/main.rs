use pollster::FutureExt;

mod ocr;
mod screen_access;

fn main() {
    screen_access::screen_entry().block_on();
}