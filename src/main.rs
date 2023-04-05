mod ocr;
mod screen_access;

fn main() {
    pollster::block_on(screen_access::screen_entry());
}