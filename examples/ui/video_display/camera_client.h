// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

#include <zircon/device/camera.h>
// #include <camera-driver-proto/camera-proto.h>
#include <zircon/types.h>
#include <zx/channel.h>
#include <zx/vmo.h>
#include <fbl/unique_ptr.h>
#include <fbl/vector.h>
#include <fbl/mutex.h>
#include <fbl/auto_lock.h>
#include <async/cpp/auto_wait.h>
#include "garnet/examples/ui/video_display/video_device_stream.h"

// namespace video {
// namespace utils {

namespace camera {
namespace camera_proto {

// C++ style aliases for protocol structures and types.
using Cmd    = camera_cmd_t;
using CmdHdr = camera_cmd_hdr_t;

// Structures used in GET_FORMATS and SET_FORMAT.
using CaptureType = camera_capture_type_t;
using PixelFormat = camera_pixel_format_t;
using VideoFormat = camera_video_format_t;

// Format of frame metadata.
using Metadata = camera_metadata_t;

// CAMERA_STREAM_CMD_GET_FORMATS
using GetFormatsReq  = camera_stream_cmd_get_formats_req_t;
using GetFormatsResp = camera_stream_cmd_get_formats_resp_t;

// CAMERA_STREAM_CMD_SET_FORMAT
using SetFormatReq  = camera_stream_cmd_set_format_req_t;
using SetFormatResp = camera_stream_cmd_set_format_resp_t;

// CAMERA_VB_CMD_SET_BUFFER
using VideoBufSetBufferReq = camera_vb_cmd_set_buffer_req_t;
using VideoBufSetBufferResp = camera_vb_cmd_set_buffer_resp_t;

// CAMERA_VB_CMD_START
using VideoBufStartReq  = camera_vb_cmd_start_req_t;
using VideoBufStartResp = camera_vb_cmd_start_resp_t;

// CAMERA_VB_CMD_STOP
using VideoBufStopReq  = camera_vb_cmd_stop_req_t;
using VideoBufStopResp = camera_vb_cmd_stop_resp_t;

// CAMERA_VB_FRAME_RELEASE
using VideoBufFrameReleaseReq = camera_vb_cmd_frame_release_req_t;
using VideoBufFrameReleaseResp = camera_vb_cmd_frame_release_resp_t;

// CAMERA_VB_FRAME_NOTIFY
using VideoBufFrameNotify = camera_vb_frame_notify_t;

const char* CaptureTypeToString(const CaptureType& capture_type);

const char* PixelFormatToString(const PixelFormat& pixel_format);

}  // namespace camera_proto

namespace utils {

using SetFormatCallback = fbl::Function<zx_status_t(uint64_t)>;
using GetFormatCallback = fbl::Function<zx_status_t(const fbl::Vector<camera_video_format_t> &out_formats)>;
using FrameNotifyCallback = fbl::Function<zx_status_t(camera_proto::VideoBufFrameNotify)>;

class CameraClient : public VideoDeviceStream {
public:

// These are the functions that should be called:
    CameraClient();
    zx_status_t SetFormat(const camera_video_format_t &format, SetFormatCallback set_format_callback);
    zx_status_t GetSupportedFormats(GetFormatCallback get_formats_callback);
    zx_status_t SetBuffer(const zx::vmo &vmo);
    zx_status_t Start(FrameNotifyCallback frame_notify_callback);
    zx_status_t ReleaseFrame(uint64_t data_offset);
    zx_status_t Stop();
    zx_status_t Open(uint32_t dev_id);
    void        Close();



private:
    zx_status_t OnGetFormatsResp(camera_proto::GetFormatsResp resp);
    zx_status_t OnSetFormatResp(camera_proto::SetFormatResp resp, zx::channel ch);
    zx_status_t OnFrameNotify(camera_proto::VideoBufFrameNotify resp);
    zx_status_t OnSetBufferResp(camera_proto::VideoBufSetBufferResp resp);
    
    async_wait_result_t OnNewCmdMessage(async_t* async, zx_status_t status,
                            const zx_packet_signal* signal); 
    zx_status_t ProcessCmdChannel();


    async_wait_result_t OnNewBufferMessage(async_t* async, zx_status_t status,
                            const zx_packet_signal* signal); 
    zx_status_t ProcessBufferChannel();

    // The maximum size a frame will occupy in the video stream.
    // A value of zero means that the video buffer channel is uninitialized.
    uint32_t max_frame_size_ = 0;
    // Tracks if we have set the buffer for the video stream:
    bool buffer_set_ = false; 
    fbl::Mutex lock_;

    // callbacks.  These functions are also used to determine state;
    // If they are defined, then we are waiting for the appropriate response
    SetFormatCallback set_format_callback_ = nullptr;
    GetFormatCallback get_formats_callback_ = nullptr;
    FrameNotifyCallback frame_notify_callback_ = nullptr;

    virtual ~CameraClient();

    zx::channel stream_ch_;
    zx::channel vb_ch_;

    fbl::Vector<camera_video_format_t> out_formats_;
    async::AutoWait cmd_msg_waiter_, buff_msg_waiter_;
};

}  // namespace utils
}  // namespace video
