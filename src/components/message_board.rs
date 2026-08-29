use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;

use crate::api::{list_messages, list_themes, post_drawing_message, post_text_message, Message, Theme};
use crate::components::download::download_yosegaki;
use crate::components::drawing_canvas::{DrawingCanvas, CANVAS_HEIGHT, CANVAS_WIDTH};

#[component]
pub fn MessageBoard() -> impl IntoView {
    let (themes, set_themes) = signal(Vec::<Theme>::new());
    let (selected_theme, set_selected_theme) = signal::<Option<i64>>(None);
    let (messages, set_messages) = signal(Vec::<Message>::new());
    let (mode, set_mode) = signal("text".to_string());
    let (text_input, set_text_input) = signal(String::new());
    let (status, set_status) = signal(String::new());
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let themes_version =
        use_context::<RwSignal<u32>>().expect("themes_version context should be provided");

    let refresh_themes = move || {
        spawn_local(async move {
            if let Ok(list) = list_themes().await {
                if selected_theme.get_untracked().is_none() {
                    if let Some(first) = list.first() {
                        set_selected_theme.set(Some(first.id));
                    }
                }
                set_themes.set(list);
            }
        });
    };

    let refresh_messages = move || {
        if let Some(theme_id) = selected_theme.get_untracked() {
            spawn_local(async move {
                match list_messages(theme_id).await {
                    Ok(list) => set_messages.set(list),
                    Err(e) => set_status.set(format!("読み込みに失敗しました: {e}")),
                }
            });
        } else {
            set_messages.set(Vec::new());
        }
    };

    Effect::new(move |_| {
        let _ = themes_version.get();
        refresh_themes();
    });

    Effect::new(move |_| {
        let _ = selected_theme.get();
        refresh_messages();
    });

    let on_post_text = move |_| {
        let Some(theme_id) = selected_theme.get_untracked() else {
            return;
        };
        let text = text_input.get_untracked();
        spawn_local(async move {
            match post_text_message(theme_id, text).await {
                Ok(()) => {
                    set_text_input.set(String::new());
                    set_status.set(String::new());
                    refresh_messages();
                }
                Err(e) => set_status.set(format!("{e}")),
            }
        });
    };

    let on_post_drawing = move |_| {
        let Some(theme_id) = selected_theme.get_untracked() else {
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
            match post_drawing_message(theme_id, base64).await {
                Ok(()) => {
                    set_status.set(String::new());
                    refresh_messages();
                    if let Some(canvas) = canvas_ref.get_untracked() {
                        if let Ok(Some(ctx)) = canvas.get_context("2d") {
                            if let Ok(ctx) = ctx.dyn_into::<web_sys::CanvasRenderingContext2d>() {
                                ctx.clear_rect(0.0, 0.0, CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64);
                            }
                        }
                    }
                }
                Err(e) => set_status.set(format!("{e}")),
            }
        });
    };

    let on_download = move |_| {
        let Some(theme_id) = selected_theme.get_untracked() else {
            return;
        };
        let title = themes
            .get_untracked()
            .iter()
            .find(|t| t.id == theme_id)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "yosegaki".to_string());
        let msgs = messages.get_untracked();
        spawn_local(async move {
            if let Err(e) = download_yosegaki(title, msgs).await {
                set_status.set(e);
            }
        });
    };

    view! {
        <div class="message-board">
            <h2>"寄せ書きに参加する"</h2>

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
                            view! { <option value=t.id.to_string()>{t.title.clone()}</option> }
                        }
                    />
                </select>
            </label>

            <div>
                <button type="button" on:click=move |_| set_mode.set("text".to_string())>"文字で書く"</button>
                <button type="button" on:click=move |_| set_mode.set("drawing".to_string())>"手書きで書く"</button>
            </div>

            <Show when=move || mode.get() == "text">
                <textarea
                    prop:value=move || text_input.get()
                    on:input=move |ev| set_text_input.set(event_target_value(&ev))
                    maxlength="500"
                ></textarea>
                <button type="button" on:click=on_post_text>"投稿する"</button>
            </Show>

            <Show when=move || mode.get() == "drawing">
                <DrawingCanvas canvas_ref=canvas_ref />
                <button type="button" on:click=on_post_drawing>"この絵を投稿する"</button>
            </Show>

            <p>{move || status.get()}</p>

            <h3>"寄せ書き一覧"</h3>
            <button type="button" on:click=on_download>"この寄せ書きをダウンロード"</button>
            <ul class="message-list">
                <For
                    each=move || messages.get()
                    key=|m| m.id
                    children=move |m: Message| {
                        let is_text = m.kind == "text";
                        view! {
                            <li>
                                <Show when=move || is_text>
                                    <p>{m.text_content.clone().unwrap_or_default()}</p>
                                </Show>
                                <Show when=move || !is_text>
                                    <img
                                        src=format!("/img/{}", m.image_key.clone().unwrap_or_default())
                                        style="max-width: 300px; border: 1px solid #ccc; display: block;"
                                    />
                                </Show>
                            </li>
                        }
                    }
                />
            </ul>
        </div>
    }
}
