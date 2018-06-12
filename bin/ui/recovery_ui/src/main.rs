extern crate fuchsia_async as async;
extern crate fuchsia_framebuffer;
extern crate fuchsia_zircon;
extern crate image;

use fuchsia_framebuffer::{Config, Frame, FrameBuffer, PixelFormat};
use image::{load_from_memory_with_format, ImageFormat, RgbaImage};
use std::io::{self, Read};
use std::{thread, time};

static LABEL_DATA: &'static [u8] = include_bytes!("../resources/recovery_mode.png");

/// Convenience function that can be called from main and causes the Fuchsia process being
/// run over ssh to be terminated when the user hits control-C.
fn wait_for_close() {
    thread::spawn(move || loop {
        let mut input = [0; 1];
        match io::stdin().read_exact(&mut input) {
            Ok(()) => {}
            Err(_) => std::process::exit(0),
        }
    });
}

fn draw_image(config: &Config, frame: &Frame, image: &RgbaImage, x: u32, y: u32) {
    let mut pixels = image.pixels();
    let values565 = &[255, 255];
    let values8888 = &[255, 255, 255, 255];
    for j in y..y + image.height() {
        for i in x..x + image.width() {
            let pixel_value = pixels.next().unwrap();
            if pixel_value[3] > 0 {
                match config.format {
                    PixelFormat::RgbX888 => frame.write_pixel(i, j, values8888),
                    PixelFormat::Argb8888 => frame.write_pixel(i, j, values8888),
                    PixelFormat::Rgb565 => frame.write_pixel(i, j, values565),
                    _ => {}
                }
            }
        }
    }
}

fn main() {
    println!("Recovery UI");
    wait_for_close();

    let mut executor = async::Executor::new().unwrap();

    let fb = FrameBuffer::new(&mut executor).unwrap();
    let config = fb.get_config();

    let values565 = &[31, 248];
    let values8888 = &[255, 0, 255, 255];

    let pink_frame = fb.new_frame(&mut executor).unwrap();

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

    let image = load_from_memory_with_format(LABEL_DATA, ImageFormat::PNG)
        .unwrap()
        .to_rgba();
    let x = config.width / 2 - image.width() / 2;
    let y = config.height / 2 - image.height() / 2;
    draw_image(&config, &pink_frame, &image, x, y);

    pink_frame.present(&fb).unwrap();
    loop {
        thread::sleep(time::Duration::from_millis(25000));
    }
}
