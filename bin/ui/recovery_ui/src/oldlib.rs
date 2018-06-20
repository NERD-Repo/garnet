#![allow(unused)]
// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! A Rust library for using the [Zircon software framebuffer](https://goo.gl/tL1Pqi).

#[macro_use]
extern crate failure;
extern crate fdio;
extern crate font_rs;
extern crate fuchsia_zircon_sys;

mod color;
mod geometry;
pub mod paint;
pub mod text;
pub mod widget;

pub use color::Color;
use failure::Error;
use fdio::fdio_sys::{fdio_ioctl, IOCTL_FAMILY_DISPLAY, IOCTL_KIND_DEFAULT, IOCTL_KIND_GET_HANDLE};
use fdio::make_ioctl;
use fuchsia_zircon_sys::{
    zx_handle_t, zx_vmar_map, zx_vmar_root_self, ZX_VM_FLAG_PERM_READ, ZX_VM_FLAG_PERM_WRITE,
};
pub use geometry::{Point, Rectangle, Size};
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::os::unix::io::AsRawFd;
use std::ptr;
use std::thread;

#[repr(C)]
struct zx_display_info_t {
    format: u32,
    width: u32,
    height: u32,
    stride: u32,
    pixelsize: u32,
    flags: u32,
}

const ZX_PIXEL_FORMAT_RGB_565: u32 = 0x00020001;
const ZX_PIXEL_FORMAT_ARGB_8888: u32 = 0x00040004;
const ZX_PIXEL_FORMAT_RGB_X888: u32 = 0x00040005;
const ZX_PIXEL_FORMAT_MONO_1: u32 = 0x00000006;
const ZX_PIXEL_FORMAT_MONO_8: u32 = 0x00010007;

#[repr(C)]
struct ioctl_display_get_fb_t {
    vmo: zx_handle_t,
    info: zx_display_info_t,
}

/// The native pixel format of the framebuffer.
/// These values are mapped to the values from [zircon/pixelformat.h](https://goo.gl/nM2T7T).
#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Rgb565,
    Argb8888,
    RgbX888,
    Mono1,
    Mono8,
    Unknown,
}

fn get_info_for_device(fd: i32) -> Result<ioctl_display_get_fb_t, Error> {
    let ioctl_display_get_fb_value = make_ioctl(IOCTL_KIND_GET_HANDLE, IOCTL_FAMILY_DISPLAY, 1);
    let mut framebuffer: ioctl_display_get_fb_t = ioctl_display_get_fb_t {
        vmo: 0,
        info: zx_display_info_t {
            format: 0,
            width: 0,
            height: 0,
            stride: 0,
            pixelsize: 0,
            flags: 0,
        },
    };
    let framebuffer_ptr: *mut std::os::raw::c_void =
        &mut framebuffer as *mut _ as *mut std::os::raw::c_void;

    let status = unsafe {
        fdio_ioctl(
            fd,
            ioctl_display_get_fb_value,
            ptr::null(),
            0,
            framebuffer_ptr,
            mem::size_of::<ioctl_display_get_fb_t>(),
        )
    };

    if status < 0 {
        if status == -2 {
            bail!(
                "Software framebuffer is not supported on devices with enabled GPU drivers. \
                 See README.md for instructions on how to disable the GPU driver."
            );
        }
        bail!("ioctl failed with {}", status);
    }

    Ok(framebuffer)
}

/// Struct that provides the interface to the Zircon framebuffer.
pub struct FrameBuffer {
    file: File,
    pixel_format: PixelFormat,
    frame_buffer_pixels: Vec<u8>,
    width: usize,
    height: usize,
    stride: usize,
    pixel_size: usize,
}

impl FrameBuffer {
    /// Create a new framebufer. By default this will open the framebuffer
    /// device at /dev/class/framebufer/000 but you can pick a different index.
    /// At the time of this writing, though, there are never any other framebuffer
    /// devices.
    pub fn new(index: Option<isize>) -> Result<FrameBuffer, Error> {
        let index = index.unwrap_or(0);
        let device_path = format!("/dev/class/framebuffer/{:03}", index);
        let file = OpenOptions::new().read(true).write(true).open(device_path)?;
        let fd = file.as_raw_fd() as i32;
        let get_fb_data = get_info_for_device(fd)?;
        let pixel_format = match get_fb_data.info.format {
            ZX_PIXEL_FORMAT_RGB_565 => PixelFormat::Rgb565,
            ZX_PIXEL_FORMAT_ARGB_8888 => PixelFormat::Argb8888,
            ZX_PIXEL_FORMAT_RGB_X888 => PixelFormat::RgbX888,
            ZX_PIXEL_FORMAT_MONO_1 => PixelFormat::Mono1,
            ZX_PIXEL_FORMAT_MONO_8 => PixelFormat::Mono8,
            _ => PixelFormat::Unknown,
        };

        let rowbytes = get_fb_data.info.stride * get_fb_data.info.pixelsize;
        let byte_size = rowbytes * get_fb_data.info.height;
        let map_flags = ZX_VM_FLAG_PERM_READ | ZX_VM_FLAG_PERM_WRITE;
        let mut pixel_buffer_addr: usize = 0;
        let pixel_buffer_addr_ptr: *mut usize = &mut pixel_buffer_addr;
        let status = unsafe {
            zx_vmar_map(
                zx_vmar_root_self(),
                0,
                get_fb_data.vmo,
                0,
                byte_size as usize,
                map_flags,
                pixel_buffer_addr_ptr,
            )
        };

        if status < 0 {
            bail!("zx_vmar_map failed with {}", status);
        }

        let frame_buffer_pixel_ptr = pixel_buffer_addr as *mut u8;
        let frame_buffer_pixels: Vec<u8> = unsafe {
            Vec::from_raw_parts(
                frame_buffer_pixel_ptr,
                byte_size as usize,
                byte_size as usize,
            )
        };

        Ok(FrameBuffer {
            file,
            pixel_format,
            frame_buffer_pixels,
            width: get_fb_data.info.width as usize,
            height: get_fb_data.info.height as usize,
            stride: get_fb_data.info.stride as usize,
            pixel_size: get_fb_data.info.pixelsize as usize,
        })
    }

    /// Call to cause changes you made to the pixel buffer to appear on screen.
    pub fn flush(&self) -> Result<(), Error> {
        let ioctl_display_flush_fb_value = make_ioctl(IOCTL_KIND_DEFAULT, IOCTL_FAMILY_DISPLAY, 2);
        let status = unsafe {
            fdio_ioctl(
                self.file.as_raw_fd(),
                ioctl_display_flush_fb_value,
                ptr::null(),
                0,
                ptr::null_mut(),
                0,
            )
        };

        if status < 0 {
            bail!("ioctl failed with {}", status);
        }

        Ok(())
    }

    /// Return the width and height of the framebuffer.
    pub fn get_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Return stride of the framebuffer in pixels.
    pub fn get_stride(&self) -> usize {
        self.stride
    }

    /// Return the size in bytes of a pixel pixels.
    pub fn get_pixel_size(&self) -> usize {
        self.pixel_size
    }

    /// Return the size in bytes of a pixel pixels.
    pub fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Return the pixel buffer as a mutable slice.
    pub fn get_pixels(&mut self) -> &mut [u8] {
        self.frame_buffer_pixels.as_mut_slice()
    }

    pub fn fill_with_color(&mut self, color: &Color) {
        let pixel_size = self.get_pixel_size();
        let values565 = color.to_565();
        let values8888 = color.to_8888();
        let pixel_data = self.get_pixels();
        for pixel_slice in pixel_data.chunks_mut(pixel_size) {
            if pixel_size == 4 {
                pixel_slice.copy_from_slice(&values8888);
            } else {
                pixel_slice.copy_from_slice(&values565);
            }
        }
    }

    fn fill_rectangle(&mut self, color: &Color, r: &Rectangle) {
        let pixel_size = self.get_pixel_size();
        let stride = self.get_stride();
        let stride_bytes = stride * pixel_size;
        let values565 = color.to_565();
        let values8888 = color.to_8888();
        let pixel_data = self.get_pixels();
        for y in r.origin.y..r.bottom() {
            let row_offset = stride_bytes * y as usize;
            let left_offset = row_offset + r.origin.x as usize * pixel_size;
            let right_offset = left_offset + r.size.width as usize * pixel_size;
            let row_slice = &mut pixel_data[left_offset..right_offset];
            for pixel_slice in row_slice.chunks_mut(pixel_size) {
                if pixel_size == 4 {
                    pixel_slice.copy_from_slice(&values8888);
                } else {
                    pixel_slice.copy_from_slice(&values565);
                }
            }
        }
    }
}

impl fmt::Debug for FrameBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "FrameBuffer {{ file: {:?}, pixel_format: {:?}, width: {}, height: {}, \
             stride: {}, pixel_size: {} }}",
            self.file, self.pixel_format, self.width, self.height, self.stride, self.pixel_size,
        )
    }
}

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

pub use widget::Widget;

pub struct UiMain {
    state: RefCell<UiState>,
}

/// An identifier for widgets, scoped to a UiMain instance. This is the
/// "entity" of the entity-component-system architecture.
pub type Id = usize;

pub struct UiState {
    listeners: BTreeMap<Id, Vec<Box<FnMut(&mut Any, ListenerCtx)>>>,

    /// The widget tree and associated state is split off into a separate struct
    /// so that we can use a mutable reference to it as the listener context.
    inner: UiInner,
}

/// The context given to listeners.
///
/// Listeners are allowed to poke widgets and mutate the graph.
pub struct UiInner {
    /// The individual widget trait objects.
    widgets: Vec<Box<Widget>>,

    /// Graph of widgets (actually a strict tree structure, so maybe should be renamed).
    graph: Graph,

    /// The state (other than widget tree) is a separate object, so that a
    /// mutable reference to it can be used as a layout context.
    c: LayoutCtx,
}

/// The context given to layout methods.
pub struct LayoutCtx {
    text_renderer: TextRenderer,
    //
    // handle: WindowHandle,
    /// Bounding box of each widget. The position is relative to the parent.
    geom: Vec<Geometry>,

    /// Queue of events to distribute to listeners
    event_q: Vec<(Id, Box<Any>)>,
}

#[derive(Default)]
struct Graph {
    root: Id,
    children: Vec<Vec<Id>>,
    parent: Vec<Id>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Geometry {
    // Maybe PointF is a better type, then we could use the math from direct2d?
    pub pos: (f32, f32),
    pub size: (f32, f32),
}

#[derive(Clone, Copy)]
pub struct BoxConstraints {
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
}

pub enum LayoutResult {
    Size((f32, f32)),
    RequestChild(Id, BoxConstraints),
}

// Contexts for widget methods.

/// Context given to handlers.
pub struct HandlerCtx<'a> {
    /// The id of the node sending the event
    id: Id,

    c: &'a mut LayoutCtx,
}

pub struct ListenerCtx<'a> {
    id: Id,

    inner: &'a mut UiInner,
}

pub struct PaintCtx<'a, 'b: 'a> {
    inner: &'a mut paint::PaintCtx<'b>,
    text_renderer: &'a TextRenderer,
}

/// A command for exiting. TODO: move commands entirely to client.
pub const COMMAND_EXIT: u32 = 0x100;

impl Geometry {
    fn offset(&self, offset: (f32, f32)) -> Geometry {
        Geometry {
            pos: (self.pos.0 + offset.0, self.pos.1 + offset.1),
            size: self.size,
        }
    }
}

impl UiMain {
    pub fn new(state: UiState) -> UiMain {
        UiMain {
            state: RefCell::new(state),
        }
    }

    pub fn paint(&self, paint_ctx: &mut paint::PaintCtx) -> bool {
        let mut state = self.state.borrow_mut();
        let root = state.graph.root;
        let size = paint_ctx.framebuffer.get_dimensions();
        let bc = BoxConstraints::tight((size.0 as f32, size.1 as f32));
        // TODO: be lazier about relayout
        state.layout(&bc, root);
        state.paint(paint_ctx, root);
        false
    }
}

impl UiState {
    pub fn new() -> UiState {
        UiState {
            listeners: Default::default(),
            inner: UiInner {
                widgets: Vec::new(),
                graph: Default::default(),
                c: LayoutCtx {
                    text_renderer: TextRenderer {},
                    geom: Vec::new(),
                    event_q: Vec::new(),
                },
            },
        }
    }

    /// Add a listener that expects a specific type.
    pub fn add_listener<A, F>(&mut self, node: Id, mut f: F)
    where
        A: Any,
        F: FnMut(&mut A, ListenerCtx) + 'static,
    {
        let wrapper: Box<FnMut(&mut Any, ListenerCtx)> = Box::new(move |a, ctx| {
            if let Some(arg) = a.downcast_mut() {
                f(arg, ctx)
            } else {
                println!("type mismatch in listener arg");
            }
        });
        self.listeners
            .entry(node)
            .or_insert(Vec::new())
            .push(wrapper);
    }

    fn mouse(&mut self, x: f32, y: f32, mods: u32, which: MouseButton, ty: MouseType) {
        mouse_rec(
            &mut self.inner.widgets,
            &self.inner.graph,
            x,
            y,
            mods,
            which,
            ty,
            &mut HandlerCtx {
                id: self.inner.graph.root,
                c: &mut self.inner.c,
            },
        );
        self.dispatch_events();
    }

    fn dispatch_events(&mut self) {
        let event_q = mem::replace(&mut self.c.event_q, Vec::new());
        for (id, mut event) in event_q {
            if let Some(listeners) = self.listeners.get_mut(&id) {
                for listener in listeners {
                    let ctx = ListenerCtx {
                        id,
                        inner: &mut self.inner,
                    };
                    listener(event.deref_mut(), ctx);
                }
            }
        }
    }
}

// Do pre-order traversal on graph, painting each node in turn.
//
// Implemented as a recursion, but we could use an explicit queue instead.
fn paint_rec(
    widgets: &mut [Box<Widget>],
    graph: &Graph,
    geom: &[Geometry],
    paint_ctx: &mut PaintCtx,
    node: Id,
    pos: (f32, f32),
) {
    let g = geom[node].offset(pos);
    widgets[node].paint(paint_ctx, &g);
    for child in graph.children[node].clone() {
        paint_rec(widgets, graph, geom, paint_ctx, child, g.pos);
    }
}

fn layout_rec(
    widgets: &mut [Box<Widget>],
    ctx: &mut LayoutCtx,
    graph: &Graph,
    bc: &BoxConstraints,
    node: Id,
) -> (f32, f32) {
    let mut size = None;
    loop {
        let layout_res = widgets[node].layout(bc, &graph.children[node], size, ctx);
        match layout_res {
            LayoutResult::Size(size) => {
                ctx.geom[node].size = size;
                return size;
            }
            LayoutResult::RequestChild(child, child_bc) => {
                size = Some(layout_rec(widgets, ctx, graph, &child_bc, child));
            }
        }
    }
}

fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

impl Deref for UiState {
    type Target = UiInner;

    fn deref(&self) -> &UiInner {
        &self.inner
    }
}

impl DerefMut for UiState {
    fn deref_mut(&mut self) -> &mut UiInner {
        &mut self.inner
    }
}

impl UiInner {
    /// Send an arbitrary payload to a widget. The type and interpretation of the
    /// payload depends on the specific target widget.
    pub fn poke<A: Any>(&mut self, node: Id, payload: &mut A) -> bool {
        let mut ctx = HandlerCtx {
            id: node,
            c: &mut self.c,
        };
        self.widgets[node].poke(payload, &mut ctx)
    }

    /// Put a widget in the graph and add its children. Returns newly allocated
    /// id for the node.
    pub fn add<W>(&mut self, widget: W, children: &[Id]) -> Id
    where
        W: Widget + 'static,
    {
        let id = self.graph.alloc_node();
        self.widgets.push(Box::new(widget));
        self.c.geom.push(Default::default());
        for &child in children {
            self.graph.append_child(id, child);
        }
        id
    }

    pub fn set_root(&mut self, root: Id) {
        self.graph.root = root;
    }

    // The following methods are really UiState methods, but don't need access to listeners
    // so are more concise to implement here.

    fn paint(&mut self, paint_ctx: &mut paint::PaintCtx, root: Id) {
        let mut paint_ctx = PaintCtx {
            inner: paint_ctx,
            text_renderer: &self.c.text_renderer,
        };
        paint_rec(
            &mut self.widgets,
            &self.graph,
            &self.c.geom,
            &mut paint_ctx,
            root,
            (0.0, 0.0),
        );
    }

    fn layout(&mut self, bc: &BoxConstraints, root: Id) {
        layout_rec(&mut self.widgets, &mut self.c, &self.graph, bc, root);
    }
}

impl BoxConstraints {
    pub fn tight(size: (f32, f32)) -> BoxConstraints {
        BoxConstraints {
            min_width: size.0,
            max_width: size.0,
            min_height: size.1,
            max_height: size.1,
        }
    }

    pub fn constrain(&self, size: (f32, f32)) -> (f32, f32) {
        (
            clamp(size.0, self.min_width, self.max_width),
            clamp(size.1, self.min_height, self.max_height),
        )
    }
}

impl LayoutCtx {
    pub fn text_renderer(&self) -> &TextRenderer {
        &self.text_renderer
    }

    pub fn position_child(&mut self, child: Id, pos: (f32, f32)) {
        self.geom[child].pos = pos;
    }

    pub fn get_child_size(&self, child: Id) -> (f32, f32) {
        self.geom[child].size
    }
}

fn mouse_rec(
    widgets: &mut [Box<Widget>],
    graph: &Graph,
    x: f32,
    y: f32,
    mods: u32,
    which: MouseButton,
    ty: MouseType,
    ctx: &mut HandlerCtx,
) -> bool {
    let node = ctx.id;
    let g = ctx.c.geom[node];
    let x = x - g.pos.0;
    let y = y - g.pos.1;
    let mut handled = false;
    if x >= 0.0 && y >= 0.0 && x < g.size.0 && y < g.size.1 {
        handled = widgets[node].mouse(x, y, mods, which, ty, ctx);
        for child in graph.children[node].iter().rev() {
            if handled {
                break;
            }
            ctx.id = *child;
            handled = mouse_rec(widgets, graph, x, y, mods, which, ty, ctx);
        }
    }
    handled
}

impl<'a> HandlerCtx<'a> {
    pub fn invalidate(&self) {}

    // Send an event, to be handled by listeners.
    pub fn send_event<A: Any>(&mut self, a: A) {
        self.c.event_q.push((self.id, Box::new(a)));
    }
}

impl<'a> Deref for ListenerCtx<'a> {
    type Target = UiInner;

    fn deref(&self) -> &UiInner {
        self.inner
    }
}

impl<'a> DerefMut for ListenerCtx<'a> {
    fn deref_mut(&mut self) -> &mut UiInner {
        self.inner
    }
}

impl<'a> ListenerCtx<'a> {
    /// Bubble a poke action up the widget hierarchy, until a widget handles it.
    ///
    /// Returns true if any widget handled the action.
    pub fn poke_up<A: Any>(&mut self, payload: &mut A) -> bool {
        let mut node = self.id;
        loop {
            let parent = self.graph.parent[node];
            if parent == node {
                return false;
            }
            node = parent;
            if self.poke(node, payload) {
                return true;
            }
        }
    }
}

impl<'a, 'b> PaintCtx<'a, 'b> {
    pub fn text_renderer(&self) -> &TextRenderer {
        self.text_renderer
    }
}

impl Graph {
    pub fn alloc_node(&mut self) -> Id {
        let id = self.children.len();
        self.children.push(vec![]);
        self.parent.push(id);
        id
    }

    pub fn append_child(&mut self, parent: Id, child: Id) {
        self.children[parent].push(child);
        self.parent[child] = parent;
    }
}

#[derive(Debug)]
pub struct TextRenderer;
#[derive(Debug, Clone, Copy)]
pub struct MouseButton;
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MouseType {
    Down,
    Up,
}
#[derive(Debug)]
pub struct GenericRenderTarget;
