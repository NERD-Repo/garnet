// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef GARNET_EXAMPLES_FIDL_PERFTEST_SERVER_H_
#define GARNET_EXAMPLES_FIDL_PERFTEST_SERVER_H_

#include <fidl/examples/perftest/cpp/fidl.h>

#include "lib/app/cpp/startup_context.h"
#include "lib/fidl/cpp/binding.h"

namespace perftest {

class PerfTestServer : public fidl::examples::perftest::PerfTest {
public:
  PerfTestServer(fidl::StringPtr name);
  virtual void Name(NameCallback callback);
  virtual void TestCases(TestCasesCallback callback);

protected:
  PerfTestServer(fidl::StringPtr name, std::unique_ptr<fuchsia::sys::StartupContext> context);

private:
  PerfTestServer(const PerfTestServer&) = delete;
  PerfTestServer& operator=(const PerfTestServer&) = delete;
  std::unique_ptr<fuchsia::sys::StartupContext> context_;
  fidl::BindingSet<PerfTest> bindings_;

  // The name of this performance test.
  fidl::StringPtr name_;
};

}  // namespace perftest

#endif  // GARNET_EXAMPLES_FIDL_PERFTEST_SERVER_H_
