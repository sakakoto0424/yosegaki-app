#![recursion_limit = "256"]

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
#[worker::send]
async fn basic_auth(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;
    use base64::Engine;

    let unauthorized = || {
        (
            StatusCode::UNAUTHORIZED,
            [("WWW-Authenticate", "Basic realm=\"yosegaki\"")],
            "認証が必要です",
        )
            .into_response()
    };

    let Some(env) = req.extensions().get::<std::sync::Arc<worker::Env>>().cloned() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "auth config error").into_response();
    };
    let Ok(expected_user) = env.secret("BASIC_AUTH_USER") else {
        return unauthorized();
    };
    let Ok(expected_pass) = env.secret("BASIC_AUTH_PASS") else {
        return unauthorized();
    };

    let provided = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|encoded| base64::engine::general_purpose::STANDARD.decode(encoded).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());

    let Some(provided) = provided else {
        return unauthorized();
    };
    let Some((user, pass)) = provided.split_once(':') else {
        return unauthorized();
    };

    if user == expected_user.to_string() && pass == expected_pass.to_string() {
        next.run(req).await
    } else {
        unauthorized()
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
        .layer(axum::middleware::from_fn(basic_auth))
        .layer(Extension(Arc::new(env))); // <- Allow leptos server functions to access Worker stuff (must be outermost so basic_auth can read it)

    Ok(router.call(req).await?)
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(app::App);
}
