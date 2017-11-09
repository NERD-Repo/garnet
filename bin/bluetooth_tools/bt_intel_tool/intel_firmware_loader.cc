// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "intel_firmware_loader.h"

#include <fbl/string_printf.h>
#include <fbl/unique_fd.h>
#include <fcntl.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <zircon/status.h>
#include <zx/event.h>
#include <zx/time.h>

#include <iostream>
#include <limits>

#include "bt_intel.h"

namespace bt_intel {

namespace {

// A file mapped into memory that we can grab chunks from.
class MemoryFile {
 public:
  MemoryFile(const std::string& filename)
      : fd_(open(filename.c_str(), O_RDONLY)), mapped_(nullptr) {
    if (!bool(fd_)) {
      std::cerr << "Failed to open file " << filename << " : "
                << strerror(errno) << std::endl;
      return;
    }

    struct stat file_stats;
    fstat(fd_.get(), &file_stats);
    size_ = file_stats.st_size;
    std::cerr << "Mapping " << size_ << " bytes of " << filename << std::endl;

    mapped_ = mmap(nullptr, size_, PROT_READ, MAP_PRIVATE, fd_.get(), 0);
    if (mapped_ == MAP_FAILED) {
      std::cerr << "Failed to map file to memory: " << strerror(errno)
                << std::endl;
      mapped_ = nullptr;
    }
  };

  ~MemoryFile() {
    if (mapped_) {
      munmap(mapped_, size_);
    }
  }

  size_t size() const { return size_; }

  bool is_valid() const { return mapped_ != nullptr; }

  const uint8_t* at(size_t offset) const {
    return static_cast<uint8_t*>(mapped_) + offset;
  }

  bluetooth::common::BufferView view(
      size_t offset,
      size_t length = std::numeric_limits<size_t>::max()) const {
    if (!is_valid() || (offset > size_)) {
      return bluetooth::common::BufferView();
    }
    if (length > (size_ - offset)) {
      length = (size_ - offset);
    }
    return bluetooth::common::BufferView(mapped_, length);
  }

 private:
  // The actual file descriptor.
  fbl::unique_fd fd_;

  // pointer to the file's space in memory
  void* mapped_;

  // Size of the file in memory
  size_t size_;
};

void SecureSend(CommandChannel* channel,
                uint8_t type,
                const bluetooth::common::BufferView& bytes) {
  size_t left = bytes.size();
  while (left > 0) {
    size_t frag_len = left > 252 ? 252 : left;
    std::cout << "IntelFirmwareLoader: Secure Sending " << frag_len << " of " << left << " bytes" << std::endl;
    auto cmd = bluetooth::hci::CommandPacket::New(kSecureSend, frag_len + 1);
    auto data = cmd->mutable_view()->mutable_payload_data();
    data[0] = type;
    data.Write(bytes.view(bytes.size() - left, frag_len), 1);

    channel->SendCommandSync(cmd->view(), [](const auto& event) {
      std::cout << "IntelFirmwareLoader: Secure Send response: "
                << std::to_string(event.event_code()) << std::endl;
      if (event.event_code() == 0xff) {
        const auto& params = event.view().template payload<IntelSecureSendEventParams>();
        std::cout << "IntelFirmwareLoader: Secure Send result: (" << params.result
                  << ", " << params.opcode << ", " << params.status << ")" << std::endl;
      }
    });
    left -= frag_len;
  }
}

}  // namespace

IntelFirmwareLoader::LoadStatus IntelFirmwareLoader::LoadBseq(
    const std::string& filename) {
  MemoryFile file(filename);

  if (!file.is_valid()) {
    std::cerr << "Failed to open firmware file." << std::endl;
    return LoadStatus::kError;
  }

  size_t ptr = 0;

  // A bseq file consists of a sequence of:
  // - [0x01] [command w/params]
  // - [0x02] [expected event w/params]
  while (file.size() - ptr > sizeof(bluetooth::hci::CommandHeader)) {
    // Parse the next items
    if (*file.at(ptr) != 0x01) {
      std::cerr << "IntelFirmwareLoader: Error: malformed file, expected "
                   "Command Packet marker"
                << std::endl;
      return LoadStatus::kError;
    }
    ptr++;
    bluetooth::common::BufferView command_view = file.view(ptr);
    bluetooth::common::PacketView<bluetooth::hci::CommandHeader> command(
        &command_view);
    command = bluetooth::common::PacketView<bluetooth::hci::CommandHeader>(
        &command_view, command.header().parameter_total_size);
    ptr += command.size();
    if ((file.size() <= ptr) || (*file.at(ptr) != 0x02)) {
      std::cerr << "IntelFirmwareLoader: Error: malformed file, expected Event "
                   "Packet marker"
                << std::endl;
      return LoadStatus::kError;
    }
    std::deque<std::unique_ptr<bluetooth::hci::EventPacket>> events;
    while ((file.size() <= ptr) || (*file.at(ptr) == 0x02)) {
      ptr++;
      // TODO(jamuraa): we should probably do this without copying,
      // maybe make a way to initialize event packets backed by unowned
      // memor
      auto event = bluetooth::hci::EventPacket::New(0u);
      memcpy(event->mutable_view()->mutable_header(), file.at(ptr),
             sizeof(bluetooth::hci::EventHeader));
      ptr += event->view().size();
      event->InitializeFromBuffer();
      memcpy(event->mutable_view()->mutable_payload_bytes(), file.at(ptr),
             event->view().payload_size());
      ptr += event->view().payload_size();
      events.push_back(std::move(event));
    }

    if (!RunCommandAndExpect(command, std::move(events))) {
      return LoadStatus::kError;
    }
  }

  return LoadStatus::kComplete;
}

bool IntelFirmwareLoader::LoadSfi(const std::string& filename) {
  MemoryFile file(filename);

  if (file.size() < 644) {
    std::cerr << "IntelFirmwareLoader: SFI file not long enough: "
              << file.size() << " < 644" << std::endl;
    return false;
  }

  size_t ptr = 0;
  // SFI File format:
  // [128 bytes CSS Header]
  SecureSend(channel_, 0x00, file.view(ptr, 128));
  //ptr += 128;
  // [256 bytes PKI]
  SecureSend(channel_, 0x03, file.view(ptr, 256));
  ptr += 256;
  // [256 bytes signature info]
  SecureSend(channel_, 0x02, file.view(ptr, 256));
  ptr += 256;
  // [N bytes of data]
  // Note: this is actually a bunch of Command Packets, padded with
  // NOP commands so they sit on 4-byte boundaries, but we write it to
  // Secure Send area anyway so I don't see the point in parsing them.
  SecureSend(channel_, 0x01, file.view(ptr, file.size() - 644));

  return true;
}

bool IntelFirmwareLoader::RunCommandAndExpect(
    const bluetooth::common::PacketView<bluetooth::hci::CommandHeader>& command,
    std::deque<std::unique_ptr<bluetooth::hci::EventPacket>>&& events) {
  zx::event events_done;
  zx::event::create(0, &events_done);

  size_t events_received = 0;
  auto event_cb = [&events_received, &events, &events_done](
                      const bluetooth::hci::EventPacket& evt_packet) {
    auto expected = std::move(events.front());
    events.pop_front();
    if (evt_packet.view().size() != expected->view().size()) {
      events_done.signal(0, ZX_USER_SIGNAL_0);
      return;
    }
    if (memcmp(evt_packet.view().data().data(), expected->view().data().data(),
               expected->view().size()) != 0) {
      events_done.signal(0, ZX_USER_SIGNAL_0);
      return;
    }
    events_received++;
  };

  channel_->SetEventCallback(event_cb);

  channel_->SendCommand(command);

  zx_signals_t signalled;
  zx_status_t status = events_done.wait_one(
      ZX_USER_SIGNAL_0, zx::deadline_after(ZX_SEC(1)), &signalled);

  channel_->SetEventCallback(nullptr);

  if (status == ZX_OK) {
    return true;
  }

  if (status == ZX_ERR_TIMED_OUT) {
    std::cerr << "IntelFirmwareLoader: timed out waiting for events"
              << std::endl;
  } else {
    std::cerr << "IntelFirmwareLoader: error waiting for events"
              << zx_status_get_string(status) << std::endl;
  }
  return false;
}

}  // namespace bt_intel
