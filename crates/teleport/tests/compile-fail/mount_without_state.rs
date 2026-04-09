use teleport::TeleportRouter;

#[derive(Clone)]
struct AppState;

fn main() {
    // This should fail because .mount() requires .state() first — the
    // typestate parameter is `NoState` until `.state(...)` is called, which
    // transitions the router into `WithState`. Only `TeleportRouter<S, WithState>`
    // exposes `.mount()`.
    let router: TeleportRouter<AppState, _> = TeleportRouter::new();
    let _ = router.mount();
}
