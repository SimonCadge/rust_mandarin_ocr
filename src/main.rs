mod ocr;
mod screen_access;
mod supported_languages;
mod positioning_structs;

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    screen_access::screen_entry().await;
}