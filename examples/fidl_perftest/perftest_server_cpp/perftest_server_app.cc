// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <fbl/string.h>

// Common perf testing library used by zircon benchmarks.
#include <perftest/perftest.h>

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
    // Run zircon benchmarks.  In reality this would be a callback passed to the
    // constructor of this class, which returns std::vector<TestCaseResults>.
    fbl::Vector<perftest::TestCaseResults> results = perftest::RunPerfTests();
    
    // Initialize the response.
    fidl::VectorPtr<fidl::examples::perftest::TestCase> test_cases;

    // Add each TestCaseResults to the response.
    for (int i=0; i < int(results.size()); i++) {
        perftest::TestCaseResults tcr = std::move(results[i]);
        fidl::examples::perftest::TestCase test_case;
        test_case.name = fidl::StringPtr(tcr.label().c_str());
        // Normally we'd map this to the proper unit instead of hardcoding.
        test_case.unit = fidl::examples::perftest::Unit::NANOSECONDS;
        test_case.values = fidl::VectorPtr<double>();
        for (auto val = tcr.values()->begin(); val != tcr.values()->end(); val++) {
          test_case.values.push_back(*val);
        }
        // Add to the response.
        test_cases.push_back(std::move(test_case));
    }
    // fidl::examples::perftest::TestCase test_case;
    // test_case.name = fidl::StringPtr("ClockGetThread");
    // test_case.unit = fidl::examples::perftest::Unit::NANOSECONDS;
    // std::vector<double> values(3, 1.5);
    // test_case.values = fidl::VectorPtr<double>(std::move(values));

    // test_cases.push_back(std::move(test_case));
    callback(std::move(test_cases));
}

}  // namespace perftest