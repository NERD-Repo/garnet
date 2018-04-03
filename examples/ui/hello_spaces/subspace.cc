// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/examples/ui/hello_spaces/app.h"
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/command_line.h"
#include "lib/fxl/log_settings_command_line.h"

int main(int argc, const char** argv) {
  auto command_line = fxl::CommandLineFromArgcArgv(argc, argv);
  if (!fxl::SetLogSettingsFromCommandLine(command_line)) return 1;

  // Set up the main message loop and app.  The app's constructor will bind all
  // of the services it needs.
  fsl::MessageLoop loop;
  hello_spaces::App subspace_app(hello_spaces::AppType::SUBSPACE);

  // Run the main message loop.
  loop.task_runner()->PostDelayedTask(
      [&loop] {
        FXL_LOG(INFO) << "HARD Quitting.";
        loop.QuitNow();
      },
      fxl::TimeDelta::FromSeconds(20));
  loop.Run();

  return 0;
}
