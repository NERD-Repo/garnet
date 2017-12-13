// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <stdio.h>

#include <utility>

#include <fdio/io.h>
#include <fcntl.h>
#include <unistd.h>
#include <zx/vmar.h>
#include <zx/vmo.h>

#include "buffer.h"

// Buffer *Buffer::NewBuffer(uint32_t width, uint32_t height) {
Buffer *Buffer::NewBuffer(uint64_t buffer_size, const zx::vmo &main_buffer, uint64_t offset, uint32_t index) {
    zx::vmo vmo;
    zx_status_t err = main_buffer.duplicate(ZX_RIGHT_SAME_RIGHTS, &vmo);
    if (err != ZX_OK) {
      FXL_LOG(ERROR) << "Failed to duplicate vmo (status: " << err << ").";
      return nullptr;
    }

    zx::event acquire_fence;
    err = zx::event::create(0, &acquire_fence);
    if (err != ZX_OK) {
        printf("Failed to create acquire_fence.\n");
        return nullptr;
    }

    zx::event release_fence;
    err = zx::event::create(0, &release_fence);
    if (err != ZX_OK) {
        printf("Failed to create release_fence.\n");
        return nullptr;
    }
    release_fence.signal(0, ZX_EVENT_SIGNALED);

    uintptr_t ptr;
    err = zx::vmar::root_self().map(
        0, vmo, offset, buffer_size,
        ZX_VM_FLAG_PERM_READ | ZX_VM_FLAG_PERM_WRITE,
        &ptr);
    if (err != ZX_OK) {
        printf("Can't map vmo.\n");
        return nullptr;
    }

    Buffer *b = new Buffer();

    b->vmo_ = std::move(vmo);
    b->pixels_ = reinterpret_cast<uint32_t *>(ptr);
    b->size_ = buffer_size;
    b->vmo_offset_ = offset;
    b->index_ = index;

    b->acquire_fence_ = std::move(acquire_fence);
    b->release_fence_ = std::move(release_fence);

    return b;
}

Buffer::~Buffer() {
    zx::vmar::root_self().unmap(reinterpret_cast<uintptr_t>(pixels_), size_);
}

void Buffer::FillARGB(uint8_t r, uint8_t g, uint8_t b) {
    uint32_t color = 0xff << 24 | r << 16 | g << 8 | b;
    uint32_t num_pixels = size_ / 4;
    for (unsigned int i = 0; i < num_pixels; i++) {
        pixels_[i] = color;
    }
    // FXL_LOG(INFO) << "Calling op_range ";

    // The zircon kernel has a bug where it does a full cache flush for every
    // page.  ZX-806.
    // TODO(MA-277): Replace the hard coded 4096 with size_ once the above bug
    // is fixed.
    vmo_.op_range(ZX_VMO_OP_CACHE_CLEAN, 0, 4096, nullptr, 0);
}

zx_status_t Buffer::SaveToFile(const char *filename) {
    int fd = ::open(filename, O_RDWR | O_CREAT);
    if (fd < 0) {
        printf("Failed to open \"%s\" (res %d)\n", filename, fd);
        return fd;
    }
    write(fd, pixels_, size_);
    close(fd);
    return ZX_OK;
}


uint8_t clip(int in) {
    uint32_t out = in < 0 ? 0 : (uint32_t)in;
    return out > 255 ? 255 : (out & 0xff);
}

uint8_t gamma_1_4[] = {0, 21, 28, 34, 39, 43, 47, 50, 53, 56, 59, 62, 64, 66, 69, 71, 73, 75, 77, 79, 81, 82, 84, 86, 88, 89, 91, 92, 94, 95, 97, 98, 100, 101, 103, 104, 105, 107, 108, 109, 110, 112, 113, 114, 115, 116, 118, 119, 120, 121, 122, 123, 124, 125, 126, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151, 152, 153, 154, 154, 155, 156, 157, 158, 159, 159, 160, 161, 162, 162, 163, 164, 165, 166, 166, 167, 168, 169, 169, 170, 171, 172, 172, 173, 174, 174, 175, 176, 177, 177, 178, 179, 179, 180, 181, 181, 182, 183, 183, 184, 185, 185, 186, 187, 187, 188, 189, 189, 190, 191, 191, 192, 193, 193, 194, 195, 195, 196, 196, 197, 198, 198, 199, 199, 200, 201, 201, 202, 202, 203, 204, 204, 205, 205, 206, 207, 207, 208, 208, 209, 209, 210, 211, 211, 212, 212, 213, 213, 214, 215, 215, 216, 216, 217, 217, 218, 218, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224, 225, 225, 226, 226, 227, 227, 228, 228, 229, 230, 230, 231, 231, 232, 232, 233, 233, 234, 234, 235, 235, 236, 236, 237, 237, 238, 238, 239, 239, 240, 240, 240, 241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246, 246, 247, 247, 248, 248, 249, 249, 249, 250, 250, 251, 251, 252, 252, 253, 253, 254, 254, 254, 255};

uint8_t gamma_1[] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255};

uint8_t gamma2_5[] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 23, 23, 24, 24, 25, 26, 26, 27, 28, 28, 29, 30, 30, 31, 32, 33, 33, 34, 35, 36, 36, 37, 38, 39, 40, 40, 41, 42, 43, 44, 45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 71, 72, 74, 75, 76, 77, 78, 80, 81, 82, 83, 85, 86, 87, 89, 90, 91, 93, 94, 96, 97, 98, 100, 101, 103, 104, 106, 107, 108, 110, 112, 113, 115, 116, 118, 119, 121, 122, 124, 126, 127, 129, 131, 132, 134, 136, 138, 139, 141, 143, 145, 146, 148, 150, 152, 154, 155, 157, 159, 161, 163, 165, 167, 169, 171, 173, 175, 177, 179, 181, 183, 185, 187, 189, 191, 193, 195, 197, 200, 202, 204, 206, 208, 211, 213, 215, 217, 220, 222, 224, 226, 229, 231, 233, 236, 238, 241, 243, 246, 248, 250, 253};

// Takes 4 bytes of YUY2 and writes 8 bytes of RGBA
void Yuv422ToRGBA2(uint8_t *yuy2, uint8_t *rgba1, uint8_t *rgba2, int gamma = 0) {
    uint8_t *correction = gamma_1;
    if (gamma < 0) correction = gamma2_5;
    if (gamma > 0) correction = gamma_1_4;
    int u  = yuy2[1] - 128;
    int y1 = 298 * (yuy2[0] - 16);
    int v  = yuy2[3] - 128;
    int y2 = 298 * (yuy2[2] - 16);
    rgba1[2] = correction[clip(((y1 + 409 * v + 128 ) /  256))];
    rgba1[1] = correction[clip(((y1 - 208 * v - 100 * u + 128 ) / 256))];
    rgba1[0] = correction[clip(((y1 + 516 * u + 128 ) / 256))];
    rgba1[3] = 0xff;

    rgba2[2] = correction[clip(((y2 + 409 * v + 128 ) / 256))];
    rgba2[1] = correction[clip(((y2 - 208 * v - 100 * u + 128 ) / 256))];
    rgba2[0] = correction[clip(((y2 + 516 * u + 128 ) / 256))];
    rgba2[3] = 0xff;
}

void Buffer::ConvertToRGBA(Buffer *b, int gamma) {
    // converts to BGRA
    uint32_t num_pixels = size_ / 8; // size in output buffer
    for (unsigned int i = 0; i < num_pixels; i++) {
        Yuv422ToRGBA2(reinterpret_cast<uint8_t*>(&b->pixels_[i]), 
                     reinterpret_cast<uint8_t*>(&pixels_[2 * i]),
                     reinterpret_cast<uint8_t*>(&pixels_[2 * i + 1]), gamma);
    }
}

void Buffer::ConvertToBgraAndMirror(Buffer *b, uint32_t width, int gamma) {
    width /= 2;
    // converts to BGRA and mirrors left-right
    uint32_t num_pixels = size_ / 8; // size is in output buffer, which is 2X
    uint32_t height = num_pixels / width;
    for (uint32_t y = 0; y < height; ++y)
    for (uint32_t x = 0; x < width; ++x) {
        uint64_t out = 2 * ((width - 1 - x) + y * width);
        Yuv422ToRGBA2(reinterpret_cast<uint8_t*>(&b->pixels_[x + y * width]), 
                     reinterpret_cast<uint8_t*>(&pixels_[out + 1]),
                     reinterpret_cast<uint8_t*>(&pixels_[out]), gamma);
    }
}

  // This function is called when the release fence is signalled
async_wait_result_t Buffer::OnReleaseFenceSignalled(async_t* async, zx_status_t status,
                            const zx_packet_signal* signal) {
    if (status != ZX_OK) {
      FXL_LOG(ERROR) << "BufferHandler received an error ("
                     << zx_status_get_string(status) << ").  Exiting.";
      return ASYNC_WAIT_FINISHED;
    }
    Reset();
    if (release_fence_callback_) {
      release_fence_callback_(this);
    }
    return ASYNC_WAIT_AGAIN;
}

void Buffer::SetReleaseFenceHandler(BufferCallback callback) {
  if (!callback) {
      FXL_LOG(ERROR) << "callback is nullptr";
      return;
  }
  release_fence_callback_ = fbl::move(callback);
  release_fence_waiter_.set_object(release_fence_.get());
  release_fence_waiter_.set_trigger(ZX_EVENT_SIGNALED);
  release_fence_waiter_.set_handler(fbl::BindMember(this, &Buffer::OnReleaseFenceSignalled));
  // Clear the release fence, so we don't just trigger ourselves
  release_fence_.signal(ZX_EVENT_SIGNALED, 0);
  auto status = release_fence_waiter_.Begin();
  FXL_DCHECK(status == ZX_OK);
}
 
void Buffer::Reset() {
    acquire_fence_.signal(ZX_EVENT_SIGNALED, 0);
    release_fence_.signal(ZX_EVENT_SIGNALED, 0);
    state_ = BufferState::kAvailable;
}

void Buffer::Signal() {
    acquire_fence_.signal(0, ZX_EVENT_SIGNALED);
    state_ = BufferState::kReadLocked;
}
