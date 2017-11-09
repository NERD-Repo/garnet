// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

#include "garnet/bin/bluetooth_tools/lib/command_dispatcher.h"
#include "lib/fxl/memory/ref_ptr.h"
#include "lib/fxl/tasks/task_runner.h"

#include "command_channel.h"

namespace bt_intel {

class CommandData final {
 public:
  CommandData(CommandChannel* cmd_channel) : cmd_channel_(cmd_channel) {}

  CommandChannel* cmd_channel() const { return cmd_channel_; }

 private:
  CommandChannel* cmd_channel_;
};

void RegisterCommands(const CommandData* data,
                      bluetooth::tools::CommandDispatcher* dispatcher);

}  // namespace bt_intel
