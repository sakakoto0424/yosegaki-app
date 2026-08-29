use leptos::{prelude::*, task::spawn_local};

use crate::api::{create_theme, list_themes, Theme};

#[component]
pub fn ThemeList() -> impl IntoView {
    let (themes, set_themes) = signal(Vec::<Theme>::new());
    let (new_title, set_new_title) = signal(String::new());
    let (status, set_status) = signal(String::new());
    let themes_version =
        use_context::<RwSignal<u32>>().expect("themes_version context should be provided");

    // 初回表示、および他コンポーネントでのテーマ作成時にも一覧を再取得
    Effect::new(move |_| {
        let _ = themes_version.get();
        spawn_local(async move {
            match list_themes().await {
                Ok(list) => set_themes.set(list),
                Err(e) => set_status.set(format!("読み込みに失敗しました: {e}")),
            }
        });
    });

    let on_submit = move |_| {
        let title = new_title.get();
        if title.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            match create_theme(title).await {
                Ok(()) => {
                    set_new_title.set(String::new());
                    set_status.set(String::new());
                    themes_version.update(|v| *v += 1);
                }
                Err(e) => set_status.set(format!("作成に失敗しました: {e}")),
            }
        });
    };

    view! {
        <div class="theme-list">
            <h2>"寄せ書きテーマ一覧"</h2>
            <ul>
                <For
                    each=move || themes.get()
                    key=|theme| theme.id
                    children=move |theme: Theme| {
                        view! { <li>{theme.title.clone()}" (" {theme.created_at.clone()} ")"</li> }
                    }
                />
            </ul>

            <h3>"新しいテーマを作る"</h3>
            <input
                type="text"
                placeholder="例: 2026年度体育祭2年5組"
                prop:value=move || new_title.get()
                on:input=move |ev| set_new_title.set(event_target_value(&ev))
            />
            <button type="button" on:click=on_submit>"作成"</button>
            <p>{move || status.get()}</p>
        </div>
    }
}
