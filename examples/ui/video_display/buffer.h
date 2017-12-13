// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
#ifndef GARNET_LIB_MAGMA_SRC_DISPLAY_PIPE_CLIENT_BUFFER_H_
#define GARNET_LIB_MAGMA_SRC_DISPLAY_PIPE_CLIENT_BUFFER_H_

#include <stdint.h>

#include <zx/event.h>
#include <zx/vmo.h>
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/command_line.h"
#include "lib/fxl/log_settings_command_line.h"
#include "lib/fxl/logging.h"
#include "zircon/status.h"
#include <fbl/function.h>
#include <async/cpp/auto_wait.h>

class Buffer;
using BufferCallback  = fbl::Function<void(Buffer *)>;

struct BufferLayout {
    zx::vmo buffer_vmo;
    std::vector<uint64_t> buffer_sizes;
    std::vector<uint64_t> buffer_offsets;
};

class Buffer {
 enum class BufferState {
    kInvalid = 0,
    kAvailable,
    kWriteLocked,
    kReadLocked
 };
 public:
  ~Buffer();

  // Assumes that the buffer is set up as an ARGB image,
  // with 4 bytes per pixel.  Fills the entire size of the buffer
  // with a set color with the red, green and blue channels
  // indicated by the r, g and b arguments.
  void FillARGB(uint8_t r, uint8_t g, uint8_t b);
  void ConvertToRGBA(Buffer *b, int gamma = 0); 
  void ConvertToBgraAndMirror(Buffer *b, uint32_t width, int gamma = 0);

  void Reset();    // clear acquire and release fences
  void Signal();   // set acquire fence

  const zx::event& acqure_fence() { return acquire_fence_; }
  const zx::event& release_fence() { return release_fence_; }

  void dupAcquireFence(zx::event *result) {
      // TODO: remove write permissions
     acquire_fence_.duplicate(ZX_RIGHT_SAME_RIGHTS, result);
  }
  
  void dupReleaseFence(zx::event *result) {
     release_fence_.duplicate(ZX_RIGHT_SAME_RIGHTS, result);
  }

  void ReplaceReleaseFence(const zx::event &new_event) {
    release_fence_.reset();
    new_event.duplicate(ZX_RIGHT_SAME_RIGHTS, &release_fence_);
    // new_event.replace(ZX_RIGHT_SAME_RIGHTS, &release_fence_);
  }

  void dupVmo(zx::vmo *result) {
     vmo_.duplicate(ZX_RIGHT_SAME_RIGHTS & ~ZX_RIGHT_WRITE, result);
  }

  static Buffer *NewBuffer(uint64_t buffer_size, const zx::vmo &main_buffer, uint64_t offset, uint32_t index);

  // Writes the contents of the buffer to a file, no header
  zx_status_t SaveToFile(const char *filename);
  
  // returns true if the buffer is neither read locked or write locked.
  bool IsAvailable() {
    return state_ == BufferState::kAvailable;
  }

  // This function is called when the release fence is signalled
  async_wait_result_t OnReleaseFenceSignalled(async_t* async, zx_status_t status,
                              const zx_packet_signal* signal);

  // Set a handler function that will be called whenever the release fence
  // is signalled.
 // void SetReleaseFenceHandler(BufferCallback callback);
  void SetReleaseFenceHandler(BufferCallback callback);

  uint32_t index() { return index_; }
  uint64_t vmo_offset() { return vmo_offset_; }
  uint64_t size() { return size_; }

 private:
  uint32_t index_;
  BufferCallback release_fence_callback_;
  async::AutoWait release_fence_waiter_;
  uint32_t *pixels_; 
  
  Buffer() : release_fence_waiter_(fsl::MessageLoop::GetCurrent()->async()) {};

  zx::vmo vmo_;
  uint64_t vmo_offset_;
  uint64_t size_;
  BufferState state_ = BufferState::kInvalid;

  zx::event acquire_fence_;
  zx::event release_fence_;
};

#endif  // GARNET_LIB_MAGMA_SRC_DISPLAY_PIPE_CLIENT_BUFFER_H_

