// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "rfcomm_socket_channel_relay.h"

#include "garnet/drivers/bluetooth/lib/data/socket_channel_relay.cc"

// The functions below must be in |btlib::common|, so that they can be found via
// argument-dependent lookup (ADL). The functions are |static|, to avoid
// conflicting with any other definition of those names in |btlib::common|.
namespace btlib::common {
using BufT = ByteBufferPtr;
static bool ValidateRxData(const BufT& buf) { return buf != nullptr; }
static size_t GetRxDataLen(const BufT& buf) { return buf->size(); }
static bool InvokeWithRxData(
    fit::function<void(common::ByteBuffer& data)> callback, const BufT& buf) {
  callback(*buf);
  return true;
}
}  // namespace btlib::common

namespace btlib::data::internal {
template class SocketChannelRelay<rfcomm::Channel, rfcomm::DLCI,
                                  common::ByteBufferPtr>;
}  // namespace btlib::data::internal
