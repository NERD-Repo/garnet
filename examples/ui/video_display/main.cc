// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <trace-provider/provider.h>

#include "garnet/examples/ui/video_display/view.h"
#include "lib/fsl/tasks/message_loop.h"
#include "lib/fxl/command_line.h"
#include "lib/fxl/log_settings_command_line.h"
#include "lib/ui/view_framework/view_provider_app.h"
#include "lib/ui/presentation/fidl/presenter.fidl.h"

int main(int argc, const char** argv) {
  auto command_line = fxl::CommandLineFromArgcArgv(argc, argv);
  if (!fxl::SetLogSettingsFromCommandLine(command_line))
    return 1;

  fsl::MessageLoop loop;
  trace::TraceProvider trace_provider(loop.async());

#if 0

  mozart::ViewProviderApp app([](mozart::ViewContext view_context) {
    return std::make_unique<video_display::View>(
        view_context.application_context, std::move(view_context.view_manager),
        std::move(view_context.view_owner_request));
  });

#else
  auto application_context_ = app::ApplicationContext::CreateFromStartupInfo();
  mozart::ViewProviderService view_provider(
      application_context_.get(), [](mozart::ViewContext view_context) {
        return std::make_unique<video_display::View>(
            view_context.application_context,
            std::move(view_context.view_manager),
            std::move(view_context.view_owner_request));
      });

  f1dl::InterfaceHandle<mozart::ViewOwner> view_owner;
  view_provider.CreateView(view_owner.NewRequest(), nullptr);
  // Ask the presenter to display it.
  auto presenter =
      application_context_->ConnectToEnvironmentService<mozart::Presenter>();
  presenter->Present(std::move(view_owner), nullptr);

// loop.task_runner()->PostDelayedTask(
// [&loop] {
// FXL_LOG(INFO) << "Quitting.";
// loop.QuitNow();
// },
// fxl::TimeDelta::FromSeconds(50));
#endif
  loop.Run();
  return 0;
}
