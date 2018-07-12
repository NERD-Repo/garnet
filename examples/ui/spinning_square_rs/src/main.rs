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
use fidl_fuchsia_ui_views_v1::{ViewManagerMarker, ViewManagerProxy, ViewMarker,
                               ViewProviderMarker, ViewProviderRequestStream};
use fidl_fuchsia_ui_views_v1_token::ViewOwnerMarker;
use futures::{FutureExt, StreamExt};
use std::sync::{Arc, Mutex};

struct BaseView {}

struct App {
    view_manager: ViewManagerProxy,
    views: Vec<BaseView>,
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

    pub fn create_view(&self, req: ServerEnd<ViewOwnerMarker>) {
        let (view, view_server_end) = create_endpoints::<ViewMarker>().unwrap();
        let (listener_client, listener_server) = zx::Channel::create().unwrap();
        let (mine, theirs) = zx::EventPair::create().unwrap();
        self.view_manager
            .create_view(view_server_end, req, listener_client.into(), theirs, None)
            .unwrap();
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
