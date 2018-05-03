/*
 * Copyright 2018 The Fuchsia Authors.
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

#define _ALL_SOURCE
#include <threads.h>

#include "core.h"
#include "debug.h"
#include "hif.h"
#include "msg_buf.h"
#include "wmi-tlv.h"

// Information about our message types. This doesn't have to be in the same order as the
// ath10k_msg_type enums, but in order for the init algorithm to work properly, a type
// must be defined in the init_data array before it appears in an 'isa' field.
#define STR_NAME(name) #name
#define MSG(type, base, hdr) { type, base, hdr, STR_NAME(type) }
static const struct {
    enum ath10k_msg_type type;
    enum ath10k_msg_type isa;
    size_t hdr_size;
    const char* name;
} ath10k_msg_types_init_data[] = {
    {ATH10K_MSG_TYPE_BASE, 0, 0, "ATH10K_MSG_TYPE_BASE"},

    HTC_MSGS,

    // Note that since all of the following use the HTC interface they must follow HTC_MSGS
    WMI_MSGS,
    WMI_TLV_MSGS,
    HTT_MSGS
};
#undef MSG

// Table to keep track of the sizes and types of each message. Once initialized, this data
// is constant so we only keep a single copy. This is perhaps a terrible idea, but it does
// allow us to have a fairly compact representation of the message types in
// ath10k_msg_types_init_data (above), which is the structure most likely to require
// ongoing maintenance.
static struct ath10k_msg_type_info {
    enum ath10k_msg_type isa;
    size_t offset;
    size_t hdr_size;
    const char* name;
} ath10k_msg_types_info[ATH10K_MSG_TYPE_COUNT];
static mtx_t ath10k_msg_types_lock = MTX_INIT;
static bool ath10k_msg_types_initialized = false;

// One-time initialization of the module
zx_status_t ath10k_msg_bufs_init(struct ath10k* ar) {

    // Organize our msg type information into something more usable (an array indexed by msg
    // type, with total size information).
    mtx_lock(&ath10k_msg_types_lock);
    if (!ath10k_msg_types_initialized) {
        for (size_t ndx = 0; ndx < countof(ath10k_msg_types_init_data); ndx++) {
            enum ath10k_msg_type type = ath10k_msg_types_init_data[ndx].type;
            enum ath10k_msg_type parent_type = ath10k_msg_types_init_data[ndx].isa;
            struct ath10k_msg_type_info* type_info = &ath10k_msg_types_info[type];

            type_info->isa = parent_type;
            type_info->offset = ath10k_msg_types_info[parent_type].offset
                                + ath10k_msg_types_info[parent_type].hdr_size;
            type_info->hdr_size = ath10k_msg_types_init_data[ndx].hdr_size;
            type_info->name = ath10k_msg_types_init_data[ndx].name;
        }
        ath10k_msg_types_initialized = true;
    }
    mtx_unlock(&ath10k_msg_types_lock);

    return ZX_OK;
}

zx_status_t ath10k_msg_buf_alloc(struct ath10k* ar,
                                 struct ath10k_msg_buf** msg_buf_ptr,
                                 enum ath10k_msg_type type, size_t extra_bytes) {
    zx_status_t status;

    ZX_DEBUG_ASSERT(type < ATH10K_MSG_TYPE_COUNT);

    struct ath10k_msg_buf* msg_buf;

    // Allocate a new buffer
    msg_buf = calloc(1, sizeof(struct ath10k_msg_buf));
    if (!msg_buf) {
        return ZX_ERR_NO_MEMORY;
    }

    zx_handle_t bti_handle;
    status = ath10k_hif_get_bti_handle(ar, &bti_handle);
    if (status != ZX_OK) {
        goto err_free_buf;
    }
    size_t buf_sz = ath10k_msg_types_info[type].offset
                    + ath10k_msg_types_info[type].hdr_size
                    + extra_bytes;
    status = io_buffer_init(&msg_buf->buf, bti_handle, buf_sz, IO_BUFFER_RW | IO_BUFFER_CONTIG);
    if (status != ZX_OK) {
        goto err_free_buf;
    }

    msg_buf->ar = ar;
    msg_buf->paddr = io_buffer_phys(&msg_buf->buf);
    ZX_DEBUG_ASSERT_MSG(msg_buf->paddr + buf_sz <= 0x100000000,
                        "unable to acquire an io buffer with a 32 bit phys addr (see ZX-1073)");
    msg_buf->vaddr = io_buffer_virt(&msg_buf->buf);
    memset(msg_buf->vaddr, 0, buf_sz);
    msg_buf->capacity = buf_sz;
    msg_buf->type = type;
    list_initialize(&msg_buf->listnode);
    msg_buf->used = msg_buf->capacity;
    *msg_buf_ptr = msg_buf;
    return ZX_OK;

err_free_buf:
    free(msg_buf);
    return status;
}

void* ath10k_msg_buf_get_header(struct ath10k_msg_buf* msg_buf,
                                enum ath10k_msg_type type) {
    return (void*)((uint8_t*)msg_buf->vaddr + ath10k_msg_types_info[type].offset);
}

void* ath10k_msg_buf_get_payload(struct ath10k_msg_buf* msg_buf) {
    enum ath10k_msg_type type = msg_buf->type;
    return (void*)((uint8_t*)msg_buf->vaddr
                   + ath10k_msg_types_info[type].offset
                   + ath10k_msg_types_info[type].hdr_size);
}

size_t ath10k_msg_buf_get_payload_len(struct ath10k_msg_buf* msg_buf,
                                      enum ath10k_msg_type msg_type) {
    return msg_buf->used - ath10k_msg_buf_get_payload_offset(msg_type);
}

size_t ath10k_msg_buf_get_offset(enum ath10k_msg_type type) {
    return ath10k_msg_types_info[type].offset;
}

size_t ath10k_msg_buf_get_payload_offset(enum ath10k_msg_type type) {
    return ath10k_msg_types_info[type].offset + ath10k_msg_types_info[type].hdr_size;
}

void ath10k_msg_buf_free(struct ath10k_msg_buf* msg_buf) {
    enum ath10k_msg_type type = msg_buf->type;
    ZX_DEBUG_ASSERT(type < ATH10K_MSG_TYPE_COUNT);
    msg_buf->used = 0;
    io_buffer_release(&msg_buf->buf);
    free(msg_buf);
}

void ath10k_msg_buf_dump(struct ath10k_msg_buf* msg_buf, const char* prefix) {
    uint8_t* raw_data = msg_buf->vaddr;
    ath10k_info("msg_buf (%s): paddr %#x\n",
                ath10k_msg_types_info[msg_buf->type].name,
                (unsigned int)msg_buf->paddr);
    unsigned ndx;
    for (ndx = 0; msg_buf->used - ndx >= 4; ndx += 4) {
        ath10k_info("%s0x%02x 0x%02x 0x%02x 0x%02x\n", prefix,
                    raw_data[ndx], raw_data[ndx + 1], raw_data[ndx + 2], raw_data[ndx + 3]);
    }
    if (ndx != msg_buf->used) {
        ath10k_err("%sBuffer has %d bytes extra\n", prefix, (int)(msg_buf->used - ndx));
    }
}
