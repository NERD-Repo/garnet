// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/examples/ui/video_display/camera_client.h"
#include <fcntl.h>
#include <inttypes.h>
#include <zircon/assert.h>
#include <zircon/device/audio.h>
#include <zircon/process.h>
#include <zircon/syscalls.h>
#include <zx/channel.h>
#include <zx/handle.h>
#include <zx/vmar.h>
#include <zx/vmo.h>
#include <fbl/algorithm.h>
#include <fbl/auto_call.h>
#include <fbl/limits.h>
#include <fdio/io.h>
#include <stdio.h>
#include <string.h>
#include "lib/fsl/tasks/message_loop.h"

namespace camera {
namespace utils {

#define GGDEBUG printf

#define CHECK_RESP_RESULT(_resp, _cmd_name)       \
    if (ZX_OK != _resp.result) {                                     \
        printf("%s failure (result: %d)\n", _cmd_name, resp.result); \
        return _resp.result;                                         \
    }


CameraClient::CameraClient()
  : cmd_msg_waiter_(fsl::MessageLoop::GetCurrent()->async()), 
    buff_msg_waiter_(fsl::MessageLoop::GetCurrent()->async()) {
}

CameraClient::~CameraClient() {
    Close();
}


zx_status_t CameraClient::Open(uint32_t dev_id) {
    char dev_path[64] = { 0 };
    snprintf(dev_path, sizeof(dev_path), "/dev/class/camera/%03u", dev_id);

    // if (stream_ch_ != ZX_HANDLE_INVALID)
        // return ZX_ERR_BAD_STATE;

    int fd = ::open(dev_path, O_RDONLY);
    if (fd < 0) {
        printf("Failed to open \"%s\" (res %d)\n", dev_path, fd);
        return fd;
    }

    ssize_t res = ::fdio_ioctl(fd, CAMERA_IOCTL_GET_CHANNEL,
                               nullptr, 0,
                               &stream_ch_, sizeof(stream_ch_));
    ::close(fd);

    if (res != sizeof(stream_ch_)) {
        printf("Failed to obtain channel (res %zd)\n", res);
        return static_cast<zx_status_t>(res);
    }

    // Set up waiter to wait for messages on this channel:
    cmd_msg_waiter_.set_object(stream_ch_.get());
    cmd_msg_waiter_.set_trigger(ZX_CHANNEL_READABLE);
    cmd_msg_waiter_.set_handler(fbl::BindMember(this, &CameraClient::OnNewCmdMessage));
    auto status = cmd_msg_waiter_.Begin();
    if (status != ZX_OK) {
        printf("Failed to start AutoWaiter\n");
        return status;
    }

    return ZX_OK;
}

zx_status_t CameraClient::GetSupportedFormats(GetFormatCallback get_formats_callback) {
    if (get_formats_callback == nullptr) {
        return ZX_ERR_INVALID_ARGS;
    }
    get_formats_callback_ = fbl::move(get_formats_callback);
    out_formats_.reset();

    camera_stream_cmd_get_formats_req req;
    req.hdr.cmd = CAMERA_STREAM_CMD_GET_FORMATS;
    zx_status_t write_status = stream_ch_.write(0, &req, sizeof(req), nullptr, 0); 
    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    }
    return write_status;
}

zx_status_t CameraClient::OnGetFormatsResp(camera_proto::GetFormatsResp resp) {
  GGDEBUG("CameraClient::OnGetFormatsResp\n");
  // CHECK_RESP_RESULT(resp, "GetFormat");
  
  if (nullptr == get_formats_callback_) {
    printf("Unexpected message response (cmd %04x, GetFormats)\n", resp.hdr.cmd);
    return ZX_ERR_BAD_STATE;
  }

    uint32_t expected_formats = resp.total_format_count;
  GGDEBUG("CameraClient::OnGetFormatsResp expected_formats: %u\n", expected_formats);
    if (!expected_formats) {
        // done grabbing formats
        zx_status_t ret = get_formats_callback_(out_formats_);
        get_formats_callback_ = nullptr;
        return ret;
    }
    
    if (out_formats_.size() == 0) {
        fbl::AllocChecker ac;
        out_formats_.reserve(expected_formats, &ac);
        if (!ac.check()) {
            printf("Failed to allocated %u entries for format ranges\n", expected_formats);
            return ZX_ERR_NO_MEMORY;
        }
    }

    // Check for out of order:
    if (out_formats_.size() != resp.already_sent_count) {
        printf("Bad format index while fetching formats (expected %lu, got %hu)\n",
                out_formats_.size(), resp.already_sent_count);
        return ZX_ERR_INTERNAL;
    }

    // Calulate how many frames to grab.  If there are more than 
    // CAMERA_STREAM_CMD_GET_FORMATS_MAX_FORMATS_PER_RESPONSE formats,
    // we will be getting multiple messages.  Each message, with the possible
    // exeption of the last message will have the max number of formats.
    // The last message will have (total messages) - (already received) 
    // messages.
    uint32_t to_grab = fbl::min(static_cast<uint32_t>(expected_formats - 
                out_formats_.size()),
            CAMERA_STREAM_CMD_GET_FORMATS_MAX_FORMATS_PER_RESPONSE);

    for (uint16_t i = 0; i < to_grab; ++i) {
        out_formats_.push_back(resp.formats[i]);
    }

    if (out_formats_.size() == expected_formats) {
        // done grabbing formats
  GGDEBUG("CameraClient::OnGetFormatsResp grabbed formats, calling callback\n");
        zx_status_t ret = get_formats_callback_(out_formats_);
        get_formats_callback_ = nullptr;
        return ret;
    }

    return ZX_OK;
}


zx_status_t CameraClient::SetFormat(const camera_video_format_t &format, SetFormatCallback set_format_callback) {
  GGDEBUG("CameraClient::SetFormat\n");
    // if ((stream_ch_ == ZX_HANDLE_INVALID) || (vb_ch_ != ZX_HANDLE_INVALID))
        // printf("Channels in wrong state for SetFormat\n");
        // return ZX_ERR_BAD_STATE;

    if (!set_format_callback) {
        printf("set_format_callback is invalid\n");
        return ZX_ERR_INVALID_ARGS;
    }
    set_format_callback_ = fbl::move(set_format_callback);

    camera_stream_cmd_set_format_req_t  req;
    req.hdr.cmd = CAMERA_STREAM_CMD_SET_FORMAT;
    req.video_format = format;
    zx_status_t write_status = stream_ch_.write(0, &req, sizeof(req), nullptr, 0); 
    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    } else {
  GGDEBUG("CameraClient::SetFormat wrote successfully\n");

    }
    return write_status;
}


zx_status_t CameraClient::OnSetFormatResp(camera_proto::SetFormatResp resp,
        zx::channel resp_handle_out) {
  CHECK_RESP_RESULT(resp, "SetFormat");

  if (!set_format_callback_) {
    printf("Unexpected message response (cmd %04x, SetFormat)\n", resp.hdr.cmd);
    return ZX_ERR_BAD_STATE;
  }

  max_frame_size_ = resp.max_frame_size;

  // TODO(garratt) : Verify the type of this handle before transferring it to
  // our ring buffer channel handle.
  vb_ch_.reset(resp_handle_out.release());
    // Now that our buffer is recognized, set up our waiter on the buffer
    // channel:
    buff_msg_waiter_.set_object(vb_ch_.get());
    buff_msg_waiter_.set_trigger(ZX_CHANNEL_READABLE);
    buff_msg_waiter_.set_handler(fbl::BindMember(this, &CameraClient::OnNewBufferMessage));
    auto status = buff_msg_waiter_.Begin();
    if (status != ZX_OK) {
        printf("Failed to start AutoWaiter\n");
        return status;
    }
  zx_status_t ret = set_format_callback_(max_frame_size_);
  set_format_callback_ = nullptr;  // reset the function, to indicate state.
  return ret;
}

zx_status_t CameraClient::SetBuffer(const zx::vmo &buffer_vmo) {
    ZX_DEBUG_ASSERT(vb_ch_);
    camera_vb_cmd_set_buffer_req_t req;
    req.hdr.cmd = CAMERA_VB_CMD_SET_BUFFER;
    zx_handle_t vmo_handle;
    //TODO(garratt): check this:
    zx_handle_duplicate(buffer_vmo.get(), ZX_RIGHT_SAME_RIGHTS, &vmo_handle);
    
    zx_status_t write_status = vb_ch_.write(0, &req, sizeof(req), &vmo_handle, 1); 

    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    }
    return write_status;
}

zx_status_t CameraClient::ReleaseFrame(uint64_t data_offset) {
    fbl::AutoLock lock(&lock_);
    camera_vb_cmd_frame_release_req req;
    req.hdr.cmd = CAMERA_VB_CMD_FRAME_RELEASE;
    req.data_vb_offset = data_offset;
    
    zx_status_t write_status = vb_ch_.write(0, &req, sizeof(req), nullptr, 0); 
    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    }
    return write_status;
}

zx_status_t CameraClient::OnSetBufferResp(camera_proto::VideoBufSetBufferResp resp) {
    // Make sure that the number of bytes we got back matches the size of the
    // response structure.
    CHECK_RESP_RESULT(resp, "SetBuffer");
    buffer_set_ = true;

    // Check if a start command was called.  If so, re-call the Start command,
    // to actually send the message
    if (frame_notify_callback_) {
        return Start(std::move(frame_notify_callback_));
    }

    return ZX_OK;
}

zx_status_t CameraClient::Start(FrameNotifyCallback frame_notify_callback) {
    if (!frame_notify_callback) {
        return ZX_ERR_INVALID_ARGS;
    }
    frame_notify_callback_ = fbl::move(frame_notify_callback);
    
    // If we have not set up the buffer yet, don't call start
    // We check in OnSetBufferResp if Start should be called,
    // based on if frame_notify_callback_ is set
    // TODO(garratt): return error if SetBuffer has not yet been called
    if (!buffer_set_) {
        return ZX_OK;
    }

    if (!vb_ch_.is_valid())
        return ZX_ERR_BAD_STATE;

    camera_vb_cmd_start_req_t  req;
    req.hdr.cmd = CAMERA_VB_CMD_START;
    zx_status_t write_status = vb_ch_.write(0, &req, sizeof(req), nullptr, 0); 
    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    }
    return write_status;
}


zx_status_t CameraClient::OnFrameNotify(camera_proto::VideoBufFrameNotify resp) {
  // CHECK_RESP_RESULT(resp, "FrameNotify");

  if (!frame_notify_callback_) {
    printf("Unexpected message response (cmd %04x, FrameNotify)\n", resp.hdr.cmd);
    return ZX_ERR_BAD_STATE;
  }
  return frame_notify_callback_(resp);
}

zx_status_t CameraClient::Stop() {
    if (!vb_ch_.is_valid())
        return ZX_ERR_BAD_STATE;

    camera_vb_cmd_stop_req_t  req;
    req.hdr.cmd = CAMERA_VB_CMD_STOP;
    zx_status_t write_status = vb_ch_.write(0, &req, sizeof(req), nullptr, 0); 
    if (write_status != ZX_OK) {
        printf("Cmd write failure (cmd %04x, res %d)\n", req.hdr.cmd, write_status);
    }
    return write_status;
}

void CameraClient::Close() {
    vb_ch_.reset();
    stream_ch_.reset();
}

typedef union {
  camera_proto::CmdHdr           hdr;
  camera_proto::GetFormatsResp   get_format;         
  camera_proto::SetFormatResp    set_format;
} CameraCmdResponse;

typedef union {
  camera_proto::CmdHdr                    hdr;
  camera_proto::VideoBufSetBufferResp     set_buffer;
  camera_proto::VideoBufStartResp         start;
  camera_proto::VideoBufStopResp          stop;
  camera_proto::VideoBufFrameReleaseResp  release_frame;
  camera_proto::VideoBufFrameNotify       frame_notify;
} CameraBufferResponse;

    // if (_expect_handle == (num_rxed_handles > 0)) {                       \
      // FXL_LOG(ERROR) << (_expect_handle ? "Missing" : "Unexpected")       \
                     // << " handle in " #_ioctl " response";                \
      // return ZX_ERR_INVALID_ARGS;                                         \
    // }                                                                     
#define CHECK_RESP(_ioctl, _payload, _expect_handle)           \
  do {                                                                    \
    if (resp_size != sizeof(resp._payload)) {                              \
      FXL_LOG(ERROR) << "Bad " #_ioctl " response length (" << resp_size  \
                     << " != " << sizeof(resp._payload) << ")";            \
      return ZX_ERR_INVALID_ARGS;                                         \
    }                                                                     \
  } while (0)

zx_status_t CameraClient::ProcessBufferChannel() {
    fbl::AutoLock lock(&lock_);

    CameraBufferResponse resp;
    static_assert(sizeof(resp) <= 256,
                  "Response buffer is getting to be too large to hold on the stack!");

    uint32_t resp_size, num_rxed_handles;
    zx_handle_t rxed_handle;
    zx_status_t res = vb_ch_.read(0, &resp, sizeof(resp), &resp_size, &rxed_handle, 0, &num_rxed_handles);
    if (res != ZX_OK)
        return res;

    if (resp_size < sizeof(resp.hdr)) {
        return ZX_ERR_INVALID_ARGS;
    }

    auto cmd = static_cast<camera_proto::Cmd>(resp.hdr.cmd);
    switch (cmd) {
        case CAMERA_VB_CMD_SET_BUFFER:
            CHECK_RESP(CAMERA_VB_CMD_SET_BUFFER,  set_buffer, false);
            return OnSetBufferResp(resp.set_buffer);
            break;
        case CAMERA_VB_FRAME_NOTIFY:
            CHECK_RESP(CAMERA_VB_FRAME_NOTIFY,   frame_notify, false);
            return OnFrameNotify(resp.frame_notify);
            break;
        // Start, Stop and Release all have the same response.
        // We don't act on the response except to freak out if it is not 
        // ZX_OK.  Combine them all here, to avoid makeing a bunch of
        // functions:
        case CAMERA_VB_CMD_START:
        case CAMERA_VB_CMD_STOP:
        case CAMERA_VB_CMD_FRAME_RELEASE:
            CHECK_RESP(START_OR_STOP_OR_RELEASE, start, false);
            if (resp.start.result != ZX_OK) {
                printf("Response to cmd was %d. Shutting down!", resp.start.result);
                // ShutDown(); TODO(garratt): make this...
            }
            return resp.start.result;
            break;
    default:
        printf("Unrecognized stream command 0x%04x\n", resp.hdr.cmd);
        return ZX_ERR_NOT_SUPPORTED;
    }
    return ZX_ERR_NOT_SUPPORTED;
}

zx_status_t CameraClient::ProcessCmdChannel() {
    fbl::AutoLock lock(&lock_);

    CameraCmdResponse resp;
    // TODO(garratt): break up the union?
    // static_assert(sizeof(resp) <= 256,
                  // "Response buffer is getting to be too large to hold on the stack!");

    uint32_t resp_size = 0, num_rxed_handles = 0;
    zx_handle_t rxed_handle;
    zx_status_t res = stream_ch_.read(0, &resp, sizeof(resp), &resp_size, &rxed_handle, 1, &num_rxed_handles);
    if (res != ZX_OK)
        return res;

    if (resp_size < sizeof(resp.hdr)) {
        return ZX_ERR_INVALID_ARGS;
    }
    printf("Received command response. cmd: 0x%04x  %u resp_size, %u handle, %u num_handles\n", 
            resp.hdr.cmd, resp_size, rxed_handle, num_rxed_handles);

    // Strip the NO_ACK flag from the request before selecting the dispatch target.
    auto cmd = static_cast<camera_proto::Cmd>(resp.hdr.cmd);
    switch (cmd) {
        case CAMERA_STREAM_CMD_GET_FORMATS:
            CHECK_RESP(CAMERA_STREAM_CMD_GET_FORMAT, get_format, false);
            return OnGetFormatsResp(resp.get_format);
            break;
        case CAMERA_STREAM_CMD_SET_FORMAT:
            CHECK_RESP(CAMERA_STREAM_CMD_SET_FORMAT, set_format, true);
            return OnSetFormatResp(resp.set_format, zx::channel(rxed_handle));
            break;
        default:
        printf("Unrecognized command response 0x%04x\n", resp.hdr.cmd);
        return ZX_ERR_NOT_SUPPORTED;
    }

    return ZX_ERR_NOT_SUPPORTED;
}
#undef CHECK_RESP

async_wait_result_t CameraClient::OnNewCmdMessage(async_t* async, zx_status_t status,
                            const zx_packet_signal* signal) {
    if (status != ZX_OK) {
      printf("Error: CameraClient received an error.  Exiting.");
      return ASYNC_WAIT_FINISHED;
    }
    // Read channel
    zx_status_t ret_status = ProcessCmdChannel();
    if (ret_status != ZX_OK) {
        // TODO(garratt): Shut it down!
      printf("Error: Got bad status when processing channel (%d)\n", ret_status);
      return ASYNC_WAIT_FINISHED;
    }
    return ASYNC_WAIT_AGAIN;
}

async_wait_result_t CameraClient::OnNewBufferMessage(async_t* async, zx_status_t status,
                            const zx_packet_signal* signal) {
    if (status != ZX_OK) {
      printf("Error: CameraClient received an error.  Exiting.");
      return ASYNC_WAIT_FINISHED;
    }
    // Read channel
    zx_status_t ret_status = ProcessBufferChannel();
    if (ret_status != ZX_OK) {
        // TODO(garratt): Shut it down!
      printf("Error: Got bad status when processing channel (%d)\n", ret_status);
      return ASYNC_WAIT_FINISHED;
    }
    return ASYNC_WAIT_AGAIN;
}

}  // namespace utils
}  // namespace audio
