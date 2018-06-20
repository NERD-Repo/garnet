use fuchsia_framebuffer::Frame;
use text::Face;

<<<<<<< HEAD
pub struct PaintCtx<'a, 'b: 'a> {
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'b>,
    pub face: &'a mut Face<'a>,
}

impl<'a, 'b: 'a> PaintCtx<'a, 'b> {}
=======
pub struct PaintCtx<'a> {
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'a>,
    pub face: &'a mut Face<'a>,
}

impl<'a> PaintCtx<'a> {}
>>>>>>> Bring in widgets
