use leptos::prelude::*;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::components::message_board::MessageBoard;
use crate::components::theme_list::ThemeList;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/yosegaki-app.css"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // テーマ一覧の更新通知(ThemeListで作成 → MessageBoardの選択肢にも反映)
    let themes_version = RwSignal::new(0u32);
    provide_context(themes_version);

    view! {
        <h1>"寄せ書きアプリ"</h1>
        <ThemeList />
        <MessageBoard />
    }
}