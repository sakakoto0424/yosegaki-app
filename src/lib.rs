mod app;
mod api;
mod components;

#[cfg(feature = "ssr")]
#[worker::send]
async fn serve_image(
    axum::extract::Extension(env): axum::extract::Extension<std::sync::Arc<worker::Env>>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let result: worker::Result<Option<Vec<u8>>> = async {
        let bucket = env.bucket("yosegaki_images")?;
        let object = bucket.get(&key).execute().await?;
        match object {
            Some(obj) => match obj.body() {
                Some(body) => {
                    let bytes = body.bytes().await?;
                    Ok(Some(bytes))
                }
                None => Ok(None),
            },
            None => Ok(None),
        }
    }
    .await;

    match result {
        Ok(Some(bytes)) => (StatusCode::OK, [("content-type", "image/png")], bytes).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response(),
    }
}

#[cfg(feature = "ssr")]
#[worker::event(fetch)]
async fn fetch(
    req: worker::HttpRequest,
    env: worker::Env,
    _ctx: worker::Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    use std::sync::Arc;

    use axum::{routing::get, Extension, Router};
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tower_service::Service;

    use app::{App, shell};

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    // build our application with a route
    let mut router = Router::new()
        .route("/img/{key}", get(serve_image))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .with_state(leptos_options)
        .layer(Extension(Arc::new(env))); // <- Allow leptos server functions to access Worker stuff

    Ok(router.call(req).await?)
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}
