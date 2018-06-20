// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <fbl/string.h>

#include "perftest_server_app.h"

#include "lib/app/cpp/startup_context.h"
#include "lib/fidl/cpp/binding.h"

namespace perftest {

// Initializing constructor
PerfTestServer::PerfTestServer(fidl::string name)
    : PerfTestServer(name, fuchsia::sys::StartupContext::CreateFromStartupInfo()) {}

PerfTestServer::PerfTestServer(
    fidl::string name, std::unique_ptr<fuchsia::sys::StartupContext> context)
    :   name_(std::move(name))
        context_(std::move(context)) {
  context_->outgoing().AddPublicService<PerfTest>(
      [this](fidl::InterfaceRequest<PerfTest> request) {
        bindings_.AddBinding(this, std::move(request));
      });
}

void PerfTestServer::Name(NameCallback callback) {
    callback(name_);
}

void PerfTestServer::TestCases(TestCasesCallback callback) {
    fidl::Vector<fidl::examples::fidl_perftest::TestCase> test_cases;

    // fidl::examples::fidl_perftest::TestCase test_case;
    
    // Return empty vector for now.
    callback(std::move(test_cases));
}  // namespace perftest
