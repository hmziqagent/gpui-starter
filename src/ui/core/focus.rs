use gpui::{App, Context, FocusHandle, Focusable, Subscription, Window};

/// A [`FocusHandle`] plus its blur [`Subscription`], held as a view field so
/// pages don't re-derive focus bookkeeping.
pub struct FocusManager {
    focus_handle: FocusHandle,
    blur_subscription: Option<Subscription>,
}

impl FocusManager {
    /// Create a manager bound to `cx`'s focus handle pool.
    pub fn new<T>(cx: &mut Context<T>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            blur_subscription: None,
        }
    }

    /// Borrow the underlying [`FocusHandle`].
    pub fn handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Subscribe to blur events; replaces any prior subscription. Returns
    /// `&mut self` for chaining.
    pub fn on_blur<V, F>(
        &mut self,
        window: &mut Window,
        cx: &mut Context<V>,
        callback: F,
    ) -> &mut Self
    where
        V: 'static,
        F: Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    {
        let handle = self.focus_handle.clone();
        self.blur_subscription = Some(cx.on_blur(&handle, window, callback));
        self
    }

    /// Request focus for this view's handle.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    /// Whether this handle currently has focus in `window`.
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }
}

impl Focusable for FocusManager {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
