use fidl::{ServerEnd, ServerImmediate};
use garnet_public_lib_ui_views_fidl::view_provider::ViewProvider;
use garnet_public_lib_ui_views_fidl::garnet_public_lib_ui_views_fidl_view_token::ViewOwner::Service as ViewOwnerService;

struct SpinningSquareViewServer {}

impl ViewProvider::Server for SpinningSquareViewServer {
    type CreateView = fidl::ServerImmediate<()>;

    fn create_view(
        &mut self,
        view_owner: ServerEnd<ViewOwnerService>,
        services: Option<ServerEnd<ViewOwnerService>>
    ) -> Self::CreateView {
    }
}
