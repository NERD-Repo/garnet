// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <fuchsia/cpp/ui.h>

#include "garnet/examples/ui/hello_spaces/app.h"
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/command_line.h"
#include "lib/fxl/log_settings_command_line.h"

int main(int argc, const char** argv) {
  auto command_line = fxl::CommandLineFromArgcArgv(argc, argv);
  if (!fxl::SetLogSettingsFromCommandLine(command_line)) return 1;

  fsl::MessageLoop loop;
  hello_spaces::App app;
  loop.task_runner()->PostDelayedTask(
      [&loop] {
        FXL_LOG(INFO) << "Quitting.";
        loop.QuitNow();
      },
      fxl::TimeDelta::FromSeconds(50));
  loop.Run();
  return 0;
}
