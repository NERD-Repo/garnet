// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "garnet/examples/ui/hello_spaces/app.h"

#include <fuchsia/cpp/gfx.h>
#include <zircon/process.h>
#include <zircon/processargs.h>
#include <zircon/types.h>
#include <zx/channel.h>
#include <zx/eventpair.h>

#include "lib/app/cpp/environment_services.h"
#include "lib/fxl/logging.h"
#include "lib/svc/cpp/services.h"
#include "lib/ui/scenic/fidl_helpers.h"

namespace hello_spaces {

// Parameters for creating a space.
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
  explicit SpaceProviderService(component::ApplicationContext* app_context,
                                SpaceFactory factory)
      : app_context_(app_context), space_factory_fn_(factory) {
    FXL_DCHECK(app_context_);

    application_context_->outgoing_services()->AddService<gfx::SpaceProvider>(
        [this](fidl::InterfaceRequest<gfx::SpaceProvider> request) {
          FXL_LOG(INFO) << "Bound service iface to impl!";

          bindings_.AddBinding(this, std::move(request));
        },
        "space_provider");
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
      override {
    FXL_LOG(INFO) << "In SpaceProvider::CreateSpace!";
  }

 private:
  component::ApplicationContext* app_context_;
  SpaceFactory space_factory_fn_;

  fidl::BindingSet<gfx::SpaceProvider> bindings_;

  FXL_DISALLOW_COPY_AND_ASSIGN(SpaceProviderService);
};

static const char* AppTypeString(AppType type) {
  if (type == AppType::CONTAINER) {
    return "[CONTAINER] ";
  } else if (type == AppType::SUBSPACE) {
    return "[SUBSPACE] ";
  } else {
    FXL_DCHECK(false);
  }
  return nullptr;
}

static component::ApplicationControllerPtr s_subspace_controller;

App::App(AppType type) : type_(type) {
  // // Obtain the handle to our export directory.  It's needed so that we can
  // // bind to our own exported services.
  // zx_handle_t app_directory = zx_get_startup_handle(PA_DIRECTORY_REQUEST);
  // app_context_ = std::make_unique<component::ApplicationContext>(
  //     component::subtle::CreateStaticServiceRootHandle(),
  //     zx::channel(app_directory));

  // Create the ApplicationContext.
  FXL_LOG(INFO) << AppTypeString(type_) << "Creating AppContext.";
  if (type_ == AppType::CONTAINER || type_ == AppType::SUBSPACE) {
    app_context_ = component::ApplicationContext::CreateFromStartupInfo();
  } else {
    FXL_CHECK(false);
  }

  FXL_LOG(INFO) << AppTypeString(type_) << "Connecting to space service.";
  // Connect the SpaceProviderService.
  if (type_ == AppType::CONTAINER) {
    // Launch the subspace app
    component::Services subspace_services;
    component::ApplicationLaunchInfo launch_info;
    launch_info.url = "hello_subspace";
    launch_info.directory_request = subspace_services.NewRequest();
    app_context_->launcher()->CreateApplication(
        std::move(launch_info), s_subspace_controller.NewRequest());

    subspace_services.ConnectToService(space_provider_iface_.NewRequest(),
                                       "space_provider");

    // component::Services app_services;
    // app_services.Bind(zx::channel(app_directory));
    // app_services.ConnectToService<ui::SpaceProvider>(
    //     space_provider_iface_.NewRequest(), "space_provider");
  } else if (type_ == AppType::SUBSPACE) {
    space_provider_impl_ = std::make_unique<SpaceProviderService>(
        app_context_.get(), [this](SpaceContext context) {
          FXL_LOG(INFO) << "In space factory fn!";
        });
  } else {
    FXL_CHECK(false);
  }

  // Connect to the global Scenic service and begin a session.
  FXL_LOG(INFO) << AppTypeString(type_) << "Connecting to Scenic service.";
  scenic_ = app_context_->ConnectToEnvironmentService<ui::Scenic>();
  scenic_.set_error_handler([this] {
    FXL_LOG(INFO) << AppTypeString(type_)
                  << "Lost connection to Scenic service.";
    fsl::MessageLoop::GetCurrent()->QuitNow();
  });
  // TODO: set up SessionListener.
  FXL_LOG(INFO) << AppTypeString(type_) << "Creating new session.";
  session_ = std::make_unique<scenic_lib::Session>(scenic_.get());
  session_->set_error_handler([this]() {
    FXL_LOG(INFO) << AppTypeString(type_)
                  << "Lost connection to Scenic session.";
    fsl::MessageLoop::GetCurrent()->QuitNow();
  });

  // Close the session after kSessionDuration seconds.
  constexpr int kSessionDuration = 10;
  fsl::MessageLoop::GetCurrent()->task_runner()->PostDelayedTask(
      [this] {
        FXL_LOG(INFO) << AppTypeString(type_) << "Closing session.";
        ReleaseSessionResources();
      },
      fxl::TimeDelta::FromSeconds(kSessionDuration));

  // Start rendering.
  scenic_->GetDisplayInfo([this](gfx::DisplayInfo display_info) {
    if (type_ == AppType::CONTAINER) {
      zx::eventpair space_export_token;
      zx::eventpair space_import_token;
      component::ServiceProviderPtr incoming_services;
      component::ServiceProviderPtr outgoing_services;
      zx_status_t status =
          zx::eventpair::create(0u, &space_export_token, &space_import_token);
      FXL_DCHECK(status == ZX_OK);

      FXL_LOG(INFO) << AppTypeString(type_) << "Creating space eventpair.";

      // application_context_->outgoing_services()->AddBinding(
      //     outgoing_services.NewRequest());
      FXL_LOG(INFO) << AppTypeString(type_) << "Creating space.";
      space_provider_iface_->CreateSpace(std::move(space_export_token),
                                         incoming_services.NewRequest(),
                                         outgoing_services);

      // Set up initial scene.
      FXL_LOG(INFO) << AppTypeString(type_) << "Creating initial scene.";
      const float display_width = static_cast<float>(display_info.width_in_px);
      const float display_height =
          static_cast<float>(display_info.height_in_px);
      CreateScene(display_width, display_height);

      // Begin the update loop.
      const uint64_t cur_time = zx_clock_get(ZX_CLOCK_MONOTONIC);
      Update(cur_time);
    }
  });
}

void App::ReleaseSessionResources() {
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

void App::CreateScene(float display_width, float display_height) {
  auto session_ptr = session_.get();

  // The top-level nesting for drawing anything is compositor -> layer-stack
  // -> layer.  Layer content can come from an image, or by rendering a
  // scene. In this case, we do the latter, so we nest layer -> renderer ->
  // camera -> scene.
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
  static const float background_width = display_width - 2.f * kBackgroundMargin;
  static const float background_height =
      display_height - 2.f * kBackgroundMargin;
  scenic_lib::ShapeNode background_node(session_ptr);
  scenic_lib::RoundedRectangle background_shape(
      session_ptr, background_width, background_height, 20.f, 20.f, 80.f, 10.f);
  scenic_lib::Material background_material(session_ptr);
  background_material.SetColor(120, 255, 120, 255);
  background_node.SetShape(background_shape);
  background_node.SetMaterial(background_material);
  root_node.SetClip(0, true);
  root_node.SetTranslation(kBackgroundMargin + background_width * 0.5f,
                           kBackgroundMargin + background_height * 0.5f, 1.f);
  root_node.AddPart(background_node);

  // static constexpr float kPaneMargin = 100.f;
  // static const float pane_width = (display_width - 3.f * kPaneMargin)
  // / 2.f; static const float pane_height = display_height - 2.f *
  // kPaneMargin;
  //
  // // The root node will enclose two "panes", each with a rounded-rect
  // part
  // // that acts as a background clipper.
  // scenic_lib::RoundedRectangle pane_shape(session_ptr, pane_width,
  // pane_height,
  //                                         20, 20, 80, 10);
  // scenic_lib::Material pane_material(session_ptr);
  // pane_material.SetColor(120, 120, 255, 255);
  //
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
