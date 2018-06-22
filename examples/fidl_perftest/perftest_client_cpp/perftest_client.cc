// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#define FIDL_ENABLE_LEGACY_WAIT_FOR_RESPONSE

#include <fidl/examples/perftest/cpp/fidl.h>
#include <lib/async-loop/cpp/loop.h>
#include <lib/async/default.h>
#include <lib/zx/process.h>
#include <zircon/processargs.h>

#include "perftest_client_app.h"
#include "lib/app/cpp/startup_context.h"
#include "lib/svc/cpp/services.h"

int main(int argc, const char** argv) {
  std::string server_url = "perftest_server_cpp";
  async::Loop loop(&kAsyncLoopConfigMakeDefault);

  perftest::PerfTestClient client;
  client.RunTest(server_url);

  // Get test name.
  client.perftest()->Name([](fidl::StringPtr name) {
    printf("****** PerfTest Name: %s\n", name->data());
  });
  client.perftest().WaitForResponse();

  // Get test cases.
  client.perftest()->TestCases([](fidl::VectorPtr<fidl::examples::perftest::TestCase> response) {
    std::vector<fidl::examples::perftest::TestCase> test_cases = response.take();
    for (auto test_case = test_cases.begin(); test_case != test_cases.end(); ++test_case) {
      printf("-- case: %s\n", test_case->name->data());
      printf("-- unit: %d\n", test_case->unit);
      printf("-- values: {");
      std::vector<double> values = test_case->values.take();
      for (auto value = values.begin(); value != values.end(); ++value) {
        printf(" %f ", *value);
      }
      printf("}\n");
    }
  });
  return client.perftest().WaitForResponse();

//   return app.echo().WaitForResponse();
}