// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate failure;
extern crate fidl;
extern crate fidl_fuchsia_ui_views_v1;
extern crate fidl_fuchsia_ui_views_v1_token;
extern crate fuchsia_app as component;
extern crate fuchsia_async as async;
extern crate fuchsia_zircon as zx;
extern crate futures;

use component::client::connect_to_service;
use component::server::ServiceFactory;
use failure::{Error, ResultExt};
use fidl::endpoints2::{create_endpoints, RequestStream, ServerEnd, ServiceMarker};
use fidl_fuchsia_ui_views_v1::ViewProviderRequest::CreateView;
use fidl_fuchsia_ui_views_v1::{ViewListenerRequest, ViewListenerRequestStream, ViewManagerMarker,
                               ViewManagerProxy, ViewMarker, ViewProviderMarker,
                               ViewProviderRequestStream};
use fidl_fuchsia_ui_views_v1_token::ViewOwnerMarker;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, Mutex};

struct BaseView {
    view: fidl_fuchsia_ui_views_v1::ViewProxy,
    mine: zx::EventPair,
}

type BaseViewPtr = Arc<Mutex<BaseView>>;

impl BaseView {
    pub fn new(
        listener_server: zx::Channel, view: fidl_fuchsia_ui_views_v1::ViewProxy,
        mine: zx::EventPair,
    ) -> BaseViewPtr {
        let view_ptr = Arc::new(Mutex::new(BaseView {
            view,
            mine,
        }));
        Self::setup(view_ptr.clone(), async::Channel::from_channel(listener_server.into()).unwrap());
        view_ptr
    }

    fn setup(view_ptr: BaseViewPtr, listener_server: async::Channel) {
        async::spawn(
            ViewListenerRequestStream::from_channel(listener_server)
                .for_each(move |req| {
                    let ViewListenerRequest::OnPropertiesChanged { properties, .. } = req;
                    view_ptr.lock().unwrap().handle_properies_changed(&properties);
                    futures::future::ok(())
                })
                .map(|_| ())
                .recover(|e| eprintln!("view listener error: {:?}", e)),
        )
    }

    fn handle_properies_changed(&mut self, properties: &fidl_fuchsia_ui_views_v1::ViewProperties) {
        println!("OnPropertiesChanged = {:#?}", properties);
    }
}

struct App {
    view_manager: ViewManagerProxy,
    views: Vec<BaseViewPtr>,
}

type AppPtr = Arc<Mutex<App>>;

impl App {
    pub fn new() -> AppPtr {
        let view_manager = connect_to_service::<ViewManagerMarker>().unwrap();
        Arc::new(Mutex::new(App {
            view_manager,
            views: vec![],
        }))
    }

    pub fn spawn_view_provider_server(app: &AppPtr, chan: async::Channel) {
        let app = app.clone();
        async::spawn(
            ViewProviderRequestStream::from_channel(chan)
                .for_each(move |req| {
                    let CreateView { view_owner, .. } = req;
                    println!("create_request = {:#?}", view_owner);
                    App::app_create_view(app.clone(), view_owner);
                    futures::future::ok(())
                })
                .map(|_| ())
                .recover(|e| eprintln!("error running view_provider server: {:?}", e)),
        )
    }

    pub fn app_create_view(app: AppPtr, req: ServerEnd<ViewOwnerMarker>) {
        app.lock().unwrap().create_view(req);
    }

    pub fn create_view(&mut self, req: ServerEnd<ViewOwnerMarker>) {
        let (view, view_server_end) = create_endpoints::<ViewMarker>().unwrap();
        let (listener_client, listener_server) = zx::Channel::create().unwrap();
        let (mine, theirs) = zx::EventPair::create().unwrap();
        self.view_manager
            .create_view(view_server_end, req, listener_client.into(), theirs, None)
            .unwrap();
        let view_ptr = BaseView::new(listener_server, view, mine);
        self.views.push(view_ptr);
    }
}

struct ViewProvider {
    app: AppPtr,
}

impl ServiceFactory for ViewProvider {
    fn service_name(&self) -> &str {
        ViewProviderMarker::NAME
    }

    fn spawn_service(&mut self, channel: async::Channel) {
        App::spawn_view_provider_server(&self.app, channel);
    }
}

fn main() -> Result<(), Error> {
    println!("spinning square rs");
    let mut executor = async::Executor::new().context("Error creating executor")?;

    let app = App::new();

    let view_provider = ViewProvider { app: app.clone() };

    let fut = component::server::ServicesServer::new()
        .add_service(view_provider)
        .start()
        .context("Error starting view provider server")?;

    executor.run_singlethreaded(fut)?;

    Ok(())
}
