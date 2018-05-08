// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// pub use zerocopy_macro::zerocopy_inner;

macro_rules! zerocopy {
    (struct $name:ident {
        $($field_name:ident: $field_type:ty,)*
    }) => (
        ::wire::macros::zerocopy_inner!($name|$($field_name: $field_type,)*);
    );
}

#[cfg(test)]
mod tests {
    // zerocopy!(struct Foo {
    //     a: u8,
    //     b: u16,
    //     c: u3,
    //     d: u2,
    //     e: bool,
    //     f: bool,
    //     g: bool,
    // });

    // #[test]
    // #[should_panic]
    // fn test_panic() {
    //     let mut foo = Foo::default();
    //     foo.set_d(4);
    // }
}
