// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <thread>

#include "garnet/examples/ui/hello_spaces/app.h"
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/command_line.h"
#include "lib/fxl/log_settings_command_line.h"

void run_app(hello_spaces::AppType type) {
  fsl::MessageLoop loop;
  hello_spaces::App app(type);
  loop.task_runner()->PostDelayedTask(
      [&app, &loop] {
        FXL_LOG(INFO) << app.AppIdentifier() << " HARD Quitting.";
        loop.QuitNow();
      },
      fxl::TimeDelta::FromSeconds(20));
  loop.Run();
}

int main(int argc, const char** argv) {
  auto command_line = fxl::CommandLineFromArgcArgv(argc, argv);
  if (!fxl::SetLogSettingsFromCommandLine(command_line)) return 1;

  // Run the controller and any client(s) on their own threads, so that each one
  // can have its own MessageLoop.
  std::thread controller_thread(run_app, hello_spaces::AppType::Controller);
  std::thread guest_thread(run_app, hello_spaces::AppType::Guest);
  controller_thread.join();
  guest_thread.join();

  return 0;
}
