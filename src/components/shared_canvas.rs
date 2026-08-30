use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, PointerEvent};

use crate::api::{create_theme, list_themes, submit_contribution, Theme};

/// 共有キャンバスの実サイズ(=保存される画像のサイズ)。書ける面積を広くとる。
pub const CANVAS_WIDTH: u32 = 1800;
pub const CANVAS_HEIGHT: u32 = 1350;
/// 画面に一度に見える範囲(スクロールして移動する)
const VIEWPORT_WIDTH: u32 = 640;
const VIEWPORT_HEIGHT: u32 = 480;
const PAN_STEP: i32 = 240;
/// 表示枠を回転できる角度(実物の色紙を回すように、90度刻みで切り替える)
const ROTATIONS: [i32; 4] = [0, 90, 180, 270];

/// 表示枠が`rotation_deg`だけ回転している時、表示枠内の相対位置(rel_x, rel_y)を
/// 回転前(0度)の表示枠ローカル座標に変換する。
fn unrotate_point(rel_x: f64, rel_y: f64, rotation_deg: i32) -> (f64, f64) {
    let w = VIEWPORT_WIDTH as f64;
    let h = VIEWPORT_HEIGHT as f64;
    match rotation_deg.rem_euclid(360) {
        90 => (rel_y, h - rel_x),
        180 => (w - rel_x, h - rel_y),
        270 => (w - rel_y, rel_x),
        _ => (rel_x, rel_y),
    }
}

/// 表示枠が`rotation_deg`だけ回転している時、見た目上の移動方向(dx, dy)を
/// 回転前(0度)のスクロール方向に変換する。
fn unrotate_vector(dx: i32, dy: i32, rotation_deg: i32) -> (i32, i32) {
    match rotation_deg.rem_euclid(360) {
        90 => (dy, -dx),
        180 => (-dx, -dy),
        270 => (-dy, dx),
        _ => (dx, dy),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pen,
    Text,
    Photo,
}

/// ペン・文字で選べる色のパレット(名前, カラーコード)
const COLORS: [(&str, &str); 9] = [
    ("黒", "#222222"),
    ("赤", "#e6432a"),
    ("オレンジ", "#f2994a"),
    ("黄", "#f2c94c"),
    ("緑", "#27ae60"),
    ("青", "#2f80ed"),
    ("紫", "#9b51e0"),
    ("ピンク", "#eb5b9d"),
    ("茶", "#8a5a34"),
];

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

fn get_2d_context(canvas: &web_sys::HtmlCanvasElement) -> Option<CanvasRenderingContext2d> {
    canvas.get_context("2d").ok()??.dyn_into().ok()
}

fn new_canvas(
    document: &web_sys::Document,
    width: u32,
    height: u32,
) -> Result<(web_sys::HtmlCanvasElement, CanvasRenderingContext2d), String> {
    let canvas: web_sys::HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| "canvas作成に失敗しました".to_string())?
        .dyn_into()
        .map_err(|_| "canvas変換に失敗しました".to_string())?;
    canvas.set_width(width);
    canvas.set_height(height);
    let ctx = get_2d_context(&canvas).ok_or("描画コンテキストの取得に失敗しました".to_string())?;
    Ok((canvas, ctx))
}

/// 実際に何か描かれている範囲(余白を除いた矩形)を検出する。
fn find_ink_bbox(ctx: &CanvasRenderingContext2d, width: u32, height: u32) -> Option<(u32, u32, u32, u32)> {
    let data = ctx
        .get_image_data(0.0, 0.0, width as f64, height as f64)
        .ok()?
        .data()
        .0;

    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for y in 0..height {
        let row = (y * width) as usize;
        for x in 0..width {
            let idx = (row + x as usize) * 4;
            let a = data[idx + 3];
            let r = data[idx];
            let g = data[idx + 1];
            let b = data[idx + 2];
            let is_blank = a == 0 || (r > 250 && g > 250 && b > 250);
            if !is_blank {
                found = true;
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    found.then_some((min_x, min_y, max_x, max_y))
}

/// 現在の共有キャンバス画像を読み込み、書き込みがある範囲だけを切り出してダウンロードさせる。
async fn download_cropped(canvas_key: String, theme_title: String) -> Result<(), String> {
    let window = web_sys::window().ok_or("windowが取得できません")?;
    let document = window.document().ok_or("documentが取得できません")?;

    let img = load_image(format!("/img/{canvas_key}"))
        .await
        .map_err(|_| "画像の読み込みに失敗しました".to_string())?;

    let (_full_canvas, full_ctx) = new_canvas(&document, CANVAS_WIDTH, CANVAS_HEIGHT)?;
    full_ctx.set_fill_style_str("#ffffff");
    full_ctx.fill_rect(0.0, 0.0, CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64);
    let _ = full_ctx.draw_image_with_html_image_element(&img, 0.0, 0.0);

    let padding: i64 = 24;
    let (min_x, min_y, max_x, max_y) =
        find_ink_bbox(&full_ctx, CANVAS_WIDTH, CANVAS_HEIGHT).unwrap_or((0, 0, 200, 200));

    let crop_x = (min_x as i64 - padding).max(0) as u32;
    let crop_y = (min_y as i64 - padding).max(0) as u32;
    let crop_w = (((max_x as i64 - crop_x as i64) + padding + 1).min(CANVAS_WIDTH as i64 - crop_x as i64)).max(1) as u32;
    let crop_h = (((max_y as i64 - crop_y as i64) + padding + 1).min(CANVAS_HEIGHT as i64 - crop_y as i64)).max(1) as u32;

    let (out_canvas, out_ctx) = new_canvas(&document, crop_w, crop_h)?;
    out_ctx.set_fill_style_str("#ffffff");
    out_ctx.fill_rect(0.0, 0.0, crop_w as f64, crop_h as f64);
    out_ctx
        .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &img,
            crop_x as f64,
            crop_y as f64,
            crop_w as f64,
            crop_h as f64,
            0.0,
            0.0,
            crop_w as f64,
            crop_h as f64,
        )
        .map_err(|_| "画像の切り出しに失敗しました".to_string())?;

    let data_url = out_canvas
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

/// 1つのテーマ = 1枚の共有キャンバス(実寸1800x1350、画面には一部だけ表示してスクロールする)。
/// みんなが「今までの状態」の上に直接書き足していく、本物の寄せ書きに近い体験。
#[component]
pub fn SharedCanvas() -> impl IntoView {
    let (themes, set_themes) = signal(Vec::<Theme>::new());
    let (selected_theme, set_selected_theme) = signal::<Option<i64>>(None);
    let (new_title, set_new_title) = signal(String::new());
    let (tool, set_tool) = signal(Tool::Pen);
    let (status, set_status) = signal(String::new());
    let (pending_text_pos, set_pending_text_pos) = signal::<Option<(f64, f64)>>(None);
    let (text_draft, set_text_draft) = signal(String::new());
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let scroll_ref = NodeRef::<leptos::html::Div>::new();
    let is_drawing = StoredValue::new(false);
    let (rotation, set_rotation) = signal(0i32);
    let (selected_color, set_selected_color) = signal(COLORS[0].1.to_string());

    // 配置調整中の写真(画像本体とObjectURLはStoredValueで保持、位置・拡大率は反応的に扱う)
    let pending_photo = StoredValue::<Option<(web_sys::HtmlImageElement, String)>>::new(None);
    let (pending_photo_pos, set_pending_photo_pos) = signal::<Option<(f64, f64)>>(None);
    let (pending_photo_size, set_pending_photo_size) = signal::<(f64, f64)>((0.0, 0.0));
    let (pending_photo_scale, set_pending_photo_scale) = signal(1.0f64);
    let is_dragging_photo = StoredValue::new(false);

    let refresh_themes = move || {
        spawn_local(async move {
            match list_themes().await {
                Ok(list) => {
                    if selected_theme.get_untracked().is_none() {
                        if let Some(first) = list.first() {
                            set_selected_theme.set(Some(first.id));
                        }
                    }
                    set_themes.set(list);
                }
                Err(e) => set_status.set(format!("読み込みに失敗しました: {e}")),
            }
        });
    };

    Effect::new(move |_| {
        refresh_themes();
    });

    // 選択中テーマの「最後に保存された状態」をキャンバスに読み込み直す(未保存の書き込みは破棄される)
    let reload_canvas = move || {
        let theme_id = selected_theme.get_untracked();
        let canvas_key = theme_id.and_then(|id| {
            themes
                .get_untracked()
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.canvas_key.clone())
        });
        let Some(canvas) = canvas_ref.get_untracked() else {
            return;
        };
        spawn_local(async move {
            let Some(ctx) = get_2d_context(&canvas) else {
                return;
            };
            ctx.set_fill_style_str("#ffffff");
            ctx.fill_rect(0.0, 0.0, CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64);
            if let Some(key) = canvas_key {
                if let Ok(img) = load_image(format!("/img/{key}")).await {
                    let _ = ctx.draw_image_with_html_image_element(&img, 0.0, 0.0);
                }
            }
        });
    };

    // テーマ切り替え・テーマ一覧更新のたびに読み込み直す
    Effect::new(move |_| {
        let _ = selected_theme.get();
        let _ = themes.get();
        reload_canvas();
    });

    let on_create_theme = move |_| {
        let title = new_title.get_untracked();
        if title.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            match create_theme(title).await {
                Ok(()) => {
                    set_new_title.set(String::new());
                    set_status.set(String::new());
                    refresh_themes();
                }
                Err(e) => set_status.set(format!("作成に失敗しました: {e}")),
            }
        });
    };

    let get_ctx = move || -> Option<CanvasRenderingContext2d> { get_2d_context(&canvas_ref.get()?) };

    // 表示枠の回転・スクロールを考慮して、ポインター位置をキャンバス上の実座標に変換する
    let pointer_to_canvas = move |ev: &PointerEvent| -> Option<(f64, f64)> {
        let el = scroll_ref.get_untracked()?;
        let rect = el.get_bounding_client_rect();
        let rel_x = ev.client_x() as f64 - rect.left();
        let rel_y = ev.client_y() as f64 - rect.top();
        let (ux, uy) = unrotate_point(rel_x, rel_y, rotation.get_untracked());
        Some((ux + el.scroll_left() as f64, uy + el.scroll_top() as f64))
    };

    let on_pointer_down = move |ev: PointerEvent| {
        let Some((x, y)) = pointer_to_canvas(&ev) else {
            return;
        };
        match tool.get_untracked() {
            Tool::Text => {
                set_pending_text_pos.set(Some((x, y)));
            }
            Tool::Photo => {
                if pending_photo.with_value(|p| p.is_some()) {
                    set_pending_photo_pos.set(Some((x, y)));
                    is_dragging_photo.set_value(true);
                }
            }
            Tool::Pen => {
                ev.prevent_default();
                if let Some(canvas) = canvas_ref.get() {
                    let _ = canvas.set_pointer_capture(ev.pointer_id());
                }
                if let Some(ctx) = get_ctx() {
                    ctx.begin_path();
                    ctx.move_to(x, y);
                }
                is_drawing.set_value(true);
            }
        }
    };

    let on_pointer_move = move |ev: PointerEvent| {
        match tool.get_untracked() {
            Tool::Pen if is_drawing.get_value() => {
                ev.prevent_default();
                let Some((x, y)) = pointer_to_canvas(&ev) else {
                    return;
                };
                if let Some(ctx) = get_ctx() {
                    ctx.line_to(x, y);
                    ctx.set_line_width(4.0);
                    ctx.set_line_cap("round");
                    ctx.set_line_join("round");
                    ctx.set_stroke_style_str(&selected_color.get_untracked());
                    ctx.stroke();
                }
            }
            Tool::Photo if is_dragging_photo.get_value() => {
                if let Some((x, y)) = pointer_to_canvas(&ev) {
                    set_pending_photo_pos.set(Some((x, y)));
                }
            }
            _ => {}
        }
    };

    let on_pointer_up = move |_ev: PointerEvent| {
        is_drawing.set_value(false);
        is_dragging_photo.set_value(false);
    };

    let on_pointer_leave = move |_ev: PointerEvent| {
        is_drawing.set_value(false);
        is_dragging_photo.set_value(false);
    };

    let on_place_text = move |_| {
        let Some((x, y)) = pending_text_pos.get_untracked() else {
            return;
        };
        let text = text_draft.get_untracked();
        if !text.trim().is_empty() {
            if let Some(ctx) = get_ctx() {
                ctx.set_fill_style_str(&selected_color.get_untracked());
                ctx.set_font("20px sans-serif");
                let _ = ctx.fill_text(&text, x, y);
            }
        }
        set_pending_text_pos.set(None);
        set_text_draft.set(String::new());
    };

    let on_cancel_text = move |_| {
        set_pending_text_pos.set(None);
        set_text_draft.set(String::new());
    };

    let clear_pending_photo = move || {
        pending_photo.update_value(|p| {
            if let Some((_, url)) = p.take() {
                let _ = web_sys::Url::revoke_object_url(&url);
            }
        });
        set_pending_photo_pos.set(None);
    };

    let on_photo_file_change = move |ev: leptos::ev::Event| {
        let Some(target) = ev.target() else {
            return;
        };
        let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() else {
            return;
        };
        let Some(files) = input.files() else {
            return;
        };
        let Some(file) = files.get(0) else {
            return;
        };
        let Ok(url) = web_sys::Url::create_object_url_with_blob(&file) else {
            set_status.set("写真の読み込みに失敗しました".to_string());
            return;
        };
        spawn_local(async move {
            match load_image(url.clone()).await {
                Ok(img) => {
                    let nw = img.natural_width() as f64;
                    let nh = img.natural_height() as f64;
                    let max_dim = 320.0f64;
                    let scale = if nw > 0.0 && nh > 0.0 {
                        (max_dim / nw.max(nh)).min(1.0)
                    } else {
                        1.0
                    };
                    set_pending_photo_size.set((nw, nh));
                    set_pending_photo_scale.set(scale);
                    let (cx, cy) = scroll_ref
                        .get_untracked()
                        .map(|el| {
                            (
                                el.scroll_left() as f64 + VIEWPORT_WIDTH as f64 / 2.0,
                                el.scroll_top() as f64 + VIEWPORT_HEIGHT as f64 / 2.0,
                            )
                        })
                        .unwrap_or((CANVAS_WIDTH as f64 / 2.0, CANVAS_HEIGHT as f64 / 2.0));
                    pending_photo.set_value(Some((img, url)));
                    set_pending_photo_pos.set(Some((cx, cy)));
                    set_status.set(String::new());
                }
                Err(_) => set_status.set("写真の読み込みに失敗しました".to_string()),
            }
        });
    };

    let on_place_photo = move |_| {
        let Some((x, y)) = pending_photo_pos.get_untracked() else {
            return;
        };
        let scale = pending_photo_scale.get_untracked();
        let (nw, nh) = pending_photo_size.get_untracked();
        let (w, h) = (nw * scale, nh * scale);
        pending_photo.with_value(|p| {
            if let Some((img, _)) = p {
                if let Some(ctx) = get_ctx() {
                    let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(
                        img,
                        x - w / 2.0,
                        y - h / 2.0,
                        w,
                        h,
                    );
                }
            }
        });
        clear_pending_photo();
    };

    let on_cancel_photo = move |_| {
        clear_pending_photo();
    };

    let on_undo = move |_| {
        set_pending_text_pos.set(None);
        set_text_draft.set(String::new());
        clear_pending_photo();
        reload_canvas();
        set_status.set("書き途中の内容を取り消しました".to_string());
    };

    let on_save = move |_| {
        let Some(theme_id) = selected_theme.get_untracked() else {
            set_status.set("テーマを選んでください".to_string());
            return;
        };
        let Some(canvas) = canvas_ref.get_untracked() else {
            return;
        };
        let data_url = match canvas.to_data_url() {
            Ok(s) => s,
            Err(_) => {
                set_status.set("画像の書き出しに失敗しました".to_string());
                return;
            }
        };
        let base64 = data_url
            .split_once(',')
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();

        spawn_local(async move {
            match submit_contribution(theme_id, base64).await {
                Ok(_key) => {
                    set_status.set("書き加えました".to_string());
                    refresh_themes();
                }
                Err(e) => set_status.set(format!("{e}")),
            }
        });
    };

    let pan = move |dx: i32, dy: i32| {
        let (dx, dy) = unrotate_vector(dx, dy, rotation.get_untracked());
        if let Some(el) = scroll_ref.get_untracked() {
            let left = el.scroll_left();
            let top = el.scroll_top();
            el.set_scroll_left(left + dx);
            el.set_scroll_top(top + dy);
        }
    };

    let on_rotate = move |_| {
        let current = rotation.get_untracked();
        let idx = ROTATIONS.iter().position(|&r| r == current).unwrap_or(0);
        let next = ROTATIONS[(idx + 1) % ROTATIONS.len()];
        set_rotation.set(next);
    };

    let on_download = move |_| {
        let Some(theme_id) = selected_theme.get_untracked() else {
            return;
        };
        let theme = themes.get_untracked().into_iter().find(|t| t.id == theme_id);
        let Some(theme) = theme else {
            return;
        };
        let Some(key) = theme.canvas_key else {
            set_status.set("まだ何も書かれていません".to_string());
            return;
        };
        spawn_local(async move {
            if let Err(e) = download_cropped(key, theme.title).await {
                set_status.set(e);
            }
        });
    };

    let has_canvas = move || {
        selected_theme
            .get()
            .and_then(|id| themes.get().iter().find(|t| t.id == id).map(|t| t.canvas_key.is_some()))
            .unwrap_or(false)
    };

    view! {
        <div class="shared-canvas">
            <h2>"寄せ書きテーマ"</h2>
            <label>
                "テーマを選ぶ: "
                <select on:change=move |ev| {
                    let v = event_target_value(&ev);
                    set_selected_theme.set(v.parse::<i64>().ok());
                }>
                    <For
                        each=move || themes.get()
                        key=|t| t.id
                        children=move |t: Theme| {
                            view! { <option value=t.id.to_string()>{t.title.clone()}" (" {t.contribution_count} "件)"</option> }
                        }
                    />
                </select>
            </label>

            <div>
                <input
                    type="text"
                    placeholder="例: 2026年度体育祭2年5組"
                    prop:value=move || new_title.get()
                    on:input=move |ev| set_new_title.set(event_target_value(&ev))
                />
                <button type="button" on:click=on_create_theme>"新しいテーマを作る"</button>
            </div>

            <h3>"みんなの寄せ書き"</h3>
            <div>
                <button
                    type="button"
                    disabled=move || tool.get() == Tool::Pen
                    on:click=move |_| set_tool.set(Tool::Pen)
                >"ペンで書く"</button>
                <button
                    type="button"
                    disabled=move || tool.get() == Tool::Text
                    on:click=move |_| set_tool.set(Tool::Text)
                >"文字を置く"</button>
                <button
                    type="button"
                    disabled=move || tool.get() == Tool::Photo
                    on:click=move |_| set_tool.set(Tool::Photo)
                >"写真を貼る"</button>
            </div>

            <Show when=move || tool.get() != Tool::Photo>
                <div class="color-palette">
                    <For
                        each=|| COLORS
                        key=|(_, code)| code.to_string()
                        children=move |(name, code): (&str, &str)| {
                            let code_owned = code.to_string();
                            let code_for_click = code_owned.clone();
                            view! {
                                <button
                                    type="button"
                                    class="color-swatch"
                                    class:selected=move || selected_color.get() == code_owned
                                    style=format!("background-color: {code};")
                                    title=name
                                    on:click=move |_| set_selected_color.set(code_for_click.clone())
                                ></button>
                            }
                        }
                    />
                </div>
            </Show>

            <Show when=move || tool.get() == Tool::Photo>
                <div>
                    <Show when=move || pending_photo_pos.get().is_none()>
                        <input type="file" accept="image/*" on:change=on_photo_file_change />
                    </Show>
                    <Show when=move || pending_photo_pos.get().is_some()>
                        <label class="scale-label">
                            "大きさ: "
                            <input
                                type="range"
                                min="0.05"
                                max="3"
                                step="0.05"
                                prop:value=move || pending_photo_scale.get().to_string()
                                on:input=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_pending_photo_scale.set(v);
                                    }
                                }
                            />
                        </label>
                        <button type="button" on:click=on_place_photo>"配置"</button>
                        <button type="button" on:click=on_cancel_photo>"やめる"</button>
                    </Show>
                </div>
            </Show>

            <p class="canvas-hint">"広いキャンバスの一部だけが表示されています。矢印ボタンで移動、回転ボタンで向きを変えられます。"</p>

            <div class="canvas-nav">
                <button type="button" class="nav-btn" on:click=move |_| pan(0, -PAN_STEP)>"↑"</button>
                <div class="canvas-nav-row">
                    <button type="button" class="nav-btn" on:click=move |_| pan(-PAN_STEP, 0)>"←"</button>
                    <button type="button" class="nav-btn" on:click=move |_| pan(PAN_STEP, 0)>"→"</button>
                </div>
                <button type="button" class="nav-btn" on:click=move |_| pan(0, PAN_STEP)>"↓"</button>
                <button type="button" class="nav-btn rotate-btn" on:click=on_rotate title="画面を回転">"⟳"</button>
            </div>

            <div class="canvas-rotate-wrapper">
                <div class="canvas-viewport" node_ref=scroll_ref
                    style=move || format!(
                        "width: {VIEWPORT_WIDTH}px; height: {VIEWPORT_HEIGHT}px; transform: rotate({}deg);",
                        rotation.get()
                    )
                >
                    <div style="position: relative;">
                        <canvas
                            node_ref=canvas_ref
                            width=CANVAS_WIDTH
                            height=CANVAS_HEIGHT
                            style="touch-action: none; display: block;"
                            on:pointerdown=on_pointer_down
                            on:pointermove=on_pointer_move
                            on:pointerup=on_pointer_up
                            on:pointerleave=on_pointer_leave
                        ></canvas>

                        <Show when=move || pending_text_pos.get().is_some()>
                            {move || {
                                let (x, y) = pending_text_pos.get().unwrap_or((0.0, 0.0));
                                view! {
                                    <div
                                        class="text-popup"
                                        style=format!("left: {x}px; top: {y}px;")
                                    >
                                        <input
                                            type="text"
                                            prop:value=move || text_draft.get()
                                            on:input=move |ev| set_text_draft.set(event_target_value(&ev))
                                        />
                                        <button type="button" on:click=on_place_text>"配置"</button>
                                        <button type="button" on:click=on_cancel_text>"やめる"</button>
                                    </div>
                                }
                            }}
                        </Show>

                        <Show when=move || pending_photo_pos.get().is_some()>
                            {move || {
                                let (x, y) = pending_photo_pos.get().unwrap_or((0.0, 0.0));
                                let (nw, nh) = pending_photo_size.get();
                                let scale = pending_photo_scale.get();
                                let (w, h) = (nw * scale, nh * scale);
                                let src = pending_photo.with_value(|p| {
                                    p.as_ref().map(|(_, url)| url.clone()).unwrap_or_default()
                                });
                                view! {
                                    <img
                                        src=src
                                        class="photo-preview"
                                        style=format!(
                                            "left: {}px; top: {}px; width: {w}px; height: {h}px;",
                                            x - w / 2.0,
                                            y - h / 2.0,
                                        )
                                    />
                                }
                            }}
                        </Show>
                    </div>
                </div>
            </div>

            <div>
                <button type="button" on:click=on_save>"書き加えて保存"</button>
                <button type="button" on:click=on_undo>"やり直す"</button>
                <Show when=has_canvas>
                    <button type="button" on:click=on_download>"この寄せ書きをダウンロード"</button>
                </Show>
            </div>

            <p>{move || status.get()}</p>
        </div>
    }
}
