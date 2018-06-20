use fuchsia_framebuffer::Frame;
use text::Face;

pub struct PaintCtx<'a> {
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'a>,
    pub face: &'a mut Face<'a>,
}

impl<'a> PaintCtx<'a> {}
