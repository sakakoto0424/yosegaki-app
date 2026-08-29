use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::CanvasRenderingContext2d;

use crate::api::Message;

async fn load_image(url: String) -> Result<web_sys::HtmlImageElement, JsValue> {
    let img = web_sys::HtmlImageElement::new()?;
    img.set_src(&url);

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let onload = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let onerror = Closure::once_into_js(move |_e: JsValue| {
            let _ = reject.call0(&JsValue::NULL);
        });
        img.set_onload(Some(onload.unchecked_ref()));
        img.set_onerror(Some(onerror.unchecked_ref()));
    });

    JsFuture::from(promise).await?;
    Ok(img)
}

fn wrap_text(ctx: &CanvasRenderingContext2d, text: &str, max_width: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(current.clone());
            current.clear();
            continue;
        }
        let mut trial = current.clone();
        trial.push(ch);
        let width = ctx
            .measure_text(&trial)
            .map(|m| m.width())
            .unwrap_or(0.0);
        if width > max_width && !current.is_empty() {
            lines.push(current.clone());
            current.clear();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// 選んだテーマの全メッセージを1枚のPNG画像にまとめてダウンロードする。
pub async fn download_yosegaki(theme_title: String, mut messages: Vec<Message>) -> Result<(), String> {
    messages.reverse(); // 投稿順(古い→新しい)にする

    let window = web_sys::window().ok_or("windowが取得できません")?;
    let document = window.document().ok_or("documentが取得できません")?;

    let canvas_width: u32 = 640;
    let row_height: u32 = 260;
    let header_height: u32 = 80;
    let padding: u32 = 20;
    let row_count = messages.len().max(1) as u32;
    let canvas_height = header_height + row_count * row_height + padding * 2;

    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "canvas作成に失敗しました".to_string())?
        .dyn_into()
        .map_err(|_| "canvas変換に失敗しました".to_string())?;
    canvas.set_width(canvas_width);
    canvas.set_height(canvas_height);

    let ctx: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|_| "描画コンテキストの取得に失敗しました".to_string())?
        .ok_or("描画コンテキストの取得に失敗しました".to_string())?
        .dyn_into()
        .map_err(|_| "描画コンテキストの変換に失敗しました".to_string())?;

    ctx.set_fill_style_str("#ffffff");
    ctx.fill_rect(0.0, 0.0, canvas_width as f64, canvas_height as f64);

    ctx.set_fill_style_str("#111111");
    ctx.set_font("bold 28px sans-serif");
    let _ = ctx.fill_text(&theme_title, padding as f64, 45.0);

    let mut y = header_height as f64 + padding as f64;
    let cell_w = (canvas_width - padding * 2) as f64;
    let cell_h = (row_height - 20) as f64;

    if messages.is_empty() {
        ctx.set_fill_style_str("#999999");
        ctx.set_font("18px sans-serif");
        let _ = ctx.fill_text("まだ寄せ書きがありません", padding as f64 + 10.0, y + 30.0);
    }

    for m in &messages {
        ctx.set_stroke_style_str("#dddddd");
        ctx.stroke_rect(padding as f64, y, cell_w, cell_h);

        if m.kind == "text" {
            ctx.set_fill_style_str("#222222");
            ctx.set_font("18px sans-serif");
            let text = m.text_content.clone().unwrap_or_default();
            let lines = wrap_text(&ctx, &text, cell_w - 20.0);
            for (i, line) in lines.iter().take(8).enumerate() {
                let _ = ctx.fill_text(line, padding as f64 + 10.0, y + 30.0 + i as f64 * 24.0);
            }
        } else if let Some(key) = &m.image_key {
            match load_image(format!("/img/{key}")).await {
                Ok(img) => {
                    let max_w = cell_w - 20.0;
                    let max_h = cell_h - 20.0;
                    let natural_w = img.natural_width() as f64;
                    let natural_h = img.natural_height() as f64;
                    let scale = if natural_w > 0.0 && natural_h > 0.0 {
                        (max_w / natural_w).min(max_h / natural_h).min(1.0)
                    } else {
                        1.0
                    };
                    let draw_w = natural_w * scale;
                    let draw_h = natural_h * scale;
                    let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(
                        &img,
                        padding as f64 + 10.0,
                        y + 10.0,
                        draw_w,
                        draw_h,
                    );
                }
                Err(_) => {
                    ctx.set_fill_style_str("#999999");
                    ctx.set_font("16px sans-serif");
                    let _ = ctx.fill_text(
                        "(画像を読み込めませんでした)",
                        padding as f64 + 10.0,
                        y + 30.0,
                    );
                }
            }
        }

        y += row_height as f64;
    }

    let data_url = canvas
        .to_data_url()
        .map_err(|_| "画像の書き出しに失敗しました".to_string())?;

    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|_| "ダウンロードリンクの作成に失敗しました".to_string())?
        .dyn_into()
        .map_err(|_| "ダウンロードリンクの作成に失敗しました".to_string())?;
    a.set_href(&data_url);
    a.set_download(&format!("{theme_title}.png"));
    a.click();

    Ok(())
}
