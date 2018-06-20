use fuchsia_framebuffer::Frame;
use text::Face;

<<<<<<< HEAD
<<<<<<< HEAD
pub struct PaintCtx<'a, 'b: 'a> {
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'b>,
    pub face: &'a mut Face<'a>,
}

impl<'a, 'b: 'a> PaintCtx<'a, 'b> {}
=======
pub struct PaintCtx<'a> {
=======
pub struct PaintCtx<'a, 'b: 'a> {
>>>>>>> More WIP
    //pub render_target: GenericRenderTarget,
    pub frame: &'a mut Frame<'b>,
    pub face: &'a mut Face<'a>,
}

<<<<<<< HEAD
impl<'a> PaintCtx<'a> {}
>>>>>>> Bring in widgets
=======
impl<'a, 'b: 'a> PaintCtx<'a, 'b> {}
>>>>>>> More WIP
