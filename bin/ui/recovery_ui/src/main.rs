extern crate failure;
extern crate fidl_fuchsia_amber as amber;
extern crate fuchsia_app as app;
extern crate fuchsia_async as async;
extern crate fuchsia_framebuffer;
extern crate fuchsia_zircon;

use app::client::connect_to_service;
use async::futures::FutureExt;
use failure::Error;
use fuchsia_framebuffer::{FrameBuffer, PixelFormat};
use std::io::{self, Read};
use std::{thread, time};

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

fn main() -> Result<(), Error> {
    println!("Recovery UI");
    wait_for_close();

    let mut executor = async::Executor::new().unwrap();

    let fb = FrameBuffer::new(None, &mut executor).unwrap();
    let config = fb.get_config();

    let values565 = &[31, 248];
    let values8888 = &[255, 0, 255, 255];

    let pink_frame = fb.new_frame(&mut executor)?;

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

    pink_frame.present(&fb).unwrap();

    let amber_control = connect_to_service::<amber::ControlMarker>().unwrap();
    let srcs = amber_control.list_srcs();

    executor.run_singlethreaded(srcs)?;

    loop {
        thread::sleep(time::Duration::from_millis(250));
    }
}
