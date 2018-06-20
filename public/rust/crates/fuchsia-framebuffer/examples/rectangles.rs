// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate failure;
extern crate fuchsia_async as async;
extern crate fuchsia_framebuffer;

use failure::Error;
use fuchsia_framebuffer::{Frame, FrameBuffer};
use std::io::{self, Read};
use std::{thread, time};

/// Convenience function that can be called from main and causes the Fuchsia process being
/// run over ssh to be terminated when the user hits control-C.
pub fn wait_for_close() {
    thread::spawn(move || loop {
        let mut input = [0; 1];
        match io::stdin().read_exact(&mut input) {
            Ok(()) => {}
            Err(_) => std::process::exit(0),
        }
    });
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

impl Size {
    pub fn add(&self, size: Size) -> Size {
        Size {
            width: self.width + size.width,
            height: self.height + size.height,
        }
    }

    pub fn subtract(&self, size: Size) -> Size {
        Size {
            width: self.width - size.width,
            height: self.height - size.height,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn add(&self, pt: Point) -> Point {
        Point {
            x: self.x + pt.x,
            y: self.y + pt.y,
        }
    }

    pub fn subtract(&self, pt: Point) -> Point {
        Point {
            x: self.x - pt.x,
            y: self.y - pt.y,
        }
    }

    pub fn to_size(&self) -> Size {
        Size {
            width: self.x,
            height: self.y,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Rectangle {
    pub origin: Point,
    pub size: Size,
}

impl Rectangle {
    pub fn empty(&self) -> bool {
        self.size.width <= 0 && self.size.height <= 0
    }

    pub fn bottom(&self) -> i32 {
        self.origin.y + self.size.height
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Color {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Color {
    pub fn new() -> Color {
        Color {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }

    pub fn extract_hex_slice(hash_code: &str, start_index: usize) -> u8 {
        u8::from_str_radix(&hash_code[start_index..start_index + 2], 16).unwrap()
    }

    pub fn convert_to_float_color_component(component: u8) -> f64 {
        (f64::from(component) * 100.0 / 255.0).round() / 100.0
    }

    pub fn from_hash_code(hash_code: &str) -> Color {
        let mut new_color = Color::new();
        new_color.red =
            Color::convert_to_float_color_component(Color::extract_hex_slice(hash_code, 1));
        new_color.green =
            Color::convert_to_float_color_component(Color::extract_hex_slice(hash_code, 3));
        new_color.blue =
            Color::convert_to_float_color_component(Color::extract_hex_slice(hash_code, 5));
        new_color
    }

    pub fn to_565(&self) -> [u8; 2] {
        let five_bit_mask = 0b1_1111;
        let six_bit_mask = 0b11_1111;
        let red: u8 = (f64::from(five_bit_mask) * self.red) as u8;
        let green: u8 = (f64::from(six_bit_mask) * self.green) as u8;
        let blue: u8 = (f64::from(five_bit_mask) * self.blue) as u8;
        let b1 = (red << 3) | ((green & 0b11_1000) >> 3);
        let b2 = ((green & 0b111) << 5) | blue;
        [b2, b1]
    }

    pub fn to_8888(&self) -> [u8; 4] {
        let red: u8 = (255.0 * self.red) as u8;
        let green: u8 = (255.0 * self.green) as u8;
        let blue: u8 = (255.0 * self.blue) as u8;
        let alpha: u8 = (255.0 * self.alpha) as u8;
        [blue, green, red, alpha]
    }

    pub fn scale(&self, amount: f64) -> Color {
        Color {
            red: self.red * amount,
            green: self.green * amount,
            blue: self.blue * amount,
            alpha: self.alpha * amount,
        }
    }
}

fn fill_rectangle(frame: &mut Frame, color: &Color, r: &Rectangle) {
    frame.fill_rectangle(
        r.origin.x as u32,
        r.origin.y as u32,
        r.size.width as u32,
        r.size.height as u32,
        &color.to_8888(),
    );
}

fn run() -> Result<(), Error> {
    wait_for_close();

    let mut executor = async::Executor::new().unwrap();

    let mut fb = FrameBuffer::new(None, &mut executor)?;
    let mut frame = fb.new_frame(&mut executor)?;
    let c1 = Color::from_hash_code("#D0D0D0");
    let c2 = Color::from_hash_code("#FFCC66");
    let c3 = Color::from_hash_code("#00FFFF");
    let fuchsia = Color::from_hash_code("#FF00FF");
    let r1 = Rectangle {
        origin: Point { x: 200, y: 200 },
        size: Size {
            width: 200,
            height: 200,
        },
    };
    let r2 = Rectangle {
        origin: Point { x: 500, y: 100 },
        size: Size {
            width: 100,
            height: 100,
        },
    };
    let r3 = Rectangle {
        origin: Point { x: 300, y: 500 },
        size: Size {
            width: 300,
            height: 100,
        },
    };
    let frame_bounds = Rectangle {
        origin: Point { x: 0, y: 0 },
        size: Size {
            width: frame.get_width() as i32,
            height: frame.get_height() as i32,
        },
    };
    let mut i: usize = 0;
    loop {
        fill_rectangle(&mut frame, &fuchsia, &frame_bounds);
        match i % 3 {
            0 => {
                fill_rectangle(&mut frame, &c1, &r1);
                fill_rectangle(&mut frame, &c2, &r2);
                fill_rectangle(&mut frame, &c3, &r3);
            }
            1 => {
                fill_rectangle(&mut frame, &c2, &r1);
                fill_rectangle(&mut frame, &c3, &r2);
                fill_rectangle(&mut frame, &c1, &r3);
            }
            _ => {
                fill_rectangle(&mut frame, &c3, &r1);
                fill_rectangle(&mut frame, &c1, &r2);
                fill_rectangle(&mut frame, &c2, &r3);
            }
        }
        i = i.wrapping_add(1);
        frame.present(&fb).unwrap();
        thread::sleep(time::Duration::from_millis(1800));
    }
}

fn main() {
    if let Err(ref e) = run() {
        println!("error: {}", e);
        ::std::process::exit(1);
    }
}
