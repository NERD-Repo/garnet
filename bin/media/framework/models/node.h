// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

#include "garnet/bin/media/framework/packet.h"
#include "garnet/bin/media/framework/payload_allocator.h"
#include "lib/fxl/functional/closure.h"
#include "lib/fxl/tasks/task_runner.h"

namespace media {

// Base class for all nodes.
template <typename TStage>
class Node {
 public:
  virtual ~Node() {}

  // Sets |stage_|. This method is called only by the graph.
  void SetStage(TStage* stage) {
    FXL_DCHECK(stage_ == nullptr);
    stage_ = stage;
  }

  // Returns the task runner to use for this node. The default implementation
  // returns nullptr, indicating that this node can use whatever task runner
  // is provided for it, either via the |Graph| constructor or via the
  // |Graph::Add| methods.
  virtual fxl::RefPtr<fxl::TaskRunner> GetTaskRunner() { return nullptr; }

 protected:
  // Returns a reference to the stage for this node.
  TStage& stage() {
    FXL_DCHECK(stage_);
    return *stage_;
  }

  // Posts a task to run as soon as possible. A task posted with this method is
  // run exclusive of any other such tasks.
  void PostTask(const fxl::Closure& task) {
    FXL_DCHECK(stage_);
    stage_->PostTask(task);
  }

 private:
  TStage* stage_ = nullptr;

  friend class Graph;
};

}  // namespace media
