use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, PointerEvent};

pub const CANVAS_WIDTH: u32 = 600;
pub const CANVAS_HEIGHT: u32 = 400;

/// 指・マウス・タッチペンで自由に線を描けるキャンバス。
/// Pointer Events を使うことで、iPad の指操作もそのまま扱える。
/// `canvas_ref` は親コンポーネントが所有し、投稿時に画像を取り出すのに使う。
#[component]
pub fn DrawingCanvas(canvas_ref: NodeRef<leptos::html::Canvas>) -> impl IntoView {
    let is_drawing = StoredValue::new(false);

    let get_ctx = move || -> Option<CanvasRenderingContext2d> {
        canvas_ref
            .get()?
            .get_context("2d")
            .ok()??
            .dyn_into::<CanvasRenderingContext2d>()
            .ok()
    };

    let on_pointer_down = move |ev: PointerEvent| {
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
        if !is_drawing.get_value() {
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

    let on_clear = move |_| {
        if let Some(ctx) = get_ctx() {
            ctx.clear_rect(0.0, 0.0, CANVAS_WIDTH as f64, CANVAS_HEIGHT as f64);
        }
    };

    view! {
        <div class="drawing-canvas">
            <canvas
                node_ref=canvas_ref
                width=CANVAS_WIDTH
                height=CANVAS_HEIGHT
                style="touch-action: none; border: 1px solid #999; background: #ffffff; max-width: 100%; display: block;"
                on:pointerdown=on_pointer_down
                on:pointermove=on_pointer_move
                on:pointerup=on_pointer_up
                on:pointerleave=on_pointer_leave
            ></canvas>
            <button type="button" on:click=on_clear>"消す"</button>
        </div>
    }
}
