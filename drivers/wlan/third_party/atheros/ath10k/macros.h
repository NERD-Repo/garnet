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

#pragma once

#include <zircon/assert.h>

#include <stdio.h>

#define ASSERT_MTX_HELD(mtx) ZX_ASSERT(mtx_trylock(mtx) != thrd_success)

// JMC
#define BITMAP_TYPE uint64_t

#define BITMAP_TYPE_NUM_BITS (sizeof(BITMAP_TYPE) * 8)

#define DECLARE_BITMAP(name, size) \
    BITMAP_TYPE name[DIV_ROUND_UP(size, BITMAP_TYPE_NUM_BITS)]

#define DIV_ROUND_UP(n, m) (((n) + ((m) - 1)) / (m))

#define ETHTOOL_FWVERS_LEN 32

#define GENMASK1(val) ((1UL << (val)) - 1)
#define GENMASK(start, end) ((GENMASK1((start) + 1) & ~GENMASK1(end)))

#define IS_ALIGNED(ptr, alignment) (((uintptr_t)(ptr) & (uintptr_t)((alignment) - 1)) == 0)

#define WARN(cond, filename, lineno) \
    printf("ath10k: unexpected condition %s at %s:%d\n", cond, filename, lineno)

#define WARN_ON(cond)                           \
    ({                                          \
        bool result = cond;                     \
        if (result) {                           \
            WARN(#cond, __FILE__, __LINE__);    \
        }                                       \
        result;                                 \
    })

#define WARN_ON_ONCE(cond)                      \
    ({                                          \
        static bool warn_next = true;           \
        bool result = cond;                     \
        if (result && warn_next) {              \
            WARN(#cond, __FILE__, __LINE__);    \
            warn_next = false;                  \
        }                                       \
        result;                                 \
    })

#define clear_bit(pos, field) \
    field[(pos) / BITMAP_TYPE_NUM_BITS] &= ~((BITMAP_TYPE)1 << ((pos) % BITMAP_TYPE_NUM_BITS))

#define ether_addr_copy(e1, e2) memcpy(e1, e2, ETH_ALEN)

#define ilog2(val)  \
    (((val) == 0) ? 0 : (((sizeof(unsigned long long) * 8) - 1) - __builtin_clzll(val)))

#define iowrite32(value, addr)                                  \
    do {                                                        \
        (*(volatile uint32_t*)(uintptr_t)(addr)) = (value);     \
    } while (0)

#define ioread32(addr) (*(volatile uint32_t*)(uintptr_t)(addr))

#define is_power_of_2(x) (((x) & ((x) - 1)) == 0)

#define mdelay(msecs)                                                                    \
    do {                                                                                 \
            zx_time_t busy_loop_end = zx_clock_get(ZX_CLOCK_MONOTONIC) + ZX_MSEC(msecs); \
        while (zx_clock_get(ZX_CLOCK_MONOTONIC) < busy_loop_end) {                       \
        }                                                                                \
    } while (0)

#define min(a,b) (((a) < (b)) ? (a) : (b))
#define min_t(t,a,b) (((t)(a) < (t)(b)) ? (t)(a) : (t)(b))

#define rounddown(n,m) ((n) - ((n) % (m)))
#define roundup(n,m) (((n) % (m) == 0) ? (n) : (n) + ((m) - ((n) % (m))))

// round_up only supports powers of two, so it can be implemented more efficiently, some day...
#define round_up(n,m) roundup(n,m)

#define roundup_pow_of_two(val) \
    ((unsigned long) (val) == 0 ? (val) : \
             1UL << ((sizeof(unsigned long) * 8) - __builtin_clzl((val) - 1)))

/* Not actually a linuxism, but closely related to the previous definition */
#define roundup_log2(val) \
    ((unsigned long) (val) == 0 ? (val) : \
             ((sizeof(unsigned long) * 8) - __builtin_clzl((val) - 1)))

#define scnprintf(buf, size, format, ...)                           \
    ({                                                              \
        int result = snprintf(buf, size, format, __VA_ARGS__);      \
        min_t(int, size, result);                                   \
    })

#define set_bit(pos, field) \
    field[(pos) / BITMAP_TYPE_NUM_BITS] |= ((BITMAP_TYPE)1 << ((pos) % BITMAP_TYPE_NUM_BITS))

#define test_bit(pos, field) \
    ((field[(pos) / BITMAP_TYPE_NUM_BITS] & \
      ((BITMAP_TYPE)1 << ((pos) % BITMAP_TYPE_NUM_BITS))) != 0)

