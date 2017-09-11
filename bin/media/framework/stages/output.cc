// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/bin/media/framework/stages/output.h"

#include "garnet/bin/media/framework/engine.h"
#include "garnet/bin/media/framework/stages/stage_impl.h"

namespace media {

Output::Output(StageImpl* stage, size_t index) : stage_(stage), index_(index) {}

Output::~Output() {}

void Output::Connect(Input* input) {
  FXL_DCHECK(input);
  FXL_DCHECK(!mate_);
  mate_ = input;
}

void Output::SetCopyAllocator(PayloadAllocator* copy_allocator) {
  FXL_DCHECK(connected());
  copy_allocator_ = copy_allocator;
}

Demand Output::demand() const {
  FXL_DCHECK(mate_);
  return mate_->demand();
}

void Output::SupplyPacket(PacketPtr packet) const {
  FXL_DCHECK(packet);
  FXL_DCHECK(mate_);
  FXL_DCHECK(demand() != Demand::kNegative);

  if (copy_allocator_ != nullptr) {
    // Need to copy the packet due to an allocation conflict.
    size_t size = packet->size();
    void* buffer;

    if (size == 0) {
      buffer = nullptr;
    } else {
      buffer = copy_allocator_->AllocatePayloadBuffer(size);
      if (buffer == nullptr) {
        FXL_LOG(WARNING) << "allocator starved copying output";
        return;
      }
      memcpy(buffer, packet->payload(), size);
    }

    packet =
        Packet::Create(packet->pts(), packet->pts_rate(), packet->keyframe(),
                       packet->end_of_stream(), size, buffer, copy_allocator_);
  }

  mate_->PutPacket(std::move(packet));
}

}  // namespace media
