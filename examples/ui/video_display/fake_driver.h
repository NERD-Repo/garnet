
#include <stdio.h>

#include <utility>

#include <zx/vmar.h>
#include <zx/vmo.h>

#include <fbl/vector.h>
#include "buffer.h"
#include <async/cpp/auto_task.h>
#include <zircon/device/camera.h>
#include <garnet/examples/ui/video_display/video_device_stream.h>
// #include <camera-proto/camera-proto.h>

using NotifyCallback  = fbl::Function<void(const camera_vb_frame_notify_t &)>;

class ColorSource {
  uint32_t frame_color_ = 0x80;
  static constexpr uint32_t kFrameColorInc = 0x01;
  static constexpr uint32_t kMaxFrameColor = 0x600;
 public:
  void WriteToBuffer(Buffer *buffer) {
    uint8_t r, g, b;
    hsv_color(frame_color_, &r, &g, &b);
    FXL_LOG(INFO) << "Filling with " << (int)r << " " << (int)g << " " << (int)b;
    buffer->FillARGB(r, g, b);
    frame_color_ += kFrameColorInc;
    if (frame_color_ > kMaxFrameColor) {
        frame_color_ -= kMaxFrameColor;
    }
  }

 private:
  void hsv_color(uint32_t index, uint8_t *r, uint8_t *g, uint8_t *b) {
    uint8_t pos = index & 0xff;
    uint8_t neg = 0xff - (index & 0xff);
    uint8_t phase = (index >> 8) & 0x7;
    uint8_t phases[6] = {0xff, 0xff, neg, 0x00, 0x00, pos};
    *r = phases[(phase + 1) % 6];
    *g = phases[(phase + 5) % 6];
    *b = phases[(phase + 3) % 6];
  }
};


class CameraRequestHandler {
    fbl::Vector<camera_cmd> cmds_;
    fbl::Vector<uint64_t> req_sizes_;
    std::vector<std::function<void(void *)>> callbacks_;

    
    public:
      void AddListener(camera_cmd_t cmd, size_t req_size, std::function<void(void *)> &&foo) {
          cmds_.push_back(cmd);
          req_sizes_.push_back(req_size);
          callbacks_.push_back(foo);
      }

      void Handle(void *req) {
          auto cmd = static_cast<camera_cmd_t*>(req);
          //todo: check size
          for (uint32_t i = 0; i < cmds_.size(); ++i) {
              if(*cmd == cmds_[i]) {
                  (callbacks_[i])(req);
                  return;
              }
          }
          // todo: error if not found
      }
};



#define addHandler(_switcher, _cmd, _type, _handler)       \
    _switcher.AddListener(_cmd, sizeof(_type), [this](void *ptr) { \
         _handler(*static_cast<_type*>(ptr)); });
             



class FakeVideoSource : public VideoDeviceStream {

uint64_t NSecPerFrame(const camera_video_format_t &format, uint64_t num_frames = 1) {
    return (format.frames_per_sec_denominator * 1e9 * num_frames ) / format.frames_per_sec_numerator;
}
 public:
    ~FakeVideoSource() {}

    zx_status_t Open(uint32_t dev_id) { return ZX_OK; }
    void Close() {}

    zx_status_t GetSupportedFormats(GetFormatCallback callback) {
        fbl::Vector<camera_video_format_t> out_formats;
        camera_video_format_t format;
        format.width = 640;
        format.height = 480;
        format.bits_per_pixel = 4;
        format.frames_per_sec_numerator = 30;
        format.frames_per_sec_denominator = 1;
        out_formats.push_back(format);
        callback(out_formats);
        return ZX_OK;
    }

    zx_status_t SetFormat(const camera_video_format_t &format, 
             SetFormatCallback callback) { 
       format_ = format;
       // TODO(garratt): get better calculation of size
       max_frame_size_ = format_.width * format_.height * format_.bits_per_pixel;
       //TODO:(garratt) additional stuff here?
       callback(max_frame_size_);
       return ZX_OK;
   }

   zx_status_t SetBuffer(const zx::vmo &vmo) {
       uint64_t buffer_size;
       vmo.get_size(&buffer_size);
       if (max_frame_size_ == 0 || buffer_size < max_frame_size_ * kMinNumberOfBuffers) {
        FXL_LOG(ERROR) << "Insufficient space has been allocated";
        return ZX_ERR_NO_MEMORY;
       }
       uint64_t num_buffers = buffer_size / max_frame_size_;
       for (uint64_t i = 0; i < num_buffers; ++i) {
           Buffer *buffer = Buffer::NewBuffer(max_frame_size_, 
                                              vmo, max_frame_size_ * i, i);
           buffer->Reset();
           // just getting the buffer to call its handler is enough.
           // We can then check IsAvailable().
           buffer->SetReleaseFenceHandler([](Buffer *b) {});
           buffers_.push_back(buffer);
       }
       return ZX_OK;
   }

   FakeVideoSource() : // VideoDeviceStream(fsl::MessageLoop::GetCurrent()->async()),
           task_(fsl::MessageLoop::GetCurrent()->async()) {}

   // void SetCallbacks() {
     // addHandler(switcher_, camera_cmd::CAMERA_VB_CMD_START, camera_vb_cmd_start_req_t, [this](camera_vb_cmd_start_req_t req) { this->Start();});
     // addHandler(switcher_, camera_cmd::CAMERA_VB_CMD_STOP, camera_vb_cmd_start_req_t, [this](camera_vb_cmd_start_req_t req) { this->Start();});
     // addHandler(switcher_, camera_cmd::CAMERA_VB_CMD_START, camera_vb_cmd_start_req_t, GlobalStart);
   // }

   
   zx_status_t Start(FrameNotifyCallback callback) {
       if (!callback) {
           FXL_LOG(ERROR) << "callback is nullptr";
           return ZX_ERR_INVALID_ARGS;
       }
       notify_callback_ = fbl::move(callback);
     if (buffers_.empty()) { 
         FXL_LOG(ERROR) << "Error: FakeVideoSource not initialized!";
         return ZX_ERR_BAD_STATE;
     }
     frame_count_ = 0;
     start_time_ = zx_clock_get(ZX_CLOCK_MONOTONIC);
     task_.set_handler([this](async_t* async,zx_status_t status) {
         if (status != ZX_OK) {
           FXL_LOG(ERROR) << "FakeVideoSource had a Autotask error, ("
                       << zx_status_get_string(status) << ").  Exiting.";
           return ASYNC_TASK_FINISHED;
         }
         this->Update();
         return ASYNC_TASK_REPEAT;
      });
       SetNextCaptureTime();
      task_.Post();
      return ZX_OK;
   }

   zx_status_t Stop() {
    task_.set_deadline(ZX_TIME_INFINITE);
    return ZX_OK;
   }

 private:
   void FillBuffer(uint32_t index) {
     FXL_LOG(INFO) << "FillBuffer: " << index;
     color_source_.WriteToBuffer(buffers_[index]);
   }

   void SignalBufferFilled(uint32_t index) {
     FXL_LOG(INFO) << "Signalling: " << index;
     if (notify_callback_) {
       camera_vb_frame_notify_t frame;
       frame.frame_size = buffers_[index]->size();
       frame.data_vb_offset = buffers_[index]->vmo_offset();
       frame.metadata.timestamp = next_frame_time_ - NSecPerFrame(format_, kFramesOfDelay); 

       notify_callback_(frame);
     }
     buffers_[index]->Signal();
     // Now send some message to the consumer...
     // Should include timestamp of next_frame_time_
   }

   void SetNextCaptureTime() {
     // Set the next frame time to be start + frame_count / frames per second.
     next_frame_time_ = start_time_ + NSecPerFrame(format_, frame_count_++); 
     task_.set_deadline(next_frame_time_);
     FXL_LOG(INFO) << "FakeVideoSource: setting next frame to: " << next_frame_time_ 
         << "   "  << next_frame_time_ - zx_clock_get(ZX_CLOCK_MONOTONIC) << " nsec from now";
   }
   
   zx_status_t ReleaseFrame(uint64_t data_offset) {
       for (uint64_t i = 0; i < buffers_.size(); ++i) {
           if (buffers_[i]->vmo_offset() == data_offset) {
                buffers_[i]->Reset();
                return ZX_OK;
           }
       }
       FXL_LOG(ERROR) << "data offset does not correspond to a frame!";
       return ZX_ERR_INVALID_ARGS;
   }

   // Checks which buffer can be written to, 
   // writes it then signals it ready
   // sleeps until next cycle
   void Update() {
       for (uint64_t i = 0; i < buffers_.size(); ++i) {
         if (buffers_[i]->IsAvailable()) {
            FillBuffer(i);
            SignalBufferFilled(i);
            break;
         }
       }
       // If no buffers are available, quietly fail to fill.
       // Schedule next frame:
       SetNextCaptureTime();
   }
  static constexpr uint32_t kMinNumberOfBuffers = 2;
  static constexpr uint32_t kFramesOfDelay = 2;
  ColorSource color_source_;
  uint64_t max_frame_size_ = 0;
  uint64_t frame_count_ = 0;
  zx_time_t start_time_ = 0;
  zx_time_t next_frame_time_ = 0;
  camera_video_format_t format_;
  std::vector<Buffer *> buffers_;
  async::AutoTask task_;
  FrameNotifyCallback notify_callback_;
  CameraRequestHandler switcher_;
};










