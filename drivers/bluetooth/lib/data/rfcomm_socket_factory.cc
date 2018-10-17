// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "rfcomm_socket_factory.h"

#include "socket_factory.cc"

namespace btlib {
namespace data {
namespace internal {

template class SocketFactory<rfcomm::Channel, rfcomm::DLCI,
                             common::ByteBufferPtr>;

}  // namespace internal
}  // namespace data
}  // namespace btlib
