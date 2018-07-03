extern crate font_rs;
extern crate fuchsia_async as async;
extern crate fuchsia_framebuffer;
extern crate fuchsia_zircon;

mod text;

use fuchsia_framebuffer::{FrameBuffer, PixelFormat};
use std::io::{self, Read};
use std::{thread, time};
use text::Face;

static FONT_DATA: &'static [u8] =
    include_bytes!("../../../fonts/third_party/robotoslab/RobotoSlab-Regular.ttf");

/// Convenience function that can be called from main and causes the Fuchsia process being
/// run over ssh to be terminated when the user hits control-C.
fn wait_for_close() {
    thread::spawn(move || loop {
        let mut input = [0; 1];
        if io::stdin().read_exact(&mut input).is_err() {
            std::process::exit(0);
        }
    });
}

fn main() {
    println!("Recovery UI");
    wait_for_close();

    let mut face = Face::new(FONT_DATA).unwrap();

    let mut executor = async::Executor::new().unwrap();

    let fb = FrameBuffer::new(None, &mut executor).unwrap();
    let config = fb.get_config();

    let values565 = &[31, 248];
    let values8888 = &[255, 0, 255, 255];

    let mut pink_frame = fb.new_frame(&mut executor).unwrap();

    for y in 0..config.height {
        for x in 0..config.width {
            match config.format {
                PixelFormat::RgbX888 => pink_frame.write_pixel(x, y, values8888),
                PixelFormat::Argb8888 => pink_frame.write_pixel(x, y, values8888),
                PixelFormat::Rgb565 => pink_frame.write_pixel(x, y, values565),
                _ => {}
            }
        }
    }

    face.draw_text_at(
        &mut pink_frame,
        (config.width / 2) as i32,
        (config.height / 2) as i32,
        256,
        "Hello",
    );

    pink_frame.present(&fb).unwrap();
    loop {
        thread::sleep(time::Duration::from_millis(25000));
    }
}
