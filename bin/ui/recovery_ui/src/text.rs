use font_rs::font::{parse, Font, FontError, GlyphBitmap};
use fuchsia_framebuffer::Frame;
use std::collections::HashMap;
use Color;
use Point;

#[derive(Hash, Eq, PartialEq, Debug)]
struct GlyphDescriptor {
    size: u32,
    glyph_id: u16,
}

pub struct Face<'a> {
    font: Font<'a>,
    glyphs: HashMap<GlyphDescriptor, GlyphBitmap>,
}

impl<'a> Face<'a> {
    pub fn new(data: &'a [u8]) -> Result<Face<'a>, FontError> {
        Ok(Face {
            font: parse(data)?,
            glyphs: HashMap::new(),
        })
    }

    pub fn get_glyph(&mut self, glyph_id: u16, size: u32) -> &GlyphBitmap {
        let font = &self.font;
        self.glyphs
            .entry(GlyphDescriptor { size, glyph_id })
            .or_insert_with(|| font.render_glyph(glyph_id, size).unwrap())
    }

    fn draw_glyph_at(frame: &mut Frame, color: &Color, glyph: &GlyphBitmap, location: &Point) {
<<<<<<< HEAD
        let top = location.y;
        let left = location.x;
        let glyph_data = &glyph.data.as_slice();
        let mut y = top;
        let pixel_size = frame.get_pixel_size();
        for glyph_row in glyph_data.chunks(glyph.width) {
            if y > 0 {
                let mut x = left;
                for one_pixel in glyph_row {
                    let scale = f64::from(*one_pixel) / 256.0;
                    if *one_pixel > 0 {
                        let scaled_color = color.scale(scale);
                        if pixel_size == 4 {
                            let values8888 = scaled_color.to_8888();
                            frame.write_pixel(x as u32, y as u32, &values8888);
                        } else {
                            let values565 = scaled_color.to_565();
                            frame.write_pixel(x as u32, y as u32, &values565);
                        }
                    }
                    x += 1;
                }
            }
            y += 1;
        }
=======
        // let top = location.y;
        // let left = location.x;
        // let pixel_size = fb.get_pixel_size();
        // let stride = fb.get_stride();
        // let stride_bytes = stride * pixel_size;
        // let pixel_data = fb.get_pixels();
        // let glyph_data = &glyph.data.as_slice();
        // let mut y = top;
        // for glyph_row in glyph_data.chunks(glyph.width) {
        //     if y > 0 {
        //         let mut x = left;
        //         let row_offset = stride_bytes * y as usize;
        //         for one_pixel in glyph_row {
        //             let left_offset = row_offset + x as usize * pixel_size;
        //             let scale = f64::from(*one_pixel) / 256.0;
        //             if *one_pixel > 0 {
        //                 let pixel_slice = &mut pixel_data[left_offset..left_offset + pixel_size];
        //                 let scaled_color = color.scale(scale);
        //                 if pixel_size == 4 {
        //                     let values8888 = scaled_color.to_8888();
        //                     pixel_slice.copy_from_slice(&values8888);
        //                 } else {
        //                     let values565 = scaled_color.to_565();
        //                     pixel_slice.copy_from_slice(&values565);
        //                 }
        //             }
        //             x += 1;
        //         }
        //     }
        //     y += 1;
        // }
>>>>>>> Bring in widgets
    }

    pub fn draw_text_at(&mut self, frame: &mut Frame, location: &Point, color: &Color, text: &str) {
        let mut pt = location.clone();
<<<<<<< HEAD
        let size = 72;
        for one_char in text.chars() {
            if one_char == ' ' {
                pt.x += size;
            } else {
                if let Some(glyph_id) = self.font.lookup_glyph_id(one_char as u32) {
                    println!("glyph_id = {:?}", glyph_id);
                    let glyph = self.get_glyph(glyph_id, size as u32);
                    let glyph_location = Point {
                        x: pt.x + glyph.left,
                        y: pt.y + glyph.top,
                    };
                    println!("glyph_location = {:?}", glyph_location);
                    Self::draw_glyph_at(frame, color, &glyph, &glyph_location);
                    pt.x += glyph.width as i32 + 2;
                }
            }
=======
        println!("pt = {:?}", pt);
        for one_char in text.chars() {
            let glyph = self.get_glyph((one_char as u16) - 30, 16);
            let glyph_location = Point {
                x: pt.x + glyph.left,
                y: pt.y + glyph.top,
            };
            println!("glyph_location = {:?}", glyph_location);
            Self::draw_glyph_at(frame, color, &glyph, &glyph_location);
            pt.x += glyph.width as i32 + 2;
>>>>>>> Bring in widgets
        }
    }
}
