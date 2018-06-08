#![allow(unused_imports, unused_variables, dead_code)]
#[macro_use]
extern crate failure;
extern crate fdio;
extern crate fidl_fuchsia_display;
extern crate fuchsia_async;
extern crate fuchsia_zircon;
extern crate shared_buffer;

use failure::Error;
use fdio::fdio_sys::{fdio_ioctl, IOCTL_FAMILY_DISPLAY_CONTROLLER, IOCTL_KIND_GET_HANDLE};
use fdio::make_ioctl;
use fuchsia_zircon::sys::zx_handle_t;
use fuchsia_zircon::{Unowned, Vmar, Vmo};
use shared_buffer::SharedBuffer;
use std::fs::{File, OpenOptions};
use std::mem;
use std::os::unix::io::AsRawFd;
use std::ptr;

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Argb8888,
    Gray8,
    Mono1,
    Mono8,
    Rgb2220,
    Rgb332,
    Rgb565,
    RgbX888,
    Unknown,
}

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
        let pixel_size = 4;
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

pub struct FrameBuffer {
    display_controller: File,
}

impl FrameBuffer {
    pub fn new() -> Result<FrameBuffer, Error> {
        let device_path = format!("/dev/class/display-controller/{:03}", 0);
        println!("device_path = {}", device_path);
        let file = OpenOptions::new().read(true).write(true).open(device_path)?;
        let fd = file.as_raw_fd() as i32;
        println!("fd = {}", fd);
        let ioctl_display_controller_get_handle = make_ioctl(IOCTL_KIND_GET_HANDLE, IOCTL_FAMILY_DISPLAY_CONTROLLER, 1);
        let mut display_handle: zx_handle_t = 0;
        let display_handle_ptr: *mut std::os::raw::c_void =
            &mut display_handle as *mut _ as *mut std::os::raw::c_void;
        let status = unsafe {
            fdio_ioctl(
                fd,
                ioctl_display_controller_get_handle,
                ptr::null(),
                0,
                display_handle_ptr,
                mem::size_of::<zx_handle_t>(),
            )
        };

        println!("display_handle = {:x}", display_handle);

        ControllerMarker::Proxy::from_channel(async::Channel::from_channel(display_handle)?)?;

        return Err(format_err!("Not yet implemented"));
    }

    pub fn new_frame<'a>(&self) -> Result<Frame<'a>, Error> {
        Frame::new(&self)
    }

    pub fn get_config(&self) -> Config {
        Config {
            height: 0,
            width: 0,
            linear_stride_pixels: 0,
            format: PixelFormat::Unknown,
        }
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use FrameBuffer;

    #[test]
    fn test_framebuffer() {
        let fb = FrameBuffer::new().unwrap();
    }
}
