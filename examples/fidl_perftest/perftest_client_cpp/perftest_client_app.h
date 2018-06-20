// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef GARNET_EXAMPLES_FIDL_PERFTEST_CLIENT_H_
#define GARNET_EXAMPLES_FIDL_PERFTEST_CLIENT_H_

#include <fidl/examples/perftest/cpp/fidl.h>

#include "lib/app/cpp/startup_context.h"

namespace perftest {

class PerfTestClient {
 public:
  PerfTestClient();
  PerfTestClient(std::unique_ptr<fuchsia::sys::StartupContext> context);

  fidl::examples::perftest::PerfTestPtr& perftest() { return perftest_; }

  void RunTest(std::string server_url);

 private:
  PerfTestClient(const PerfTestClient&) = delete;
  PerfTestClient& operator=(const PerfTestClient&) = delete;

  std::unique_ptr<fuchsia::sys::StartupContext> context_;
  fuchsia::sys::Services perftest_provider_;
  fuchsia::sys::ComponentControllerPtr controller_;
  fidl::examples::perftest::PerfTestPtr perftest_;
};

}  // namespace perftest

#endif  // GARNET_EXAMPLES_FIDL_PERFTEST_CLIENT_H_
