#[macro_use]
extern crate failure;
extern crate fuchsia_framebuffer_sys;
extern crate fuchsia_zircon;
extern crate shared_buffer;

use failure::Error;

#[derive(Debug)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub linear_stride_pixels: u32,
    pub format: PixelFormat,
}

pub struct Frame<'a> {
    config: Config,
    image_id: u64,
    pixel_size: usize,
    pixel_buffer_addr: usize,
    pixel_buffer: SharedBuffer<'a>,
}

impl<'a> Frame<'a> {
    pub fn new(framebuffer: &FrameBuffer) -> Result<Frame<'a>, Error> {
        return Err(format_err!("Not yet implemented"));
    }

    pub fn write_pixel(&self, x: u32, y: u32, value: &[u8]) {
        let pixel_size = pixel_format_bytes(self.config.format.into());
        let offset = self.config.linear_stride_pixels as usize * pixel_size * y as usize
            + x as usize * pixel_size;
        self.pixel_buffer.write_at(offset, value);
    }

    pub fn fill_rectangle(&self, x: u32, y: u32, width: u32, height: u32, value: &[u8]) {
        let left = x.min(self.config.width);
        let right = (left + width).min(self.config.width);
        let top = y.min(self.config.height);
        let bottom = (top + height).min(self.config.width);
        for j in top..bottom {
            for i in left..right {
                self.write_pixel(i, j, value);
            }
        }
    }

    pub fn present(&self) -> Result<(), Error> {
        return Err(format_err!("Not yet implemented"));
    }

    fn byte_size(&self) -> usize {
        self.config.linear_stride_pixels as usize * self.pixel_size * self.config.height as usize
    }
}

impl<'a> Drop for Frame<'a> {
    fn drop(&mut self) {
        Vmar::root_self()
            .unmap(self.pixel_buffer_addr, self.byte_size())
            .unwrap();
    }
}

pub struct FrameBuffer {}

impl FrameBuffer {
    pub fn new() -> Result<FrameBuffer, Error> {
        return Err(format_err!("Not yet implemented"));
    }

    pub fn new_frame<'a>(&self) -> Result<Frame<'a>, Error> {
        Frame::new(&self)
    }

    pub fn get_config(&self) -> Config {
        let mut width = 0;
        let mut height = 0;
        let mut linear_stride_pixels = 0;
        let mut format = 0;
        Config {
            width,
            height,
            linear_stride_pixels,
            format: PixelFormat::from(format),
        }
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {
        unsafe {
            fb_release();
        }
    }
}
