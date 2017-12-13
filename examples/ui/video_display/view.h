
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

// #include "garnet/examples/ui/shadertoy/service/services/shadertoy.fidl.h"
// #include
// "garnet/examples/ui/shadertoy/service/services/shadertoy_factory.fidl.h"
#include "lib/app/cpp/application_context.h"
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/macros.h"
#include "lib/ui/scenic/client/resources.h"
#include "lib/ui/view_framework/base_view.h"
#include "garnet/examples/ui/video_display/buffer.h"
#include "garnet/examples/ui/video_display/fake_driver.h"
#include "garnet/examples/ui/video_display/camera_client.h"
#include "garnet/examples/ui/video_display/frame_scheduler.h"
#include <fbl/vector.h>
#include <deque>
#include <list>

namespace video_display {

class View : public mozart::BaseView {
 public:
  View(app::ApplicationContext* application_context,
       mozart::ViewManagerPtr view_manager,
       f1dl::InterfaceRequest<mozart::ViewOwner> view_owner_request);

  ~View() override;

  // When an incoming buffer is filled, View releases the aquire fence
  zx_status_t IncomingBufferFilled(const camera_vb_frame_notify_t& frame);

  // called to reserve a buffer for writing
  zx_status_t ReserveIncomingBuffer(Buffer* buffer);

 private:
  // When a buffer is released, signal that it is available to the writer
  // In this case, that means directly write to the buffer then re-present it
  void BufferReleased(Buffer* buffer);
  // Callbacks from asyncronous interface:
  zx_status_t OnGetFormats(
      const fbl::Vector<camera_video_format_t>& out_formats);
  zx_status_t OnSetFormat(uint64_t max_frame_size);

  // mozart::BaseView.
  virtual bool OnInputEvent(mozart::InputEventPtr event) override;

  // |BaseView|.
  void OnSceneInvalidated(
      ui_mozart::PresentationInfoPtr presentation_info) override;

  // Creates a new buffer and registers an image with scenic.  If the buffer
  // already exists,
  // returns a pointer to that buffer.  buffeer is not required to be valid.
  // If it is nullptr, the returned status can be used to check if that buffer
  // is now available.
  zx_status_t FindOrCreateBuffer(uint32_t frame_size,
                                 uint64_t vmo_offset,
                                 Buffer** buffer,
                                 const camera_video_format_t& format);

  // TODO(garratt) this should support multiple formats, one for each stream...
  camera_video_format_t format_;

  fsl::MessageLoop* loop_;
  static constexpr uint16_t kNumberOfBuffers = 8;
  scenic_lib::ShapeNode node_;

  // Image pipe to send to display
  scenic::ImagePipePtr image_pipe_;

  std::vector<Buffer*> frame_buffers_;
  uint32_t last_buffer_index_ = 0;
  uint64_t max_frame_size_ = 0;

  zx::vmo vmo_;
  FrameScheduler frame_scheduler_;
  // zx_time_t frame_start_time_ = 0;
  VideoDeviceStream* video_source_;
  FakeVideoSource fake_video_source_;
  // std::mutex buffers_lock_;
  int gamma_state_ = 1;
  FXL_DISALLOW_COPY_AND_ASSIGN(View);
};

}  // namespace video_display
