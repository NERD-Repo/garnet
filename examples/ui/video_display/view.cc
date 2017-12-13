
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/examples/ui/video_display/view.h"

#if defined(countof)
// TODO(ZX-377): Workaround for compiler error due to Zircon defining countof()
// as a macro.  Redefines countof() using GLM_COUNTOF(), which currently
// provides a more sophisticated implementation anyway.
#undef countof
#include <glm/glm.hpp>
#define countof(X) GLM_COUNTOF(X)
#else
// No workaround required.
#include <glm/glm.hpp>
#endif
#include <glm/gtc/type_ptr.hpp>
#include "lib/fxl/logging.h"
#include "lib/fxl/log_level.h"

#include "lib/ui/scenic/fidl_helpers.h"

namespace video_display {

namespace {
constexpr uint32_t kShapeWidth = 640;
constexpr uint32_t kShapeHeight = 480;
}  // namespace

#define GGDEBUG printf

// When a buffer is released, signal that it is available to the writer
// In this case, that means directly write to the buffer then re-present it
void View::BufferReleased(Buffer* buffer) {
  FXL_LOG(INFO) << "BufferReleased " << buffer->index();
  frame_scheduler_.ReleaseBuffer(buffer->index());
  video_source_->ReleaseFrame(buffer->vmo_offset());
}

// We allow the incoming stream to reserve a a write lock on a buffer
// it is writing to.  Reserving this buffer signals that it will be the latest
// buffer to be displayed. In other words, no buffer locked after this buffer
// will be displayed before this buffer.
// If the incoming buffer already filled, the driver could just call
// IncomingBufferFilled(), which will make sure the buffer is reserved first.
zx_status_t View::ReserveIncomingBuffer(Buffer* buffer) {
  if (nullptr == buffer) {
    FXL_LOG(ERROR) << "Invalid input buffer";
    return ZX_ERR_INVALID_ARGS;
  }

  uint32_t buffer_index = buffer->index();
  FXL_LOG(INFO) << "Reserving incoming Buffer " << buffer_index;

  // check that no fences are set
  if (!buffer->IsAvailable()) {
    FXL_LOG(ERROR) << "Attempting to Reserve buffer " << buffer_index
                   << " which is marked unavailable.";
    return ZX_ERR_BAD_STATE;
  }
  if (frame_scheduler_.IsEnqueued(buffer->index())) {
    FXL_LOG(ERROR) << "Attempting to Reserve already enqued Buffer "
                   << buffer_index;
    return ZX_ERR_BAD_STATE;
  }

  // TODO(garratt): check that we are presenting stuff
  uint64_t pres_time = frame_scheduler_.EnqueBuffer(buffer_index);

  auto acq = f1dl::Array<zx::event>::New(1);
  auto rel = f1dl::Array<zx::event>::New(1);
  // TODO(garratt): these are supposed to be fire and forget:
  buffer->dupAcquireFence(&acq.front());
  buffer->dupReleaseFence(&rel.front());
  FXL_LOG(INFO) << "presenting Buffer " << buffer_index << " at " << pres_time;

  image_pipe_->PresentImage(
      buffer_index, pres_time, std::move(acq), std::move(rel),
      [this, buffer_index](const ui_mozart::PresentationInfoPtr& info) {
        this->frame_scheduler_.Update(
            info->presentation_time, info->presentation_interval, buffer_index);
      });
  return ZX_OK;
}

// When an incomming buffer is  filled, View releases the aquire fence
zx_status_t View::IncomingBufferFilled(const camera_vb_frame_notify_t& frame) {
  Buffer* buffer;
  if (frame.error != 0) {
    printf("Error set on incoming frame. Error: %d\n", frame.error);
    return ZX_OK;  // no reason to stop the channel...
  }
  GGDEBUG(
      "IncomingBufferFilled: got buffer size: %u, %lu offset.  h*s = %d"
      "  format: %d  NV12: %d YUYV:%d\n",
      frame.frame_size, frame.data_vb_offset,
      format_.width * format_.height * format_.bits_per_pixel / 8,
      format_.pixel_format, NV12, YUY2);
  GGDEBUG("IncomingBufferFilled: image timestamp: %u.  Now: %lu\n",
          frame.metadata.presentation_timestamp,
          zx_clock_get(ZX_CLOCK_MONOTONIC));

  zx_status_t status = FindOrCreateBuffer(
      frame.frame_size, frame.data_vb_offset, &buffer, format_);
  if (ZX_OK != status) {
    FXL_LOG(ERROR) << "Failed to create a frame for the incoming buffer";
    // What can we do here? If we cannot display the frame, quality will
    // suffer...
    return status;
  }

  // Now we know that the buffer exists.
  // If we have not reserved the buffer, do so now. ReserveIncomingBuffer
  // will quietly return if the buffer is already reserved.
  status = ReserveIncomingBuffer(buffer);
  if (ZX_OK != status) {
    FXL_LOG(ERROR) << "Failed to reserve a frame for the incoming buffer";
    return status;
  }

  // Signal that the buffer is ready to be presented:
  buffer->Signal();

  return ZX_OK;
}

// frame interval:
// After we produce frames, we get a callback with when the frame was produced
// and the presentation interval.  The presentation interval is an upper bound
// on our frame rate, so we mostly just need to make sure that we are
// presenting at our desired rate, and make sure that we don't fall behind the
// presentation times being reported
void Gralloc(uint64_t buffer_size, uint32_t num_buffers, zx::vmo* buffer_vmo) {
  // In the future, some special alignment might happen here, or special
  // memory allocated...
  zx::vmo::create(num_buffers * buffer_size, 0, buffer_vmo);
}

// Creates a new buffer and registers an image with scenic.  If the buffer
// already exists,
// returns a pointer to that buffer.  buffer is not required to be valid.
// If it is nullptr, the returned status can be used to check if that buffer is
// now available.
zx_status_t View::FindOrCreateBuffer(uint32_t frame_size,
                                     uint64_t vmo_offset,
                                     Buffer** buffer,
                                     const camera_video_format_t& format) {
  if (nullptr != buffer) {
    *buffer = nullptr;
  }
  // If the buffer exists, return the pointer
  for (Buffer* b : frame_buffers_) {
    // TODO(garratt): why does the frame size change around?
    if (b->vmo_offset() == vmo_offset && b->size() >= frame_size) {
      if (nullptr != buffer) {
        *buffer = b;
      }
      return ZX_OK;
    }
  }
  last_buffer_index_++;
  FXL_LOG(INFO) << "Creating buffer " << last_buffer_index_;
  // TODO(garratt): change back to frame_size when we fix the fact that they are
  // changing...
  Buffer* b =
      Buffer::NewBuffer(max_frame_size_, vmo_, vmo_offset, last_buffer_index_);
  if (nullptr == b) {
    return ZX_ERR_INTERNAL;
  }
  // Set release fence callback so we know when a frame is made available
  b->SetReleaseFenceHandler([this](Buffer* b) { this->BufferReleased(b); });
  b->Reset();
  frame_buffers_.push_back(b);
  if (nullptr != buffer) {
    *buffer = b;
  }

  // Now add that buffer to the image pipe:
  FXL_LOG(INFO) << "Creating ImageInfo ";
  auto image_info = scenic::ImageInfo::New();
  image_info->stride = format.stride;
  image_info->tiling = scenic::ImageInfo::Tiling::LINEAR;
  image_info->width = format.width;
  image_info->height = format.height;

  // To make things look like a webcam application, mirror left-right.
  image_info->transform = scenic::ImageInfo::Transform::FLIP_HORIZONTAL;

  zx::vmo vmo;
  FXL_LOG(INFO) << "Duping VMO ";
  b->dupVmo(&vmo);
  // image_info->stride = format.stride * 2;// this is not right...
  // image_info->width = format.width;
  image_info->pixel_format = scenic::ImageInfo::PixelFormat::YUY2;
  image_pipe_->AddImage(b->index(), std::move(image_info), std::move(vmo),
                        scenic::MemoryType::HOST_MEMORY, vmo_offset);

  return ZX_OK;
}

View::View(app::ApplicationContext* application_context,
           mozart::ViewManagerPtr view_manager,
           f1dl::InterfaceRequest<mozart::ViewOwner> view_owner_request)
    : BaseView(std::move(view_manager),
               std::move(view_owner_request),
               "Video Display Example"),
      // application_context_(application_context),
      loop_(fsl::MessageLoop::GetCurrent()),
      node_(session()) {
  // start_time_(zx_time_get(ZX_CLOCK_MONOTONIC)) {

  FXL_LOG(INFO) << "Creating View";
  // Pass the other end of the ImagePipe to the Session, and wrap the
  // resulting resource in a Material.
  uint32_t image_pipe_id = session()->AllocResourceId();
  session()->Enqueue(scenic_lib::NewCreateImagePipeOp(
      image_pipe_id, image_pipe_.NewRequest()));
  scenic_lib::Material material(session());
  material.SetTexture(image_pipe_id);
  // material.SetColor(0xff, 0, 0, 0xff);
  session()->ReleaseResource(image_pipe_id);

  // Create a rounded-rect shape to display the Shadertoy image on.
  scenic_lib::RoundedRectangle shape(session(), kShapeWidth, kShapeHeight, 80,
                                     80, 80, 80);

  node_.SetShape(shape);
  node_.SetMaterial(material);
  parent_node().AddChild(node_);
  node_.SetTranslation(640, 480, 50);
  InvalidateScene();

  FXL_LOG(INFO) << "Creating View - set up image pipe";

  video_source_ = new camera::utils::CameraClient();
  video_source_->Open(0);
  video_source_->GetSupportedFormats(
      fbl::BindMember(this, &View::OnGetFormats));
}

// Asyncronous setup of camera:
// 1) Get format
// 2) Set format
// 3) Set buffer
// 4) Start

zx_status_t View::OnGetFormats(
    const fbl::Vector<camera_video_format_t>& out_formats) {
  // For now, just configure to the first format available:
  if (out_formats.size() < 1) {
    FXL_LOG(ERROR) << "No supported formats available";
    return ZX_ERR_INTERNAL;
  }
  // For other configurations, we would chose a format in a fancier way...
  format_ = out_formats[0];
  GGDEBUG(
      "Chose format.  Capture Type: %d W:H:S = %u:%u:%u bbp: %u format: %u\n",
      format_.capture_type, format_.width, format_.height, format_.stride,
      format_.bits_per_pixel, format_.pixel_format);
  return video_source_->SetFormat(format_,
                                  fbl::BindMember(this, &View::OnSetFormat));
}

zx_status_t View::OnSetFormat(uint64_t max_frame_size) {
  GGDEBUG("OnSetFormat: max_frame_size: %lu  making buffer size: %lu\n",
          max_frame_size, max_frame_size * kNumberOfBuffers);
  // Allocate the memory:

  if (max_frame_size < format_.stride * format_.height) {
    GGDEBUG("OnSetFormat: max_frame_size: %lu < needed frame size: %u\n",
            max_frame_size, format_.stride * format_.height);
    max_frame_size = format_.stride * format_.height;
  }
  max_frame_size_ = max_frame_size;
  Gralloc(max_frame_size, kNumberOfBuffers, &vmo_);
  // Tell the driver about the memory:
  // TODO(garratt): this whole handshaking with the memory feels awkward...
  // Also, does it make sense to wait in this app for setbuffer?
  zx_status_t ret = video_source_->SetBuffer(vmo_);
  if (ret != ZX_OK) {
    return ret;
  }
  return video_source_->Start(
      fbl::BindMember(this, &View::IncomingBufferFilled));
}

View::~View() = default;

void View::OnSceneInvalidated(
    ui_mozart::PresentationInfoPtr presentation_info) {
  // FXL_LOG(INFO) << "View::OnSceneInvalidated";
  if (!has_logical_size())
    return;

  // Compute the amount of time that has elapsed since the view was created.
  double seconds =
      static_cast<double>(presentation_info->presentation_time) / 1'000'000'000;

  const float kHalfWidth = logical_size().width * 0.5f;
  const float kHalfHeight = logical_size().height * 0.5f;

  // Compute the translation for kSwirling mode.
  // Each node has a slightly different speed.
  node_.SetTranslation(kHalfWidth * (1.1 + .1 * sin(seconds * 0.8)),
                       kHalfHeight * (1.2 + .1 * sin(seconds * 0.6)), 50.0);

  // The rounded-rectangles are constantly animating; invoke InvalidateScene()
  // to guarantee that OnSceneInvalidated() will be called again.
  InvalidateScene();
}

bool View::OnInputEvent(mozart::InputEventPtr event) {
  if (event->is_keyboard()) {
    const auto& keyboard = event->get_keyboard();
    if (keyboard->phase == mozart::KeyboardEvent::Phase::PRESSED) {
      //&& keyboard->hid_usage == 6 /* c */) {
      gamma_state_ = (gamma_state_ + 1) % 3;
      printf("Gamma = %d\n", gamma_state_ - 1);
    }
    return true;
  }
  return false;
}

}  // namespace shadertoy_client
