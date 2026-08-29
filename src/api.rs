use leptos::prelude::*;
use serde::{Deserialize, Serialize};

// --- 無料枠を守るための上限(リミッター) ---
const MAX_TEXT_LEN: usize = 500;
const MAX_IMAGE_BYTES: usize = 500 * 1024; // 500KB
const MAX_MESSAGES_PER_THEME: i64 = 500;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub theme_id: i64,
    pub kind: String,
    pub text_content: Option<String>,
    pub image_key: Option<String>,
    pub created_at: String,
}

#[cfg(feature = "ssr")]
async fn env() -> Result<std::sync::Arc<worker::Env>, ServerFnError> {
    use axum::extract::Extension;

    let Extension(env): Extension<std::sync::Arc<worker::Env>> = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(env)
}

#[cfg(feature = "ssr")]
async fn d1() -> Result<worker::D1Database, ServerFnError> {
    env()
        .await?
        .d1("yosegaki_db")
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg(feature = "ssr")]
async fn bucket() -> Result<worker::Bucket, ServerFnError> {
    env()
        .await?
        .bucket("yosegaki_images")
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg(feature = "ssr")]
async fn message_count(db: &worker::D1Database, theme_id: i64) -> Result<i64, ServerFnError> {
    #[derive(Deserialize)]
    struct Count {
        cnt: i64,
    }

    let stmt = db
        .prepare("SELECT COUNT(*) as cnt FROM messages WHERE theme_id = ?1")
        .bind(&[(theme_id as f64).into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let result = stmt
        .all()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let counts: Vec<Count> = result
        .results()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(counts.first().map(|c| c.cnt).unwrap_or(0))
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

#[cfg_attr(feature = "ssr", worker::send)]
#[server(ListMessages)]
pub async fn list_messages(theme_id: i64) -> Result<Vec<Message>, ServerFnError> {
    let db = d1().await?;

    let stmt = db
        .prepare(
            "SELECT id, theme_id, kind, text_content, image_key, created_at \
             FROM messages WHERE theme_id = ?1 ORDER BY created_at DESC",
        )
        .bind(&[(theme_id as f64).into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let result = stmt
        .all()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    result
        .results::<Message>()
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[cfg_attr(feature = "ssr", worker::send)]
#[server(PostTextMessage)]
pub async fn post_text_message(theme_id: i64, text: String) -> Result<(), ServerFnError> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(ServerFnError::new("メッセージが空です"));
    }
    if text.chars().count() > MAX_TEXT_LEN {
        return Err(ServerFnError::new(format!(
            "メッセージは{MAX_TEXT_LEN}文字までです"
        )));
    }

    let db = d1().await?;

    if message_count(&db, theme_id).await? >= MAX_MESSAGES_PER_THEME {
        return Err(ServerFnError::new(
            "このテーマは投稿数の上限に達しました。新しいテーマを作成してください",
        ));
    }

    let stmt = db
        .prepare("INSERT INTO messages (theme_id, kind, text_content) VALUES (?1, 'text', ?2)")
        .bind(&[(theme_id as f64).into(), text.into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    stmt.run()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[cfg_attr(feature = "ssr", worker::send)]
#[server(PostDrawingMessage)]
pub async fn post_drawing_message(
    theme_id: i64,
    image_base64: String,
) -> Result<(), ServerFnError> {
    use base64::Engine;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_base64)
        .map_err(|e| ServerFnError::new(format!("画像の読み込みに失敗しました: {e}")))?;

    if bytes.is_empty() {
        return Err(ServerFnError::new("画像が空です"));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ServerFnError::new(format!(
            "画像サイズが大きすぎます({}KBまで)",
            MAX_IMAGE_BYTES / 1024
        )));
    }

    let db = d1().await?;

    if message_count(&db, theme_id).await? >= MAX_MESSAGES_PER_THEME {
        return Err(ServerFnError::new(
            "このテーマは投稿数の上限に達しました。新しいテーマを作成してください",
        ));
    }

    let key = format!("{}.png", uuid::Uuid::new_v4());

    let b = bucket().await?;
    b.put(key.clone(), bytes)
        .execute()
        .await
        .map_err(|e| ServerFnError::new(format!("画像の保存に失敗しました: {e}")))?;

    let stmt = db
        .prepare("INSERT INTO messages (theme_id, kind, image_key) VALUES (?1, 'drawing', ?2)")
        .bind(&[(theme_id as f64).into(), key.into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    stmt.run()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}
