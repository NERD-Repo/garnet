/*
 * Copyright (c) 2018 The Fuchsia Authors.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */
#ifndef _SPARSE_ARRAY_H_
#define _SPARSE_ARRAY_H_

#include <stddef.h>
#include <sys/types.h>

struct sparse_array;
typedef struct sparse_array* sparse_array_t;

// Allocate a new sparse array
void sa_init(sparse_array_t* psa, size_t size);

// Deallocate a sparse array
void sa_free(sparse_array_t sa);

// Add an element to a sparse array, returns the index
ssize_t sa_add(sparse_array_t sa, void* payload);

// Get the element at the specified index
void* sa_get(sparse_array_t sa, ssize_t ndx);

// Remove an element from a sparse array
void sa_remove(sparse_array_t sa, ssize_t ndx);

// Call a function on each element in a sparse array
void sa_for_each(sparse_array_t sa, void (*fn)(ssize_t ndx, void* ptr, void* ctx), void* ctx);

#endif
