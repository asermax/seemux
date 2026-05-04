# DES-011: Background Thread HTTP Polling with AtomicBool Stop

**Scope**: Project-wide
**Date**: 2026-05-04
**Last Updated**: 2026-05-04

## Pattern

For recurring HTTP polling from the GTK main loop, spawn a persistent background thread that performs blocking HTTP requests on a fixed interval and sends results through `mpsc::channel`. A `glib::timeout_add_local` timer on the main thread does non-blocking `try_recv()` to consume results. Use `Arc<AtomicBool>` as a stop flag for clean thread termination when the polling is no longer needed.

## Rationale

Synchronous HTTP calls inside `glib::timeout_add_local` block the entire GTK main loop. Even fast localhost requests can hang (e.g., dead process with lingering socket), freezing the UI for the HTTP client's timeout duration (10s for `minreq`). Moving the HTTP call to a background thread keeps the main loop responsive. The `AtomicBool` stop flag allows clean shutdown when the polled resource is destroyed.

This extends DES-001 (one-shot work + mpsc + poll) to the recurring polling case, where the background thread loops indefinitely and the GTK timer repeatedly consumes results.

## Examples

### Do This

```rust
// Background thread loops: sleep -> check stop flag -> HTTP GET -> send result
// Main thread: non-blocking try_recv() every 50ms
let (tx, rx) = mpsc::channel();
let stop = Arc::new(AtomicBool::new(false));
let flag = stop.clone();
std::thread::spawn(move || {
    while !flag.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
        if flag.load(Ordering::Relaxed) { break; }
        if let Some(result) = http_get(&url) {
            if tx.send(result).is_err() { break; }
        }
    }
});

let source_id = glib::timeout_add_local(Duration::from_millis(50), move || {
    match rx.try_recv() {
        Ok(result) => { /* update UI */ ControlFlow::Continue }
        Err(TryRecvError::Empty) => ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => ControlFlow::Break,
    }
});

// On cleanup: set flag + remove timer
stop.store(true, Ordering::Relaxed);
source_id.remove();
```

**Why**: HTTP call runs off the main thread. Main loop only does non-blocking `try_recv()`. Stop flag ensures thread terminates promptly on cleanup.

### Don't Do This

```rust
// Synchronous HTTP directly in the GTK timer callback
glib::timeout_add_local(Duration::from_millis(500), move || {
    let response = minreq::get(&url).send(); // BLOCKS main loop!
    // ...
});
```

**Why**: Blocks the GTK main loop for the HTTP client's timeout (up to 10s) on every tick. If the target hangs, the entire UI freezes.

## Exceptions

- For sub-millisecond operations that cannot hang (e.g., reading a local file), synchronous calls in the timer are acceptable.
- For one-shot operations (single request, not recurring), use DES-001 directly instead of this recurring pattern.

---

## Related

- See also: [DES-001](DES-001-background-thread-mpsc-poll.md) - Background Thread + mpsc + GTK Poll (one-shot variant)
- Used by: `src/session/manager.rs` (CDP URL polling for browser panes)
