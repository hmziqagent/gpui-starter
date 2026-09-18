use gpui_starter::web::bootstrap;

fn main() {
    // The full startup path (single-instance preflight, GPUI application,
    // init + first window, exec-reload tail) lives in the lib so the wasm
    // entry (`#[wasm_bindgen(start)]` in `gpui_starter::web`) boots through
    // the exact same code. See src/web.rs.
    bootstrap();
}
