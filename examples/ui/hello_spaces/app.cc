// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/examples/ui/hello_spaces/app.h"

#include <fuchsia/cpp/gfx.h>
#include <zx/eventpair.h>

#include "lib/fidl/cpp/interface_handle.h"
#include "lib/fidl/cpp/interface_request.h"
#include "lib/fxl/logging.h"
#include "lib/ui/scenic/fidl_helpers.h"

namespace hello_spaces {

// Parameters for creating a view.
struct SpaceContext {
  component::ApplicationContext* application_context;
  // fidl::InterfaceRequest<views_v1_token::ViewOwner> view_owner_request;
  fidl::InterfaceRequest<component::ServiceProvider> outgoing_services;
};

// A callback to create a space in response to a call to
// |SpaceProvider.CreateSpace()|.
using SpaceFactory = std::function<void(SpaceContext context)>;

class SpaceProviderService : public gfx::SpaceProvider {
 public:
  explicit SpaceProviderService(
      component::ApplicationContext* application_context, SpaceFactory factory)
      : application_context_(application_context), space_factory_fn_(factory) {
    FXL_DCHECK(application_context_);

    application_context_->outgoing_services()->AddService<gfx::SpaceProvider>(
        [this](fidl::InterfaceRequest<gfx::SpaceProvider> request) {
          bindings_.AddBinding(this, std::move(request));
        });
  }

  ~SpaceProviderService() {
    application_context_->outgoing_services()
        ->RemoveService<gfx::SpaceProvider>();
  }

  // |ui::SpaceProvider|
  void CreateSpace(
      ::zx::eventpair token,
      ::fidl::InterfaceRequest<component::ServiceProvider> incoming_services,
      ::fidl::InterfaceHandle<component::ServiceProvider> outgoing_services)
      override {}

 private:
  component::ApplicationContext* application_context_;
  SpaceFactory space_factory_fn_;

  fidl::BindingSet<gfx::SpaceProvider> bindings_;

  FXL_DISALLOW_COPY_AND_ASSIGN(SpaceProviderService);
};

static std::unique_ptr<SpaceProviderService> s_space1;

App::App()
    : application_context_(
          component::ApplicationContext::CreateFromStartupInfo()),
      loop_(fsl::MessageLoop::GetCurrent()) {
  // Create the "cross-process" sub-spaces.
  s_space1 = std::make_unique<SpaceProviderService>(
      application_context_.get(), [](SpaceContext space_context) {});
  // Connect to the global Scenic service.
  scenic_ = application_context_->ConnectToEnvironmentService<ui::Scenic>();
  scenic_.set_error_handler([this] {
    FXL_LOG(INFO) << "Lost connection to Scenic service.";
    loop_->QuitNow();
  });
  scenic_->GetDisplayInfo(
      [this](gfx::DisplayInfo display_info) { Init(std::move(display_info)); });
}

void App::Init(gfx::DisplayInfo display_info) {
  FXL_LOG(INFO) << "Creating new Session";

  // TODO: set up SessionListener.
  session_ = std::make_unique<scenic_lib::Session>(scenic_.get());
  session_->set_error_handler([this] {
    FXL_LOG(INFO) << "Session terminated.";
    loop_->QuitNow();
  });

  // Wait kSessionDuration seconds, and close the session.
  constexpr int kSessionDuration = 40;
  loop_->task_runner()->PostDelayedTask(
      [this] { ReleaseSessionResources(); },
      fxl::TimeDelta::FromSeconds(kSessionDuration));

  // Set up initial scene.
  const float display_width = static_cast<float>(display_info.width_in_px);
  const float display_height = static_cast<float>(display_info.height_in_px);
  CreateExampleScene(display_width, display_height);

  Update(zx_clock_get(ZX_CLOCK_MONOTONIC));
}

void App::ReleaseSessionResources() {
  FXL_LOG(INFO) << "Closing session.";

  compositor_.reset();
  camera_.reset();
  session_.reset();
}

void App::Update(uint64_t next_presentation_time) {
  // Present
  session_->Present(
      next_presentation_time, [this](images::PresentationInfo info) {
        Update(info.presentation_time + info.presentation_interval);
      });
}

void App::CreateExampleScene(float display_width, float display_height) {
  auto session_ptr = session_.get();

  // The top-level nesting for drawing anything is compositor -> layer-stack
  // -> layer.  Layer content can come from an image, or by rendering a scene.
  // In this case, we do the latter, so we nest layer -> renderer -> camera ->
  // scene.
  compositor_ = std::make_unique<scenic_lib::DisplayCompositor>(session_ptr);
  scenic_lib::LayerStack layer_stack(session_ptr);
  scenic_lib::Layer layer(session_ptr);
  scenic_lib::Renderer renderer(session_ptr);
  scenic_lib::Scene scene(session_ptr);
  camera_ = std::make_unique<scenic_lib::Camera>(scene);

  compositor_->SetLayerStack(layer_stack);
  layer_stack.AddLayer(layer);
  layer.SetSize(display_width, display_height);
  layer.SetRenderer(renderer);
  renderer.SetCamera(camera_->id());

  // Set up lights.
  scenic_lib::AmbientLight ambient_light(session_ptr);
  scenic_lib::DirectionalLight directional_light(session_ptr);
  scene.AddLight(ambient_light);
  scene.AddLight(directional_light);
  ambient_light.SetColor(0.3f, 0.3f, 0.3f);
  directional_light.SetColor(0.7f, 0.7f, 0.7f);
  directional_light.SetDirection(1.f, 1.f, -2.f);

  // Create an EntityNode to serve as the scene root.
  scenic_lib::EntityNode root_node(session_ptr);
  scene.AddChild(root_node.id());

  static constexpr float kBackgroundMargin = 100.f;
  static const float background_width =
      (display_width - 1.f * kBackgroundMargin);
  static const float background_height =
      display_height - 0.5f * kBackgroundMargin;
  scenic_lib::ShapeNode background_shape(session_ptr);
  scenic_lib::RoundedRectangle background_rect(
      session_ptr, background_width, background_height, 20, 20, 80, 10);
  scenic_lib::Material background_material(session_ptr);
  background_material.SetColor(120, 255, 120, 255);

  background_shape.SetShape(background_rect);
  background_shape.SetMaterial(background_material);
  root_node.AddPart(background_shape);

  // static constexpr float kPaneMargin = 100.f;
  // static const float pane_width = (display_width - 3.f * kPaneMargin) / 2.f;
  // static const float pane_height = display_height - 2.f * kPaneMargin;
  //
  // // The root node will enclose two "panes", each with a rounded-rect part
  // // that acts as a background clipper.
  // scenic_lib::RoundedRectangle pane_shape(session_ptr, pane_width,
  // pane_height,
  //                                         20, 20, 80, 10);
  // scenic_lib::Material pane_material(session_ptr);
  // pane_material.SetColor(120, 120, 255, 255);

  // scenic_lib::EntityNode pane_node_1(session_ptr);
  // scenic_lib::ShapeNode pane_bg_1(session_ptr);
  // pane_bg_1.SetShape(pane_shape);
  // pane_bg_1.SetMaterial(pane_material);
  // pane_node_1.AddPart(pane_bg_1);
  // pane_node_1.SetTranslation(kPaneMargin + pane_width * 0.5,
  //                            kPaneMargin + pane_height * 0.5, 20);
  // pane_node_1.SetClip(0, true);
  // root_node.AddChild(pane_node_1);
  //
  // scenic_lib::EntityNode pane_node_2(session_ptr);
  // scenic_lib::ShapeNode pane_bg_2(session_ptr);
  // pane_bg_2.SetShape(pane_shape);
  // pane_bg_2.SetMaterial(pane_material);
  // pane_node_2.AddPart(pane_bg_2);
  // pane_node_2.SetTranslation(kPaneMargin * 2 + pane_width * 1.5,
  //                            kPaneMargin + pane_height * 0.5, 20);
  // pane_node_2.SetClip(0, true);
  // root_node.AddChild(pane_node_2);
}

}  // namespace hello_spaces
