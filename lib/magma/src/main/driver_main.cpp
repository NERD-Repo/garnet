// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mx/channel.h"
#include <ddk/binding.h>
#include <ddk/device.h>
#include <ddk/driver.h>
#include <ddk/protocol/display.h>
#include <ddk/protocol/pci.h>
#include <hw/pci.h>

#include <atomic>
#include <magenta/process.h>
#include <magenta/types.h>
#include <thread>

#include "magma_util/dlog.h"
#include "magma_util/platform/magenta/magenta_platform_ioctl.h"
#include "magma_util/platform/magenta/magenta_platform_trace.h"
#include "sys_driver/magma_driver.h"
#include "sys_driver/magma_system_buffer.h"

#if MAGMA_TEST_DRIVER
void magma_indriver_test(mx_device_t* device);
#endif

#define INTEL_I915_VID (0x8086)

struct intel_i915_device_t {
    mx_device_t* mxdev;
    mx_device_t* parent_device;

    void* framebuffer_addr;
    uint64_t framebuffer_size;

    mx_display_info_t info;
    uint32_t flags;

    mx_display_cb_t ownership_change_callback{nullptr};
    void* ownership_change_cookie{nullptr};

    std::unique_ptr<magma::PlatformBuffer> console_buffer;
    std::unique_ptr<magma::PlatformBuffer> placeholder_buffer;
    std::unique_ptr<MagmaDriver> magma_driver;
    std::shared_ptr<MagmaSystemDevice> magma_system_device;
    std::shared_ptr<MagmaSystemBuffer> console_framebuffer;
    std::shared_ptr<MagmaSystemBuffer> placeholder_framebuffer;
    std::mutex magma_mutex;
    std::atomic_bool console_visible{true};
};

static int magma_start(intel_i915_device_t* dev);
static int magma_stop(intel_i915_device_t* dev);

intel_i915_device_t* get_i915_device(void* context)
{
    return static_cast<intel_i915_device_t*>(context);
}

static void intel_i915_enable_backlight(intel_i915_device_t* dev, bool enable)
{
    // Take action on backlight here for certain platforms as necessary.
}

// implement display protocol

static mx_status_t intel_i915_set_mode(mx_device_t* dev, mx_display_info_t* info)
{
    return MX_ERR_NOT_SUPPORTED;
}

static mx_status_t intel_i915_get_mode(mx_device_t* dev, mx_display_info_t* info)
{
    assert(info);
    intel_i915_device_t* device = get_i915_device(dev->ctx);
    memcpy(info, &device->info, sizeof(mx_display_info_t));
    return MX_OK;
}

static mx_status_t intel_i915_get_framebuffer(mx_device_t* dev, void** framebuffer)
{
    assert(framebuffer);
    intel_i915_device_t* device = get_i915_device(dev->ctx);
    (*framebuffer) = device->framebuffer_addr;
    return MX_OK;
}

#define CACHELINE_SIZE 64
#define CACHELINE_MASK 63

static inline void clflush_range(void* start, size_t size)
{
    DLOG("clflush_range");

    uint8_t* p = reinterpret_cast<uint8_t*>(reinterpret_cast<uintptr_t>(start) & ~CACHELINE_MASK);
    uint8_t* end = reinterpret_cast<uint8_t*>(start) + size;

    __builtin_ia32_mfence();
    while (p < end) {
        __builtin_ia32_clflush(p);
        p += CACHELINE_SIZE;
    }
}

static void intel_i915_flush(mx_device_t* dev)
{
    intel_i915_device_t* device = get_i915_device(dev->ctx);
    // Don't incur overhead of flushing when console's not visible
    if (device->console_visible)
        clflush_range(device->framebuffer_addr, device->framebuffer_size);
}

static void intel_i915_acquire_or_release_display(mx_device_t* dev, bool acquire)
{
    intel_i915_device_t* device = get_i915_device(dev->ctx);
    DLOG("intel_i915_acquire_or_release_display");

    std::unique_lock<std::mutex> lock(device->magma_mutex);

    if (acquire && device->magma_system_device->page_flip_enabled()) {
        DLOG("flipping to console");
        // Ensure any software writes to framebuffer are visible
        device->console_visible = true;
        if (device->ownership_change_callback)
            device->ownership_change_callback(true, device->ownership_change_cookie);
        clflush_range(device->framebuffer_addr, device->framebuffer_size);
        magma_system_image_descriptor image_desc{MAGMA_IMAGE_TILING_LINEAR};
        auto last_framebuffer = device->magma_system_device->PageFlipAndEnable(
            device->console_framebuffer, &image_desc, false);
        if (last_framebuffer)
            device->placeholder_framebuffer = last_framebuffer;
    } else if (!acquire && !device->magma_system_device->page_flip_enabled()) {
        DLOG("flipping to placeholder_framebuffer");
        magma_system_image_descriptor image_desc{MAGMA_IMAGE_TILING_OPTIMAL};
        device->magma_system_device->PageFlipAndEnable(device->placeholder_framebuffer, &image_desc,
                                                       true);
        device->console_visible = false;
        if (device->ownership_change_callback)
            device->ownership_change_callback(false, device->ownership_change_cookie);
    }
}

static void intel_i915_set_ownership_change_callback(mx_device_t* dev, mx_display_cb_t callback,
                                                     void* cookie)
{
    intel_i915_device_t* device = get_i915_device(dev->ctx);
    std::unique_lock<std::mutex> lock(device->magma_mutex);
    device->ownership_change_callback = callback;
    device->ownership_change_cookie = cookie;
}

static mx_display_protocol_t intel_i915_display_proto = {
    .set_mode = intel_i915_set_mode,
    .get_mode = intel_i915_get_mode,
    .get_framebuffer = intel_i915_get_framebuffer,
    .acquire_or_release_display = intel_i915_acquire_or_release_display,
    .set_ownership_change_callback = intel_i915_set_ownership_change_callback,
    .flush = intel_i915_flush,
};

// implement device protocol

static mx_status_t intel_i915_open(void* ctx, mx_device_t** out, uint32_t flags)
{
    intel_i915_device_t* device = get_i915_device(ctx);
    intel_i915_enable_backlight(device, true);
    return MX_OK;
}

static mx_status_t intel_i915_close(void* ctx, uint32_t flags) { return MX_OK; }

static int reset_placeholder(intel_i915_device_t* device)
{
    void* addr;
    if (device->placeholder_buffer->MapCpu(&addr)) {
        memset(addr, 0, device->placeholder_buffer->size());
        clflush_range(addr, device->placeholder_buffer->size());
        device->placeholder_buffer->UnmapCpu();
    }

    uint32_t buffer_handle;
    if (!device->placeholder_buffer->duplicate_handle(&buffer_handle))
        return DRET_MSG(MX_ERR_NO_RESOURCES, "duplicate_handle failed");

    device->placeholder_framebuffer =
        MagmaSystemBuffer::Create(magma::PlatformBuffer::Import(buffer_handle));
    if (!device->placeholder_framebuffer)
        return DRET_MSG(MX_ERR_NO_MEMORY, "failed to created magma system buffer");

    return MX_OK;
}

static mx_status_t intel_i915_ioctl(void* ctx, uint32_t op, const void* in_buf, size_t in_len,
                                    void* out_buf, size_t out_len, size_t* out_actual)
{
    intel_i915_device_t* device = get_i915_device(ctx);

    DASSERT(device->magma_system_device);

    ssize_t result = MX_ERR_NOT_SUPPORTED;

    switch (op) {
        case IOCTL_MAGMA_QUERY: {
            DLOG("IOCTL_MAGMA_QUERY");
            const uint64_t* param = reinterpret_cast<const uint64_t*>(in_buf);
            if (!in_buf || in_len < sizeof(*param))
                return DRET_MSG(MX_ERR_INVALID_ARGS, "bad in_buf");
            uint64_t* value_out = reinterpret_cast<uint64_t*>(out_buf);
            if (!out_buf || out_len < sizeof(*value_out))
                return DRET_MSG(MX_ERR_INVALID_ARGS, "bad out_buf");
            switch (*param) {
                case MAGMA_QUERY_DEVICE_ID:
                    *value_out = device->magma_system_device->GetDeviceId();
                    break;
                default:
                    if (!device->magma_system_device->Query(*param, value_out))
                        return DRET_MSG(MX_ERR_INVALID_ARGS, "unhandled param 0x%" PRIx64,
                                        *value_out);
            }
            DLOG("query param 0x%" PRIx64 " returning 0x%" PRIx64, *param, *value_out);
            *out_actual = sizeof(*value_out);
            result = MX_OK;
            break;
        }
        case IOCTL_MAGMA_CONNECT: {
            DLOG("IOCTL_MAGMA_CONNECT");
            auto request = reinterpret_cast<const magma_system_connection_request*>(in_buf);
            if (!in_buf || in_len < sizeof(*request))
                return DRET(MX_ERR_INVALID_ARGS);

            auto device_handle_out = reinterpret_cast<uint32_t*>(out_buf);
            if (!out_buf || out_len < sizeof(*device_handle_out))
                return DRET(MX_ERR_INVALID_ARGS);

            // Override console for new display connections
            if (request->capabilities & MAGMA_CAPABILITY_DISPLAY) {
                reset_placeholder(device);
                magma_system_image_descriptor image_desc{MAGMA_IMAGE_TILING_OPTIMAL};
                device->magma_system_device->PageFlipAndEnable(device->placeholder_framebuffer,
                                                               &image_desc, true);
                device->console_visible = false;
                if (device->ownership_change_callback)
                    device->ownership_change_callback(false, device->ownership_change_cookie);
            }

            auto connection = MagmaSystemDevice::Open(device->magma_system_device,
                                                      request->client_id, request->capabilities);
            if (!connection)
                return DRET(MX_ERR_INVALID_ARGS);

            *device_handle_out = connection->GetHandle();
            *out_actual = sizeof(*device_handle_out);
            result = MX_OK;

            device->magma_system_device->StartConnectionThread(std::move(connection));

            break;
        }

        case IOCTL_MAGMA_DUMP_STATUS: {
            DLOG("IOCTL_MAGMA_DUMP_STATUS");
            std::unique_lock<std::mutex> lock(device->magma_mutex);
            intel_i915_device_t* device = get_i915_device(ctx);
            if (device->magma_system_device)
                device->magma_system_device->DumpStatus();
            result = MX_OK;
            break;
        }

        case IOCTL_DISPLAY_GET_FB: {
            DLOG("MAGMA IOCTL_DISPLAY_GET_FB");
            if (out_len < sizeof(ioctl_display_get_fb_t))
                return DRET(MX_ERR_INVALID_ARGS);
            ioctl_display_get_fb_t* description = static_cast<ioctl_display_get_fb_t*>(out_buf);
            device->console_buffer->duplicate_handle(
                reinterpret_cast<uint32_t*>(&description->vmo));
            description->info = device->info;
            *out_actual = sizeof(ioctl_display_get_fb_t);
            result = MX_OK;
            break;
        }

#if MAGMA_TEST_DRIVER
        case IOCTL_MAGMA_TEST_RESTART: {
            DLOG("IOCTL_MAGMA_TEST_RESTART");
            std::unique_lock<std::mutex> lock(device->magma_mutex);
            result = magma_stop(device);
            if (result != MX_OK)
                return DRET_MSG(result, "magma_stop failed");
            result = magma_start(device);
            break;
        }
#endif

        case IOCTL_MAGMA_DISPLAY_GET_SIZE: {
            DLOG("IOCTL_MAGMA_DISPLAY_GET_SIZE");
            if (in_len != 0)
                return DRET_MSG(MX_ERR_INVALID_ARGS, "bad in_buf");
            auto* value_out = static_cast<magma_display_size*>(out_buf);
            if (!out_buf || out_len < sizeof(*value_out))
                return DRET_MSG(MX_ERR_INVALID_ARGS, "bad out_buf");

            std::unique_lock<std::mutex> lock(device->magma_mutex);
            if (device->magma_system_device) {
                if (msd_device_display_get_size(device->magma_system_device->msd_dev(),
                                                value_out) == MAGMA_STATUS_OK) {
                    result = sizeof(*value_out);
                }
            }
            break;
        }

        default:
            DLOG("intel_i915_ioctl unhandled op 0x%x", op);
    }

    return result;
}

static void intel_i915_release(void* ctx)
{
    DLOG("intel_i915_release");
    intel_i915_device_t* device = get_i915_device(ctx);

    std::unique_lock<std::mutex> lock(device->magma_mutex);

    intel_i915_enable_backlight(device, false);

    magma_stop(device);

    delete (device);
}

static mx_protocol_device_t intel_i915_device_proto = {
    .version = DEVICE_OPS_VERSION,
    .open = intel_i915_open,
    .close = intel_i915_close,
    .ioctl = intel_i915_ioctl,
    .release = intel_i915_release,
};

// implement driver object:

static mx_status_t intel_i915_bind(void* ctx, mx_device_t* mx_device, void** cookie)
{
    DLOG("intel_i915_bind start mx_device %p", mx_device);

    pci_protocol_t* pci;
    if (device_op_get_protocol(mx_device, MX_PROTOCOL_PCI, (void**)&pci))
        return DRET_MSG(MX_ERR_NOT_SUPPORTED, "device_op_get_protocol failed");

    mx_status_t status = pci->claim_device(mx_device);
    if (status < 0)
        return DRET_MSG(status, "claim_device failed");

    // map resources and initialize the device
    auto device = std::make_unique<intel_i915_device_t>();

    mx_display_info_t* di = &device->info;
    uint32_t format, width, height, stride, pitch;
    status = mx_bootloader_fb_get_info(&format, &width, &height, &stride);
    if (status == MX_OK) {
        di->format = format;
        di->width = width;
        di->height = height;
        di->stride = stride;
    } else {
        di->format = MX_PIXEL_FORMAT_ARGB_8888;
        di->width = 2560 / 2;
        di->height = 1700 / 2;
        di->stride = 2560 / 2;
    }

    switch (di->format) {
        case MX_PIXEL_FORMAT_RGB_565:
            pitch = di->stride * sizeof(uint16_t);
            break;
        default:
            DLOG("unrecognized format 0x%x, defaulting to 32bpp", di->format);
        case MX_PIXEL_FORMAT_ARGB_8888:
        case MX_PIXEL_FORMAT_RGB_x888:
            pitch = di->stride * sizeof(uint32_t);
            break;
    }

    device->framebuffer_size = pitch * di->height;

    device->console_buffer =
        magma::PlatformBuffer::Create(device->framebuffer_size, "console-buffer");

    if (!device->console_buffer->MapCpu(&device->framebuffer_addr))
        return DRET_MSG(MX_ERR_NO_MEMORY, "Failed to map framebuffer");

    // Placeholder is in tiled format
    device->placeholder_buffer = magma::PlatformBuffer::Create(
        magma::round_up(pitch, 512) * di->height, "placeholder-buffer");

    di->flags = MX_DISPLAY_FLAG_HW_FRAMEBUFFER;

    // Tell the kernel about the console framebuffer so it can display a kernel panic screen.
    // If other display clients come along and change the scanout address, then the panic
    // won't be visible; however the plan is to move away from onscreen panics, instead
    // writing the log somewhere it can be recovered then triggering a reboot.
    uint32_t handle;
    if (!device->console_buffer->duplicate_handle(&handle))
        return DRET_MSG(MX_ERR_INTERNAL, "Failed to duplicate framebuffer handle");

    status = mx_set_framebuffer_vmo(get_root_resource(), handle, device->framebuffer_size, format,
                                    width, height, stride);
    if (status != MX_OK)
        magma::log(magma::LOG_WARNING, "Failed to pass framebuffer to magenta: %d", status);

    // TODO remove when the gfxconsole moves to user space
    intel_i915_enable_backlight(device.get(), true);

    magma::PlatformTrace::Initialize();

    device->magma_driver = MagmaDriver::Create();
    if (!device->magma_driver)
        return DRET_MSG(MX_ERR_INTERNAL, "MagmaDriver::Create failed");

#if MAGMA_TEST_DRIVER
    DLOG("running magma indriver test");
    magma_indriver_test(mx_device);
#endif

    device->parent_device = mx_device;

    status = magma_start(device.get());
    if (status != MX_OK)
        return DRET_MSG(status, "magma_start failed");

    device_add_args_t args = {};
    args.version = DEVICE_ADD_ARGS_VERSION;
    args.name = "intel_i915_disp";
    args.ctx = device.get();
    args.ops = &intel_i915_device_proto;
    args.proto_id = MX_PROTOCOL_DISPLAY;
    args.proto_ops = &intel_i915_display_proto;

    status = device_add(mx_device, &args, &device->mxdev);
    if (status != MX_OK)
        return DRET_MSG(status, "device_add failed");

    device.release();

    DLOG("initialized magma intel display driver");

    return MX_OK;
}

static mx_driver_ops_t intel_gen_gpu_driver_ops = {
    .version = DRIVER_OPS_VERSION, .bind = intel_i915_bind,
};

// clang-format off
MAGENTA_DRIVER_BEGIN(intel_gen_gpu, intel_gen_gpu_driver_ops, "magenta", "!0.1", 5)
    BI_ABORT_IF(NE, BIND_PROTOCOL, MX_PROTOCOL_PCI),
    BI_ABORT_IF(NE, BIND_PCI_VID, INTEL_I915_VID),
    BI_MATCH_IF(EQ, BIND_PCI_CLASS, 0x3), // Display class
MAGENTA_DRIVER_END(intel_gen_gpu)
    // clang-format on

    static int magma_start(intel_i915_device_t* device)
{
    DLOG("magma_start");

    device->magma_system_device = device->magma_driver->CreateDevice(device->parent_device);
    if (!device->magma_system_device)
        return DRET_MSG(MX_ERR_NO_RESOURCES, "Failed to create device");

    DLOG("Created device %p", device->magma_system_device.get());

    DASSERT(device->console_buffer);
    DASSERT(device->placeholder_buffer);

    uint32_t buffer_handle;

    if (!device->console_buffer->duplicate_handle(&buffer_handle))
        return DRET_MSG(MX_ERR_NO_RESOURCES, "duplicate_handle failed");

    device->console_framebuffer =
        MagmaSystemBuffer::Create(magma::PlatformBuffer::Import(buffer_handle));
    if (!device->console_framebuffer)
        return DRET_MSG(MX_ERR_NO_MEMORY, "failed to created magma system buffer");

    int result = reset_placeholder(device);
    if (result != 0)
        return result;

    magma_system_image_descriptor image_desc{MAGMA_IMAGE_TILING_LINEAR};
    device->magma_system_device->PageFlipAndEnable(device->console_framebuffer, &image_desc, false);

    return MX_OK;
}

static int magma_stop(intel_i915_device_t* device)
{
    DLOG("magma_stop");

    device->console_framebuffer.reset();
    device->placeholder_framebuffer.reset();

    device->magma_system_device->Shutdown();
    device->magma_system_device.reset();

    return MX_OK;
}
