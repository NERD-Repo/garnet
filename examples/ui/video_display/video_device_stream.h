
#pragma once

#include <zx/vmo.h>

#include <fbl/vector.h>
#include "zircon/status.h"
#include <zircon/types.h>


using SetFormatCallback = fbl::Function<zx_status_t(uint64_t)>;
using GetFormatCallback = fbl::Function<zx_status_t(const fbl::Vector<camera_video_format_t> &out_formats)>;
using FrameNotifyCallback = fbl::Function<zx_status_t(camera_vb_frame_notify_t)>;

class VideoDeviceStream {
public:
    virtual zx_status_t SetFormat(const camera_video_format_t &format, SetFormatCallback set_format_callback)=0;
    virtual zx_status_t GetSupportedFormats(GetFormatCallback get_formats_callback)=0;
    virtual zx_status_t SetBuffer(const zx::vmo &vmo)=0;
    virtual zx_status_t Start(FrameNotifyCallback frame_notify_callback)=0;
    virtual zx_status_t ReleaseFrame(uint64_t data_offset)=0;
    virtual zx_status_t Stop()=0;
    virtual zx_status_t Open(uint32_t dev_id)=0;
    virtual void        Close() = 0;
};
