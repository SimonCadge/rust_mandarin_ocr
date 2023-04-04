use core::{time};
use std::thread;

use fltk::{app, button::Button, frame::Frame, prelude::*, window::Window, draw, image::PngImage, enums::{Color, FrameType}};
use graphicsmagick::{initialize, types::{FilterTypes, ColorspaceType}, wand::MagickWand};

pub fn screen_entry() {
    let app = app::App::default();
    let mut wind = Window::new(100, 100, 400, 300, "Hello from rust");
    wind.set_frame(FrameType::NoBox);
    let mut frame = Frame::new(0, 0, 400, 200, "");
    frame.set_frame(FrameType::NoBox);
    let mut but = Button::new(160, 210, 80, 40, "Click me!");
    wind.end();
    wind.show();
    but.set_callback(move |_| {
        let image = draw::capture_window(&mut wind).unwrap();
        let binding = image.to_rgb_data();
        let binding = &binding[..];
        // println!("blob: {:?}", binding);
        initialize();
        let mut wand = MagickWand::new();
        wand.set_format("RGB").unwrap();
        println!("Width - {} Height - {}", image.width(), image.height());
        println!("Length of array - {}", binding.len());
        let columns: u64 = (image.height()).try_into().unwrap();
        let rows: u64 = (image.width()).try_into().unwrap();
        wand.set_size(rows / 2, columns / 2).unwrap();
        wand.read_image_blob(binding).unwrap();
        wand.write_image("screen.png").unwrap();
        println!("Printed");
    }); // the closure capture is mutable borrow to our button
    app.run().unwrap();

    
    

}

// https://gtk-rs.org/gtk4-rs/stable/latest/book/g_object_memory_management.html
// https://gitlab.gnome.org/GNOME/gnome-screenshot/-/tree/master/