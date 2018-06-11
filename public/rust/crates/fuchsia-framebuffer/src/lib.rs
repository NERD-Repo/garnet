#![allow(dead_code)]
#[macro_use]
extern crate failure;
extern crate fdio;
extern crate fidl_fuchsia_display as display;
extern crate fuchsia_async as async;
extern crate fuchsia_zircon as zx;
extern crate shared_buffer;

use async::futures::{FutureExt, StreamExt};
use display::{ControllerEvent, ControllerProxy};
use failure::Error;
use fdio::fdio_sys::{fdio_ioctl, IOCTL_FAMILY_DISPLAY_CONTROLLER, IOCTL_KIND_GET_HANDLE};
use fdio::make_ioctl;
use shared_buffer::SharedBuffer;
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::mem;
use std::os::unix::io::AsRawFd;
use std::ptr;
use std::rc::Rc;
use zx::sys::{zx_cache_flush, zx_handle_t, ZX_CACHE_FLUSH_DATA, ZX_VM_FLAG_PERM_READ,
              ZX_VM_FLAG_PERM_WRITE};
use zx::{Handle, Status, Vmar, Vmo};

#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_NONE: ::std::os::raw::c_uint = 0;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_RGB_565: ::std::os::raw::c_uint = 131073;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_RGB_332: ::std::os::raw::c_uint = 65538;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_RGB_2220: ::std::os::raw::c_uint = 65539;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_ARGB_8888: ::std::os::raw::c_uint = 262148;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_RGB_x888: ::std::os::raw::c_uint = 262149;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_MONO_8: ::std::os::raw::c_uint = 65543;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_GRAY_8: ::std::os::raw::c_uint = 65543;
#[allow(non_camel_case_types, non_upper_case_globals)]
const ZX_PIXEL_FORMAT_MONO_1: ::std::os::raw::c_uint = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
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

impl Default for PixelFormat {
    fn default() -> PixelFormat {
        PixelFormat::Unknown
    }
}

impl From<u32> for PixelFormat {
    fn from(pixel_format: u32) -> Self {
        #[allow(non_upper_case_globals)]
        match pixel_format {
            ZX_PIXEL_FORMAT_ARGB_8888 => PixelFormat::Argb8888,
            ZX_PIXEL_FORMAT_MONO_1 => PixelFormat::Mono1,
            ZX_PIXEL_FORMAT_MONO_8 => PixelFormat::Mono8,
            ZX_PIXEL_FORMAT_RGB_2220 => PixelFormat::Rgb2220,
            ZX_PIXEL_FORMAT_RGB_332 => PixelFormat::Rgb332,
            ZX_PIXEL_FORMAT_RGB_565 => PixelFormat::Rgb565,
            ZX_PIXEL_FORMAT_RGB_x888 => PixelFormat::RgbX888,
            // ZX_PIXEL_FORMAT_GRAY_8 is an alias for ZX_PIXEL_FORMAT_MONO_8
            ZX_PIXEL_FORMAT_NONE => PixelFormat::Unknown,
            _ => PixelFormat::Unknown,
        }
    }
}

impl Into<u32> for PixelFormat {
    fn into(self) -> u32 {
        match self {
            PixelFormat::Argb8888 => ZX_PIXEL_FORMAT_ARGB_8888,
            PixelFormat::Mono1 => ZX_PIXEL_FORMAT_MONO_1,
            PixelFormat::Mono8 => ZX_PIXEL_FORMAT_MONO_8,
            PixelFormat::Rgb2220 => ZX_PIXEL_FORMAT_RGB_2220,
            PixelFormat::Rgb332 => ZX_PIXEL_FORMAT_RGB_332,
            PixelFormat::Rgb565 => ZX_PIXEL_FORMAT_RGB_565,
            PixelFormat::RgbX888 => ZX_PIXEL_FORMAT_RGB_x888,
            PixelFormat::Gray8 => ZX_PIXEL_FORMAT_GRAY_8,
            PixelFormat::Unknown => ZX_PIXEL_FORMAT_NONE,
        }
    }
}

fn pixel_format_bytes(pixel_format: u32) -> usize {
    ((pixel_format >> 16) & 7) as usize
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub linear_stride_pixels: u32,
    pub format: PixelFormat,
    pub pixel_size_bytes: u32,
}

pub struct Frame<'a> {
    config: Config,
    image_id: u64,
    pixel_size: usize,
    pixel_buffer_addr: usize,
    pixel_buffer: SharedBuffer<'a>,
    vmo: Vmo,
}

impl<'a> Frame<'a> {
    pub fn new(
        framebuffer: &FrameBuffer, executor: &mut async::Executor,
    ) -> Result<Frame<'a>, Error> {
        let vmo_out: Rc<RefCell<Option<Vmo>>> = Rc::new(RefCell::new(None));
        let vmo_response = framebuffer
            .controller
            .allocate_vmo(framebuffer.config.pixel_size_bytes as u64)
            .map(|(status, vmo)| {
                if status == Status::OK {
                    *vmo_out.borrow_mut() = vmo;
                }
            });
        executor.run_singlethreaded(vmo_response)?;
        let vmo_out2 = *vmo_out.borrow();
        let byte_size = framebuffer.byte_size();
        if let Some(image_vmo) = vmo_out2 {
            println!("image_vmo = {:#?}", &image_vmo);
            let pixel_buffer_addr = Vmar::root_self().map(
                0,
                &image_vmo,
                0,
                byte_size as usize,
                ZX_VM_FLAG_PERM_READ | ZX_VM_FLAG_PERM_WRITE,
            )?;

            let frame_buffer_pixel_ptr = pixel_buffer_addr as *mut u8;
            Ok(Frame {
                config: framebuffer.get_config(),
                image_id: 0,
                pixel_size: 0,
                pixel_buffer_addr,
                pixel_buffer: unsafe { SharedBuffer::new(frame_buffer_pixel_ptr, byte_size as usize) },
                vmo: image_vmo,
            })
        } else {
            Err(format_err!("Could not allocate VMO"))
        }
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
    controller: ControllerProxy,
    config: Config,
}

impl FrameBuffer {
    pub fn new(executor: &mut async::Executor) -> Result<FrameBuffer, Error> {
        let device_path = format!("/dev/class/display-controller/{:03}", 0);
        let file = OpenOptions::new().read(true).write(true).open(device_path)?;
        let fd = file.as_raw_fd() as i32;
        let ioctl_display_controller_get_handle =
            make_ioctl(IOCTL_KIND_GET_HANDLE, IOCTL_FAMILY_DISPLAY_CONTROLLER, 1);
        let mut display_handle: zx_handle_t = 0;
        let display_handle_ptr: *mut std::os::raw::c_void =
            &mut display_handle as *mut _ as *mut std::os::raw::c_void;
        let result_size = unsafe {
            fdio_ioctl(
                fd,
                ioctl_display_controller_get_handle,
                ptr::null(),
                0,
                display_handle_ptr,
                mem::size_of::<zx_handle_t>(),
            )
        };

        if result_size != mem::size_of::<zx_handle_t>() as isize {
            return Err(format_err!(
                "ioctl_display_controller_get_handle failed: {}",
                result_size
            ));
        }

        let zx_handle = unsafe { Handle::from_raw(display_handle) };
        let channel = async::Channel::from_channel(zx_handle.into())?;
        let proxy = ControllerProxy::new(channel);
        let config: Rc<RefCell<Option<Config>>> = Rc::new(RefCell::new(None));
        let stream = proxy.take_event_stream();
        let event_listener = stream
            .filter(|event| {
                match event {
                    ControllerEvent::DisplaysChanged { added, .. } => {
                        let mut zx_pixel_format = 0;
                        let mut linear_stride_pixels = 0;
                        let mut pixel_format = PixelFormat::Unknown;
                        let mut pixel_size_bytes = 0;
                        if added.len() > 0 {
                            let first_added = &added[0];
                            if first_added.pixel_format.len() > 0 {
                                zx_pixel_format = first_added.pixel_format[0];
                                pixel_format = zx_pixel_format.into();
                            }
                            if first_added.modes.len() > 0 {
                                let mode = &first_added.modes[0];
                                if pixel_format != PixelFormat::Unknown {
                                    pixel_size_bytes = pixel_format_bytes(zx_pixel_format);
                                    linear_stride_pixels = pixel_format_bytes(zx_pixel_format)
                                        as u32
                                        * mode.horizontal_resolution;
                                }
                                let calculated_config = Config {
                                    width: mode.horizontal_resolution,
                                    height: mode.vertical_resolution,
                                    linear_stride_pixels,
                                    format: pixel_format,
                                    pixel_size_bytes: pixel_size_bytes as u32,
                                };
                                *config.borrow_mut() = Some(calculated_config);
                            }
                        }
                    }
                    _ => {}
                }
                Ok(true)
            })
            .next();

        executor
            .run_singlethreaded(event_listener)
            .map_err(|(e, _rest_of_stream)| e)?;

        let config = *config.borrow();

        println!("config = {:#?}", config);

        if let Some(config) = config {
            Ok(FrameBuffer {
                display_controller: file,
                controller: proxy,
                config: config,
            })
        } else {
            return Err(format_err!(
                "ioctl_display_controller_get_handle failed: {}",
                result_size
            ));
        }
    }

    pub fn new_frame<'a>(&self, executor: &mut async::Executor) -> Result<Frame<'a>, Error> {
        Frame::new(&self, executor)
    }

    pub fn get_config(&self) -> Config {
        Config {
            height: 0,
            width: 0,
            linear_stride_pixels: 0,
            format: PixelFormat::Unknown,
            pixel_size_bytes: 0,
        }
    }

    pub fn byte_size(&self) -> u32 {
        self.config.height * self.config.linear_stride_pixels
    }
}

impl Drop for FrameBuffer {
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    extern crate fuchsia_async as async;

    use FrameBuffer;

    #[test]
    fn test_framebuffer() {
        let mut executor = async::Executor::new().unwrap();
        let fb = FrameBuffer::new(&mut executor).unwrap();
        let _frame = fb.new_frame(&mut executor).unwrap();
    }
}
