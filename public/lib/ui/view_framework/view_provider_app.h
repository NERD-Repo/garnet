// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef APPS_MOZART_LIB_VIEW_FRAMEWORK_VIEW_PROVIDER_APP_H_
#define APPS_MOZART_LIB_VIEW_FRAMEWORK_VIEW_PROVIDER_APP_H_

#include <memory>

#include "lib/app/cpp/application_context.h"
#include "lib/ui/view_framework/view_provider_service.h"
#include "lib/fxl/macros.h"

namespace mozart {

// Provides a skeleton for an entire application that only offers
// a view provider service.
// This is only intended to be used for simple example programs.
class ViewProviderApp {
 public:
  explicit ViewProviderApp(ViewFactory factory);
  ~ViewProviderApp();

 private:
  std::unique_ptr<app::ApplicationContext> application_context_;
  ViewProviderService service_;

  FXL_DISALLOW_COPY_AND_ASSIGN(ViewProviderApp);
};

}  // namespace mozart

#endif  // APPS_MOZART_LIB_VIEW_FRAMEWORK_VIEW_PROVIDER_APP_H_
