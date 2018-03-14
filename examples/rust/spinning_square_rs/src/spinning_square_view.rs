use fidl::{ServerEnd, ServerImmediate};
use garnet_public_lib_ui_views_fidl::view_provider::ViewProvider;
use garnet_public_lib_app_fidl_service_provider::service_provider::ServiceProvider::Service as ServiceProviderService;
use garnet_public_lib_ui_views_fidl_view_token::view_token::ViewOwner::Service as ViewOwnerService;

struct SpinningSquareViewServer {}

impl ViewProvider::Server for SpinningSquareViewServer {
    type CreateView = ServerImmediate<()>;

    fn create_view(
        &mut self,
        view_owner: ServerEnd<ViewOwnerService>,
        services: Option<ServerEnd<ServiceProviderService>>
    ) -> Self::CreateView {
    }
}
