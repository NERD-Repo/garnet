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

#define ARRAY_SIZE(arr) \
    (sizeof(arr) / sizeof(arr[0]))

#define BIT(pos) (1UL << (pos))

#define DIV_ROUND_UP(n, m) (((n) + ((m) - 1)) / (m))

#define ETHTOOL_FWVERS_LEN 32

#define GENMASK1(val) ((1UL << (val)) - 1)
#define GENMASK(start, end) ((GENMASK1((start) + 1) & ~GENMASK1(end)))

#define LOCK_ASSERT_HELD(lock)                                              \
    do {                                                                    \
        int res = mtx_trylock(lock);                                        \
        ZX_ASSERT(res != 0);                                                \
        if (res == 0) {                                                     \
            printf("ath10k: lock not held at %s:%d\n", __FILE__, __LINE__); \
            mtx_unlock(lock);                                               \
        }                                                                   \
    } while (0)

#define WARN(cond, filename, lineno) \
    printf("ath10k: unexpected condition %s at %s:%d\n", cond, filename, lineno)

#define WARN_ON(cond)                       \
    ({                          \
        if (cond) {                 \
            WARN(#cond, __FILE__, __LINE__);    \
        }                       \
        cond;                       \
    })

#define WARN_ON_ONCE(cond)                  \
    ({                          \
        static bool warn_next = true;           \
        if (cond && warn_next) {            \
            WARN(#cond, __FILE__, __LINE__);    \
            warn_next = false;          \
        }                       \
        cond;                       \
    })

#define ilog2(val)  \
    (((val) == 0) ? 0 : (((sizeof(unsigned long long) * 8) - 1) - __builtin_clzll(val)))

#define iowrite32(value, addr)                          \
    do {                                    \
        (*(volatile uint32_t*)(uintptr_t)(addr)) = (value);     \
    } while (0)

#define ioread32(addr) (*(volatile uint32_t*)(uintptr_t)(addr))

#define lockdep_assert_held(mtx)    \
    ZX_ASSERT(mtx_trylock(mtx) != thrd_success)

#define mdelay(msecs)                                       \
    do {                                            \
            zx_time_t busy_loop_end = zx_clock_get(ZX_CLOCK_MONOTONIC) + ZX_MSEC(msecs); \
        while (zx_clock_get(ZX_CLOCK_MONOTONIC) < busy_loop_end) {           \
        }                                       \
    } while (0)

#define min(a,b) (((a) < (b)) ? (a) : (b))
#define min_t(t,a,b) (((t)(a) < (t)(b)) ? (t)(a) : (t)(b))

#define __packed __attribute__((packed))
#define __aligned(n) __attribute__((aligned(n)))

#define rounddown(n,m) ((n) - ((n) % (m)))
#define roundup(n,m) (((n) % (m) == 0) ? (n) : (n) + ((m) - ((n) % (m))))

#define roundup_pow_of_two(val) \
    ((unsigned long) (val) == 0 ? (val) : \
             1UL << ((sizeof(unsigned long) * 8) - __builtin_clzl((val) - 1)))

/* Not actually a linuxism, but closely related to the previous definition */
#define roundup_log2(val) \
    ((unsigned long) (val) == 0 ? (val) : \
             ((sizeof(unsigned long) * 8) - __builtin_clzl((val) - 1)))

