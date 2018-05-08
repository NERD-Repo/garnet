// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![feature(proc_macro)]

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

use proc_macro2::{Span, Term};
use quote::Tokens;

#[proc_macro]
pub fn zerocopy_inner(input: TokenStream) -> TokenStream {
    // format and remove all spaces to simplify parsing code
    let s = format!("{}", input).replace(" ", "").replace("\n", "");
    println!("{}", s);

    let pipe_index = s.find('|')
        .expect("expected input of the form <struct name>|<fields>");
    let (name, fields) = s.split_at(pipe_index);
    // remove leading |
    let fields = &fields[1..];

    let fields = fields
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|field_str| {
            let (name, ty) =
                if let [name, ty] = field_str.split(':').collect::<Vec<&str>>().as_slice() {
                    (*name, *ty)
                } else {
                    panic!("could not parse {} as name:type", field_str);
                };
            if name.is_empty() || ty.is_empty() {
                panic!("empty name or type");
            }

            let ty = if ty.starts_with('u') {
                let n = ty[1..]
                    .parse::<u8>()
                    .expect(&format!("could not parse {} as bit width", ty));
                Type::U(n)
            } else if ty == "bool" {
                Type::Bool
            } else if ty.starts_with("[u8;") && ty.ends_with("]") {
                let n = ty["[u8;".len()..ty.len() - 1]
                    .parse::<u8>()
                    .expect(&format!("could not parse {} as byte array", ty));
                Type::ByteArray(n)
            } else {
                panic!("unsupported type {}", ty);
            };

            (name, ty)
        })
        .collect::<Vec<_>>();

    let mut offsets = Vec::new();
    let mut bit_offset = 0;
    for (_, ty) in &fields {
        offsets.push((bit_offset / 8, bit_offset % 8));
        bit_offset += match *ty {
            Type::U(n) => n as usize,
            Type::ByteArray(n) => 8 * (n as usize),
            Type::Bool => 1,
        };
    }
    if bit_offset % 8 != 0 {
        panic!("must have an integer number of bytes");
    }
    let bytes = bit_offset / 8;

    let getters_setters = fields.iter().zip(offsets.iter()).map(
        |((name, ty), (byte_offset, bit_offset))| {
            getter_setter(name, *byte_offset, *bit_offset as u8, *ty)
        },
    );

    let name = str_to_tokens(name);
    let tokens = quote!(
        // ensure that the repr is equivalent to [u8; #bytes]
        #[repr(transparent)]
        #[derive(Copy, Clone, Default, Eq, PartialEq)]
        struct #name([u8; #bytes]);
        impl #name { #(#getters_setters)* }
        unsafe impl ::wire::util::Pod for #name {}
    ).into();
    println!("{}", tokens);
    tokens
}

#[derive(Copy, Clone)]
enum Type {
    U(u8),
    ByteArray(u8),
    Bool,
}

impl Type {
    fn name(&self) -> Tokens {
        match *self {
            Type::U(n) => str_to_tokens(&format!("u{}", round_bits_up(n))),
            Type::ByteArray(n) => {
                // array size must be a usize, not a u8
                let n = n as usize;
                quote!([u8; #n])
            }
            Type::Bool => quote!(bool),
        }
    }
}

fn round_bits_up(n: u8) -> u8 {
    assert!(n <= 128);
    if n < 8 {
        8
    } else {
        n.next_power_of_two()
    }
}

/// Create getters and setters for this field.
///
/// Note that `bit_offset_msb` is the offset from the MSB, not the LSB.
fn getter_setter(name: &str, byte_offset: usize, bit_offset_msb: u8, ty: Type) -> Tokens {
    // construct getter and setter bodies for a bool type
    fn bool_bodies(name: &Tokens, byte_offset: usize, bit_offset_msb: u8) -> (Tokens, Tokens) {
        let true_mask = 1u8 << (7 - bit_offset_msb);
        let false_mask = 0xFFu8 - (1u8 << (7 - bit_offset_msb));
        let getter = quote!((self.0[#byte_offset] & #true_mask) != 0);
        let setter = quote!(
            if #name {
                self.0[#byte_offset] |= #true_mask;
            } else {
                self.0[#byte_offset] &= #false_mask;
            }
        );
        (getter, setter)
    }

    // construct getter and setter bodies for a byte array type
    fn byte_array_bodies(name: &Tokens, byte_offset: usize, bit_offset_msb: u8, bytes: u8) -> (Tokens, Tokens) {
        assert_eq!(bit_offset_msb, 0,"we don't support byte slices not on a byte boundary");
        let end = byte_offset + (bytes as usize);
        // array size must be a usize, not a u8
        let bytes = bytes as usize;
        // create a temporary buffer, copy from the slice into that buffer,
        // and then return the buffer
        let getter = quote!(
            let mut buf = [0; #bytes];
            buf.copy_from_slice(&self.0[#byte_offset..#end]);
            buf
        );
        let setter = quote!(
            self.0[#byte_offset..#end].copy_from_slice(&#name);
        );
        (getter, setter)
    }

    // construct getter and setter bodies for a uXXX type
    fn u_bodies(name: &Tokens, byte_offset: usize, bit_offset_msb: u8, bits: u8) -> (Tokens, Tokens) {
        if bits < 8 {
            let trailing_bits = 8 - (bit_offset_msb + bits);
            let mask = ((1u16 << bits) - 1) as u8;
            let inv_shifted_mask = !(mask << trailing_bits);
            let getter = quote!((self.0[#byte_offset] >> #trailing_bits) & #mask);
            let setter = quote!(
                assert!(#name <= #mask);
                let zeroed = self.0[#byte_offset] & #inv_shifted_mask;
                self.0[#byte_offset] = zeroed | (#name << #trailing_bits);
            );
            (getter, setter)
        } else if bits == 8 {
            assert_eq!(bit_offset_msb, 0, "we don't support u8s not on a byte boundary");
            (quote!(self.0[#byte_offset]), quote!(self.0[#byte_offset] = #name))
        } else if bits.is_power_of_two() {
            assert_eq!(bit_offset_msb, 0,"we don't support u{}s not on a byte boundary",bits);
            let read = str_to_tokens(&format!("read_u{}", bits));
            let write = str_to_tokens(&format!("write_u{}", bits));
            let end = byte_offset + ((bits/8) as usize);
            let getter = quote!(
                use byteorder::ByteOrder;
                ::byteorder::BigEndian::#read(&self.0[#byte_offset..#end])
            );
            let setter = quote!(
                use byteorder::ByteOrder;
                ::byteorder::BigEndian::#write(&mut self.0[#byte_offset..#end], #name)
            );
            (getter, setter)
        } else {
            panic!("unsupported bit size: {}", bits);
        }
    }

    let name_tokens = str_to_tokens(name);
    let (getter_body, setter_body) = match ty {
        Type::U(bits) => u_bodies(&name_tokens, byte_offset, bit_offset_msb, bits),
        Type::ByteArray(bytes) => byte_array_bodies(&name_tokens, byte_offset, bit_offset_msb, bytes),
        Type::Bool => bool_bodies(&name_tokens, byte_offset, bit_offset_msb),
    };

    let type_name = ty.name();
    let get_name = str_to_tokens(&("get_".to_owned() + name));
    let set_name = str_to_tokens(&("set_".to_owned() + name));
    quote!(
        #[allow(unused)]
        fn #get_name(&self) -> #type_name { #getter_body }
        #[allow(unused)]
        fn #set_name(&mut self, #name_tokens: #type_name) { #setter_body }
    )
}

/// Convert a string to a `Tokens` which can be used in `quote!`.
///
/// If a string-typed variable is used directly in `quote!`, it will show up in
/// quotes in the resulting AST. E.g., `fn "get_foo" (&self) -> u8 { ... }`.
/// This usually isn't what you want. Instead, you want a `Tokens`, which
/// `quote!` will handle properly.
fn str_to_tokens(s: &str) -> Tokens {
    let mut t = Tokens::new();
    t.append(Term::new(s, Span::call_site()));
    t
}
