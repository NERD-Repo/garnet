// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "perftest_client_app.h"
#include "lib/app/cpp/startup_context.h"

namespace perftest {
    PerfTestClient::PerfTestClient()
        : PerfTestClient(fuchsia::sys::StartupContext::CreateFromStartupInfo()) {}

    PerfTestClient::PerfTestClient(
        std::unique_ptr<fuchsia::sys::StartupContext> context)
    : context_(std::move(context)) {}

    void PerfTestClient::RunTest(std::string server_url) {
        fuchsia::sys::LaunchInfo launch_info;
        launch_info.url = server_url;
        launch_info.directory_request = perftest_provider_.NewRequest();
        context_->launcher()->CreateComponent(
            std::move(launch_info), controller_.NewRequest());

        perftest_provider_.ConnectToService(perftest_.NewRequest().TakeChannel(),
            fidl::examples::perftest::PerfTest::Name_);
    }
}