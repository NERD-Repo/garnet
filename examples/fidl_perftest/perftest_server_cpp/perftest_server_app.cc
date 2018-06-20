// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <fbl/string.h>

#include "perftest_server_app.h"

#include "lib/app/cpp/startup_context.h"
#include "lib/fidl/cpp/binding.h"

namespace perftest {

// Initializing constructor
PerfTestServer::PerfTestServer(fidl::StringPtr name)
    : PerfTestServer(name, fuchsia::sys::StartupContext::CreateFromStartupInfo()) {}

PerfTestServer::PerfTestServer(
    fidl::StringPtr name, std::unique_ptr<fuchsia::sys::StartupContext> context)
    : context_(std::move(context)),
      name_(std::move(name)) {
  context_->outgoing().AddPublicService<PerfTest>(
      [this](fidl::InterfaceRequest<PerfTest> request) {
        bindings_.AddBinding(this, std::move(request));
      });
}

void PerfTestServer::Name(NameCallback callback) {
    callback(name_);
}

void PerfTestServer::TestCases(TestCasesCallback callback) {
    fidl::VectorPtr<fidl::examples::perftest::TestCase> test_cases;

    fidl::examples::perftest::TestCase test_case;
    test_case.name = fidl::StringPtr("ClockGetThread");
    test_case.unit = fidl::examples::perftest::Unit::NANOSECONDS;
    std::vector<double> values(3, 1.5);
    test_case.values = fidl::VectorPtr<double>(std::move(values));

    test_cases.push_back(std::move(test_case));
    callback(std::move(test_cases));
}

}  // namespace perftest