//! The seam over `getDisplayMedia` — step 8 of the structure-refactor
//! plan. `start_sharing` / `switch_source_handler` (`session::media`)
//! take a `DisplayCapture` instead of calling
//! `infra::webrtc::capture_display` directly, so a test can hand them a
//! stream without a real capture prompt.
//!
//! This isn't a stylistic nicety: headless Chrome has no display to
//! capture, so `capture_display()` always rejects there — every
//! `start_sharing` "happy path" (the stream actually gets stored, the
//! native-stop listener gets attached, `StartShare` gets sent) has had
//! zero coverage until now, only the cancelled-picker branch. A fake
//! `DisplayCapture` that resolves closes that gap.
//!
//! Static dispatch (a generic parameter), not `dyn` — `capture` is an
//! `async fn` in the trait, and the two call sites this seam covers are
//! all that exist, so there's no dyn-safety cost to pay for a
//! never-exercised trait-object path.
//!
//! The trait lives here (all of `infra` is `hydrate`-only); the real
//! `BrowserDisplayCapture` impl lives in `session::media` instead — a
//! zero-sized marker with no `web_sys` inside it, so the `ssr` build's
//! inert `switch_source_handler` stub, which shares a call site with
//! the `hydrate` one, can still name the type even though `infra`
//! doesn't exist there at all.

pub(crate) trait DisplayCapture {
    async fn capture(&self) -> Result<web_sys::MediaStream, wasm_bindgen::JsValue>;
}
