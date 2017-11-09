// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "command_channel.h"

#include <fcntl.h>

#include <iostream>

#include <async/default.h>
#include <async/loop.h>
#include <zircon/device/bt-hci.h>
#include <zircon/status.h>
#include <zx/event.h>
#include <zx/time.h>
#include <zx/timer.h>

#include "garnet/drivers/bluetooth/lib/hci/slab_allocators.h"

namespace {

zx::channel GetCommandChannel(int fd) {
  zx::channel channel;
  ssize_t status =
      ioctl_bt_hci_get_command_channel(fd, channel.reset_and_get_address());
  if (status < 0) {
    std::cerr << "hci: Failed to obtain command channel handle: "
              << zx_status_get_string(status) << std::endl;
    assert(!channel.is_valid());
  }

  return channel;
}


zx::channel GetAclChannel(int fd) {
  zx::channel channel;
  ssize_t status =
      ioctl_bt_hci_get_acl_data_channel(fd, channel.reset_and_get_address());
  if (status < 0) {
    std::cerr << "hci: Failed to obtain ADL data channel handle: "
              << zx_status_get_string(status) << std::endl;
    assert(!channel.is_valid());
  }

  return channel;
}

}  // namespace

CommandChannel::CommandChannel(std::string hcidev_path) {
  hci_fd_.reset(open(hcidev_path.c_str(), O_RDWR));
  if (!bool(hci_fd_)) {
    return;
  }
  channel_ = GetCommandChannel(hci_fd_.get());
  channel_wait_.set_object(channel_.get());
  channel_wait_.set_trigger(ZX_CHANNEL_READABLE);
  channel_wait_.set_handler(
      fbl::BindMember(this, &CommandChannel::OnChannelReady));
  zx_status_t status = channel_wait_.Begin(async_get_default());
  if (status != ZX_OK) {
    std::cerr << "CommandChannel: problem setting up HCI command channel: "
              << zx_status_get_string(status) << std::endl;
    return;
  }

  acl_channel_ = GetAclChannel(hci_fd_.get());
  acl_channel_wait_.set_object(acl_channel_.get());
  acl_channel_wait_.set_trigger(ZX_CHANNEL_READABLE);
  acl_channel_wait_.set_handler(
      fbl::BindMember(this, &CommandChannel::OnAclChannelReady));
  status = acl_channel_wait_.Begin(async_get_default());
  if (status != ZX_OK) {
    std::cerr << "CommandChannel: problem setting up ACL data channel: "
              << zx_status_get_string(status) << std::endl;
  }
}

CommandChannel::~CommandChannel() {
  SetEventCallback(nullptr);
  channel_wait_.Cancel(async_get_default());
  acl_channel_wait_.Cancel(async_get_default());
}

void CommandChannel::SetEventCallback(const EventCallback& callback) {
  event_callback_ = callback;
}

void CommandChannel::SendCommand(
    const bluetooth::common::PacketView<bluetooth::hci::CommandHeader>&
        command) {
  // TODO(jamuraa): handle this in a non-shitty way later.
  // If this is a 0xfc09 packet (and we're in bootloader mode, which is what we
  // boot to - it needs to, nonsensically, be sent down the bulk URB instead of
  // the standard one, so send it via the ACL channel, because that will do the
  // right thing, since the ACL channel uses the bulk endpoint always.
  zx_status_t status;
  if (command.header().opcode == 0xfc09) {
    status = acl_channel_.write(0, command.data().data(), command.size(), nullptr, 0);
  } else {
    status = channel_.write(0, command.data().data(), command.size(), nullptr, 0);
  }
  if (status < 0) {
    // TODO(jamuraa): Maybe return the zx_status_t in this case?
    std::cerr << "CommandChannel: Failed to send command: "
              << zx_status_get_string(status) << std::endl;
  }
}

void CommandChannel::SendCommandSync(
    const bluetooth::common::PacketView<bluetooth::hci::CommandHeader>& command,
    const EventCallback& callback) {
  zx::event received;
  zx::event::create(0, &received);

  auto cb = [this, &received, callback](const auto& event_packet) {
    if (callback) {
      callback(event_packet);
    }
    received.signal(0, ZX_USER_SIGNAL_0);
  };

  SetEventCallback(cb);

  SendCommand(command);

  // Spin here until we get a response..
  zx_status_t status;
  zx::timer timeout;
  zx::timer::create(0, ZX_CLOCK_MONOTONIC, &timeout);
  timeout.set(zx::deadline_after(ZX_MSEC(200)), ZX_MSEC(50));
  for (;;) {
    async_loop_run(async_get_default(), zx::deadline_after(ZX_MSEC(10)), true);
    status = received.wait_one(ZX_USER_SIGNAL_0, 0u, nullptr);
    if (status != ZX_ERR_TIMED_OUT) {
      break;
    }
    status = timeout.wait_one(ZX_TIMER_SIGNALED, 0u, nullptr);
    if (status != ZX_ERR_TIMED_OUT) {
      status = ZX_ERR_TIMED_OUT;
      break;
    }
  }

  SetEventCallback(nullptr);

  if (status == ZX_OK) {
    return;
  }

  std::cerr << "CommandChannel: error waiting for event "
            << zx_status_get_string(status) << std::endl;
}

async_wait_result_t CommandChannel::HandleChannelReady(
    const zx::channel& channel,
    async_t* async,
    zx_status_t status,
    const zx_packet_signal_t* signal) {
  FXL_DCHECK(signal->observed & ZX_CHANNEL_READABLE);

  if (status != ZX_OK) {
    std::cerr << "CommandChannel: channel error: "
              << zx_status_get_string(status) << std::endl;
    return ASYNC_WAIT_FINISHED;
  }

  // Allocate a buffer for the event. Since we don't know the size
  // beforehand we allocate the largest possible buffer.

  for (size_t count = 0; count < signal->count; count++) {
    uint32_t read_size;
    auto packet = bluetooth::hci::EventPacket::New(
        bluetooth::hci::slab_allocators::kLargeControlPayloadSize);
    if (!packet) {
      std::cerr << "CommandChannel: Failed to allocate event packet!"
                << std::endl;
      return ASYNC_WAIT_FINISHED;
    }
    auto packet_bytes = packet->mutable_view()->mutable_data();
    zx_status_t read_status =
        channel.read(0u, packet_bytes.mutable_data(), packet_bytes.size(),
                      &read_size, nullptr, 0, nullptr);
    if (read_status < 0) {
      std::cerr << "CommandChannel: Failed to read event bytes: "
                << zx_status_get_string(read_status) << std::endl;
      // Clear the handler so that we stop receiving events from it.
      return ASYNC_WAIT_FINISHED;
    }

    if (read_size < sizeof(bluetooth::hci::EventHeader)) {
      std::cerr << "CommandChannel: Malformed event packet - "
                << "expected at least " << sizeof(bluetooth::hci::EventHeader)
                << " bytes, got " << read_size << std::endl;
      continue;
    }

    // Compare the received payload size to what is in the header.
    const size_t rx_payload_size =
        read_size - sizeof(bluetooth::hci::EventHeader);
    const size_t size_from_header =
        packet->view().header().parameter_total_size;
    if (size_from_header != rx_payload_size) {
      std::cerr << "CommandChannel: Malformed event packet - "
                << "payload size from header (" << size_from_header << ")"
                << " does not match received payload size: " << rx_payload_size
                << std::endl;
      continue;
    }

    packet->InitializeFromBuffer();

    if (event_callback_) {
      event_callback_(*packet);
    } else {
      std::cerr << "CommandChannel: Event received with no handler:"
                << packet->event_code() << std::endl;
    }
  }
  return ASYNC_WAIT_AGAIN;
}

async_wait_result_t CommandChannel::OnChannelReady(
    async_t* async,
    zx_status_t status,
    const zx_packet_signal_t* signal) {
  // Just handle this.
  return HandleChannelReady(channel_, async, status, signal);
}

async_wait_result_t CommandChannel::OnAclChannelReady(
    async_t* async,
    zx_status_t status,
    const zx_packet_signal_t* signal) {
  // This is probably a Command packet response from a Secure Send command.
  std::cerr << "CommandChannel: ACL Data packet received, treating as a command packet.." << std::endl;
  return HandleChannelReady(acl_channel_, async, status, signal);
}
