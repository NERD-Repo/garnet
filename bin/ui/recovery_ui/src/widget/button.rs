// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A button widget

use std::any::Any;

use geometry::{Point, Rectangle, Size};
use widget::Widget;
use {BoxConstraints, Geometry, LayoutResult, MouseButton, MouseType};
<<<<<<< HEAD
<<<<<<< HEAD
use {Color, HandlerCtx, Id, LayoutCtx, MainPaintCtx, UiInner};
=======
use {Color, HandlerCtx, Id, LayoutCtx, PaintCtx, UiInner};
>>>>>>> Bring in widgets
=======
use {Color, HandlerCtx, Id, LayoutCtx, MainPaintCtx, UiInner};
>>>>>>> More WIP

/// A text label with no interaction.
pub struct Label {
    label: String,
}

/// A clickable button with a label.
pub struct Button {
    label: Label,
}

impl Label {
    pub fn new<S: Into<String>>(label: S) -> Label {
        Label {
            label: label.into(),
        }
    }

    pub fn ui(self, ctx: &mut UiInner) -> Id {
        ctx.add(self, &[])
    }
}

impl Widget for Label {
<<<<<<< HEAD
<<<<<<< HEAD
    fn paint(&mut self, paint_ctx: &mut MainPaintCtx, geom: &Geometry) {
=======
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
>>>>>>> Bring in widgets
=======
    fn paint(&mut self, paint_ctx: &mut MainPaintCtx, geom: &Geometry) {
>>>>>>> More WIP
        let location = Point {
            x: geom.pos.0 as i32,
            y: geom.pos.1 as i32,
        };
        println!("label {} loc {:?}", self.label, location);
        let white = Color::from_hash_code("#FFFFFF");
        paint_ctx
            .inner
            .face
            .draw_text_at(paint_ctx.inner.frame, &location, &white, &self.label);
    }

    fn layout(
        &mut self, bc: &BoxConstraints, _children: &[Id], _size: Option<(f32, f32)>,
        _ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        // TODO: measure text properly
        LayoutResult::Size(bc.constrain((100.0, 17.0)))
    }

    fn mouse(
        &mut self, x: f32, y: f32, mods: u32, which: MouseButton, ty: MouseType,
        ctx: &mut HandlerCtx,
    ) -> bool {
        println!("button {} {} {:x} {:?} {:?}", x, y, mods, which, ty);
        if ty == MouseType::Down {
            ctx.send_event(true);
        }
        true
    }

    fn poke(&mut self, payload: &mut Any, ctx: &mut HandlerCtx) -> bool {
        if let Some(string) = payload.downcast_ref::<String>() {
            self.label = string.clone();
            ctx.invalidate();
            true
        } else {
            println!("downcast failed");
            false
        }
    }
}

impl Button {
    pub fn new<S: Into<String>>(label: S) -> Button {
        Button {
            label: Label::new(label),
        }
    }

    pub fn ui(self, ctx: &mut UiInner) -> Id {
        ctx.add(self, &[])
    }
}

impl Widget for Button {
<<<<<<< HEAD
<<<<<<< HEAD
    fn paint(&mut self, paint_ctx: &mut MainPaintCtx, geom: &Geometry) {
=======
    fn paint(&mut self, paint_ctx: &mut PaintCtx, geom: &Geometry) {
>>>>>>> Bring in widgets
=======
    fn paint(&mut self, paint_ctx: &mut MainPaintCtx, geom: &Geometry) {
>>>>>>> More WIP
        let c1 = Color::from_hash_code("#404048");
        let r = Rectangle {
            origin: Point {
                x: geom.pos.0 as i32,
                y: geom.pos.1 as i32,
            },
            size: Size {
                width: geom.size.0 as i32,
                height: geom.size.1 as i32,
            },
        };
        //        paint_ctx.inner.frame.fill_rectangle(&c1, &r);
        self.label.paint(paint_ctx, geom);
    }

    fn layout(
        &mut self, bc: &BoxConstraints, children: &[Id], size: Option<(f32, f32)>,
        ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        self.label.layout(bc, children, size, ctx)
    }

    fn mouse(
        &mut self, x: f32, y: f32, mods: u32, which: MouseButton, ty: MouseType,
        ctx: &mut HandlerCtx,
    ) -> bool {
        println!("button {} {} {:x} {:?} {:?}", x, y, mods, which, ty);
        if ty == MouseType::Down {
            ctx.send_event(true);
        }
        true
    }

    fn poke(&mut self, payload: &mut Any, ctx: &mut HandlerCtx) -> bool {
        self.label.poke(payload, ctx)
    }
}
