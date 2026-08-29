use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[server(SayHello)]
pub async fn say_hello(num: i32) -> Result<String, ServerFnError> {
    Ok(format!("Hello from the API!!! I got {num}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub id: i64,
    pub title: String,
    pub created_at: String,
}

#[cfg(feature = "ssr")]
async fn d1() -> Result<worker::D1Database, ServerFnError> {
    use axum::extract::Extension;
    use std::sync::Arc;
    use worker::Env;

    let Extension(env): Extension<Arc<Env>> = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    env.d1("yosegaki_db")
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg_attr(feature = "ssr", worker::send)]
#[server(ListThemes)]
pub async fn list_themes() -> Result<Vec<Theme>, ServerFnError> {
    let db = d1().await?;

    let stmt = db.prepare("SELECT id, title, created_at FROM themes ORDER BY created_at DESC");
    let result = stmt
        .all()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    result
        .results::<Theme>()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg_attr(feature = "ssr", worker::send)]
#[server(CreateTheme)]
pub async fn create_theme(title: String) -> Result<(), ServerFnError> {
    let db = d1().await?;

    let stmt = db
        .prepare("INSERT INTO themes (title) VALUES (?1)")
        .bind(&[title.into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    stmt.run()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}
