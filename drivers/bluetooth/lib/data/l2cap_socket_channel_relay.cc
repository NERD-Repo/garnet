// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "l2cap_socket_channel_relay.h"

#include "garnet/drivers/bluetooth/lib/data/socket_channel_relay.cc"

// The functions below must be in |btlib::l2cap|, so that they can be found via
// argument-dependent lookup (ADL). The functions are |static|, to
// avoid conflicting with any other definition of those names in |btlib::l2cap|.
namespace btlib::l2cap {
static bool ValidateRxData(const SDU& sdu) { return sdu.is_valid(); }
static size_t GetRxDataLen(const SDU& sdu) { return sdu.length(); }
static bool InvokeWithRxData(
    fit::function<void(const common::ByteBuffer& data)> callback,
    const SDU& sdu) {
  return SDU::Reader(&sdu).ReadNext(sdu.length(), callback);
}
}  // namespace btlib::l2cap

namespace btlib::data::internal {
template class SocketChannelRelay<l2cap::Channel, l2cap::ChannelId, l2cap::SDU>;
}  // namespace btlib::data::internal
