# Quiet Android Activity lifecycle patch

Upstream: `winit 0.30.13`, from the crates.io source archive. Upstream LICENSE is retained.

Three files are changed:

- `src/event_loop.rs`: Android's `EVENT_LOOP_CREATED` guard is thread-local. Every NativeActivity owns a separate `android_main` thread and `AndroidApp`/Looper; Android may replace the initial app-icon Activity with a HOME Activity. A process-wide guard incorrectly rejects this transition. The per-thread guard still rejects a second event loop on the same Activity thread. Other platforms retain the original process-wide guard.
- `src/platform_impl/android/mod.rs`: `MainEvent::Destroy` dispatches `WindowEvent::Destroyed` and exits the event loop. NativeActivity waits for `android_main` to return during destruction. Ignoring Destroy blocks the Java main thread and prevents a replacement Activity from creating its window.
- `src/platform_impl/android/keycodes.rs`: Android BACK maps to `NamedKey::Escape`. egui-winit ignores `BrowserBack`; mapping to Escape lets Quiet close its app drawer or settings using the system Back button. The IME still handles Back first while visible.

The Android backend and renderer are otherwise unchanged. Recheck this patch when updating winit, and rerun the emulator HOME/LAUNCHER transition tests before removing it.
