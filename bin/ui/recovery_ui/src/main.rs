#![allow(unused_imports, unused_variables, dead_code)]
extern crate failure;
extern crate fidl_fuchsia_amber as amber;
extern crate font_rs;
extern crate fuchsia_app as app;
extern crate fuchsia_async as async;
extern crate fuchsia_framebuffer;
extern crate fuchsia_zircon;

mod color;
mod geometry;
pub mod paint;
pub mod text;
pub mod widget;

use app::client::connect_to_service;
pub use color::Color;
use failure::Error;
use fuchsia_framebuffer::{Frame, FrameBuffer, PixelFormat};
pub use geometry::{Point, Rectangle, Size};
use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::rc::Rc;
use std::{thread, time};
use text::Face;
use widget::{Button, Column, Label, Padding, Row};

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

fn main() -> Result<(), Error> {
    println!("Recovery UI");
    wait_for_close();

    let mut executor = async::Executor::new().unwrap();

    let fb = FrameBuffer::new(None, &mut executor).unwrap();
    let config = fb.get_config();

    let values565 = &[31, 248];
    let values8888 = &[255, 0, 255, 255];

    let mut pink_frame = fb.new_frame(&mut executor)?;

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

    let mut state = UiState::new();
    build_recovery(&mut state);
    let main = Box::new(UiMain::new(state));
    let mut face = Face::new(FONT_DATA).unwrap();
    let mut paint_ctx = paint::PaintCtx {
        frame: &mut pink_frame,
        face: &mut face,
    };

    let amber_control = connect_to_service::<amber::ControlMarker>().unwrap();
    let srcs = amber_control.list_srcs();

    executor.run_singlethreaded(srcs)?;

    loop {
        main.paint(&mut paint_ctx);
        paint_ctx.frame.present(&fb).unwrap();
        thread::sleep(time::Duration::from_millis(25000));
    }
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

pub struct MainPaintCtx<'a, 'b: 'a, 'c: 'a + 'b> {
    inner: &'a mut paint::PaintCtx<'b, 'c>,
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

fn get_dimensions(frame: &Frame) -> (f32, f32) {
    (frame.get_width() as f32, frame.get_height() as f32)
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
        let size = get_dimensions(paint_ctx.frame);
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
    widgets: &mut [Box<Widget>], graph: &Graph, geom: &[Geometry], paint_ctx: &mut MainPaintCtx,
    node: Id, pos: (f32, f32),
) {
    let g = geom[node].offset(pos);
    widgets[node].paint(paint_ctx, &g);
    for child in graph.children[node].clone() {
        paint_rec(widgets, graph, geom, paint_ctx, child, g.pos);
    }
}

fn layout_rec(
    widgets: &mut [Box<Widget>], ctx: &mut LayoutCtx, graph: &Graph, bc: &BoxConstraints, node: Id,
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
        let mut paint_ctx = MainPaintCtx {
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
    widgets: &mut [Box<Widget>], graph: &Graph, x: f32, y: f32, mods: u32, which: MouseButton,
    ty: MouseType, ctx: &mut HandlerCtx,
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

impl<'a, 'b, 'c> MainPaintCtx<'a, 'b, 'c> {
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

struct RecoveryState {
    message: String,
}

fn pad(widget: Id, ui: &mut UiState) -> Id {
    Padding::uniform(25.0).ui(widget, ui)
}

fn build_recovery(ui: &mut UiState) {
    let recovery = Rc::new(RefCell::new(RecoveryState {
        message: "Recovery Mode".to_string(),
    }));
    let message = Label::new(recovery.borrow().message.clone()).ui(ui);
    let row0 = pad(message, ui);
    let panel = Column::new().ui(&[row0], ui);
    let root = pad(panel, ui);
    ui.set_root(root);
}
