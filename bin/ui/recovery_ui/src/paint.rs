use fuchsia_framebuffer::Frame;
use text::Face;

pub struct PaintCtx<'a, 'b: 'a> {
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'b>,
    pub face: &'a mut Face<'a>,
}

impl<'a, 'b: 'a> PaintCtx<'a, 'b> {}
