use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, PointerEvent};

use crate::api::{create_theme, list_themes, submit_contribution, Theme};

pub const CANVAS_WIDTH: u32 = 640;
pub const CANVAS_HEIGHT: u32 = 480;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Pen,
    Text,
}

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

/// 1つのテーマ = 1枚の共有キャンバス。
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
    let is_drawing = StoredValue::new(false);

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

    // テーマ切り替え・テーマ一覧更新のたびに、共有キャンバスの「今の状態」を読み込み直す
    Effect::new(move |_| {
        let theme_id = selected_theme.get();
        let canvas_key = theme_id.and_then(|id| {
            themes
                .get()
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.canvas_key.clone())
        });
        let Some(canvas) = canvas_ref.get() else {
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

    let get_ctx = move || -> Option<CanvasRenderingContext2d> {
        get_2d_context(&canvas_ref.get()?)
    };

    let on_pointer_down = move |ev: PointerEvent| {
        if tool.get_untracked() == Tool::Text {
            set_pending_text_pos.set(Some((ev.offset_x() as f64, ev.offset_y() as f64)));
            return;
        }
        ev.prevent_default();
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.set_pointer_capture(ev.pointer_id());
        }
        if let Some(ctx) = get_ctx() {
            ctx.begin_path();
            ctx.move_to(ev.offset_x() as f64, ev.offset_y() as f64);
        }
        is_drawing.set_value(true);
    };

    let on_pointer_move = move |ev: PointerEvent| {
        if tool.get_untracked() != Tool::Pen || !is_drawing.get_value() {
            return;
        }
        ev.prevent_default();
        if let Some(ctx) = get_ctx() {
            ctx.line_to(ev.offset_x() as f64, ev.offset_y() as f64);
            ctx.set_line_width(4.0);
            ctx.set_line_cap("round");
            ctx.set_line_join("round");
            ctx.set_stroke_style_str("#222222");
            ctx.stroke();
        }
    };

    let on_pointer_up = move |_ev: PointerEvent| {
        is_drawing.set_value(false);
    };

    let on_pointer_leave = move |_ev: PointerEvent| {
        is_drawing.set_value(false);
    };

    let on_place_text = move |_| {
        let Some((x, y)) = pending_text_pos.get_untracked() else {
            return;
        };
        let text = text_draft.get_untracked();
        if !text.trim().is_empty() {
            if let Some(ctx) = get_ctx() {
                ctx.set_fill_style_str("#222222");
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

    let selected_canvas_key = move || {
        selected_theme
            .get()
            .and_then(|id| themes.get().iter().find(|t| t.id == id).and_then(|t| t.canvas_key.clone()))
    };
    let selected_title = move || {
        selected_theme
            .get()
            .and_then(|id| themes.get().iter().find(|t| t.id == id).map(|t| t.title.clone()))
            .unwrap_or_else(|| "yosegaki".to_string())
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
            </div>

            <div style="position: relative; display: inline-block;">
                <canvas
                    node_ref=canvas_ref
                    width=CANVAS_WIDTH
                    height=CANVAS_HEIGHT
                    style="touch-action: none; border: 1px solid #999; max-width: 100%; display: block;"
                    on:pointerdown=on_pointer_down
                    on:pointermove=on_pointer_move
                    on:pointerup=on_pointer_up
                    on:pointerleave=on_pointer_leave
                ></canvas>

                <Show when=move || pending_text_pos.get().is_some()>
                    {move || {
                        let (x, y) = pending_text_pos.get().unwrap_or((0.0, 0.0));
                        view! {
                            <div style=format!(
                                "position: absolute; left: {x}px; top: {y}px; background: white; border: 1px solid #999; padding: 4px;"
                            )>
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
            </div>

            <div>
                <button type="button" on:click=on_save>"書き加えて保存"</button>
                <Show when=move || selected_canvas_key().is_some()>
                    <a
                        href=move || format!("/img/{}", selected_canvas_key().unwrap_or_default())
                        download=move || format!("{}.png", selected_title())
                    >"この寄せ書きをダウンロード"</a>
                </Show>
            </div>

            <p>{move || status.get()}</p>
        </div>
    }
}
