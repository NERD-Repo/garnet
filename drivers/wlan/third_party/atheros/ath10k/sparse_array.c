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

#include <stdlib.h>

#include "sparse_array.h"

struct sa_elem {
    ssize_t prev_ndx;
    ssize_t next_ndx;
    void* ptr;
};
    
struct sparse_array {
    size_t size;
    ssize_t free;
    ssize_t used;
    struct sa_elem elems[0];
};

void sa_init(sparse_array_t* psa, size_t size) {
    size_t total_size = sizeof(sparse_array_t) + (sizeof(struct sa_elem) * size);
    *psa = calloc(1, total_size);
    if (*psa == NULL) {
        return;
    }

    sparse_array_t sa = *psa;
    sa->size = size;

    // Initialize used list as empty
    sa->used = -1;

    // Add all elements to the free list
    for (ssize_t ndx = 0; ndx < (ssize_t)size; ndx++) {
        sa->elems[ndx].prev_ndx = ndx - 1;
        sa->elems[ndx].next_ndx = ndx + 1;
    }
    sa->free = 0;
}

void sa_free(sparse_array_t sa) {
    free(sa);
}

ssize_t sa_add(sparse_array_t sa, void* payload) {
    if (sa->free == -1) {
        return -1;
    }

    ssize_t elem_ndx = sa->free;
    struct sa_elem* elem = &sa->elems[elem_ndx];

    // Remove from free list
    sa->free = elem->next_ndx;
    if (sa->free != -1) {
        sa->elems[sa->free].prev_ndx = -1;
    }

    // Add to used list
    elem->next_ndx = sa->used;
    if (sa->used != -1) {
        sa->elems[sa->used].next_ndx = elem_ndx;
    }
    sa->used = elem_ndx;

    return elem_ndx;
}

void* sa_get(sparse_array_t sa, ssize_t ndx) {
    return sa->elems[ndx].ptr;
}

void sa_remove(sparse_array_t sa, ssize_t ndx) {
    struct sa_elem* elem = &sa->elems[ndx];
    ssize_t prev_ndx = elem->prev_ndx;
    ssize_t next_ndx = elem->next_ndx;

    // Remove from used list
    if (prev_ndx == -1) {
        sa->used = next_ndx;
    } else {
        struct sa_elem* prev_elem = &sa->elems[prev_ndx];
        prev_elem->next_ndx = next_ndx;
    }
    if (next_ndx != -1) {
        struct sa_elem* next_elem = &sa->elems[next_ndx];
        next_elem->prev_ndx = prev_ndx;
    }

    // Add to free list
    next_ndx = sa->free;
    elem->prev_ndx = -1;
    elem->next_ndx = next_ndx;
    sa->free = ndx;
    if (next_ndx != -1) {
        struct sa_elem* next_elem = &sa->elems[next_ndx];
        next_elem->prev_ndx = ndx;
    }
}

void sa_for_each(sparse_array_t sa, void (*fn)(ssize_t, void*, void*), void* ctx) {
    ssize_t next_ndx = sa->used;
    while (next_ndx != -1) {
        struct sa_elem* elem = &sa->elems[next_ndx];
        fn(next_ndx, elem->ptr, ctx);
        next_ndx = sa->elems[next_ndx].next_ndx;
    }
}
