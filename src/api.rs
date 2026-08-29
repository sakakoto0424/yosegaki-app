use leptos::prelude::*;
use serde::{Deserialize, Serialize};

// --- 無料枠を守るための上限(リミッター) ---
const MAX_IMAGE_BYTES: usize = 3 * 1024 * 1024; // 3MB(共有キャンバスは書き足すほど大きくなるため)
const MAX_CONTRIBUTIONS_PER_THEME: i64 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub canvas_key: Option<String>,
    pub contribution_count: i64,
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

#[cfg_attr(feature = "ssr", worker::send)]
#[server(ListThemes)]
pub async fn list_themes() -> Result<Vec<Theme>, ServerFnError> {
    let db = d1().await?;

    let stmt = db.prepare(
        "SELECT id, title, created_at, canvas_key, contribution_count \
         FROM themes ORDER BY created_at DESC",
    );
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
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(ServerFnError::new("テーマ名が空です"));
    }

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

/// テーマに書き足す(=共有キャンバスの新しいバージョンを保存する)。
/// `image_base64` は「今までの絵 + 自分が書き足した分」を1枚に焼き込んだPNG。
#[cfg_attr(feature = "ssr", worker::send)]
#[server(SubmitContribution)]
pub async fn submit_contribution(
    theme_id: i64,
    image_base64: String,
) -> Result<String, ServerFnError> {
    use base64::Engine;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image_base64)
        .map_err(|e| ServerFnError::new(format!("画像の読み込みに失敗しました: {e}")))?;

    if bytes.is_empty() {
        return Err(ServerFnError::new("画像が空です"));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ServerFnError::new(format!(
            "画像サイズが大きすぎます({}MBまで)",
            MAX_IMAGE_BYTES / 1024 / 1024
        )));
    }

    let db = d1().await?;

    #[derive(Deserialize)]
    struct Count {
        contribution_count: i64,
    }
    let stmt = db
        .prepare("SELECT contribution_count FROM themes WHERE id = ?1")
        .bind(&[(theme_id as f64).into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let result = stmt
        .all()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let counts: Vec<Count> = result
        .results()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let current_count = counts.first().map(|c| c.contribution_count).unwrap_or(0);

    if current_count >= MAX_CONTRIBUTIONS_PER_THEME {
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
        .prepare(
            "UPDATE themes SET canvas_key = ?1, contribution_count = contribution_count + 1 \
             WHERE id = ?2",
        )
        .bind(&[key.clone().into(), (theme_id as f64).into()])
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    stmt.run()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(key)
}
