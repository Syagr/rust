# Приклади для Практичної роботи 6 — Send / Sync

Ці приклади показують прості випадки, коли компілятор забороняє операції через відсутність `Send` або `Sync`.

1) `Rc` не можна передати в інший потік

```rust
// rc_send.rs
use std::rc::Rc;
use std::thread;

fn main() {
    let r = Rc::new(5);
    // Помилка: Rc<i32> не є Send
    let h = thread::spawn(move || {
        println!("Rc = {}", r);
    });
    h.join().unwrap();
}
```

2) `RefCell` не є `Sync` — не можна загорнути в `Arc` для спільного доступу між потоками

```rust
// refcell_sync.rs
use std::cell::RefCell;
use std::sync::Arc;
use std::thread;

fn main() {
    let c = Arc::new(RefCell::new(0));
    let c2 = Arc::clone(&c);
    let h = thread::spawn(move || {
        *c2.borrow_mut() += 1; // Помилка: RefCell не є Sync
    });
    h.join().unwrap();
}
```

3) `MutexGuard` не можна переслати в інший потік

```rust
// guard_send.rs
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let m = Arc::new(Mutex::new(0));
    let guard = m.lock().unwrap();
    let h = thread::spawn(move || {
        drop(guard); // Помилка: MutexGuard не є Send
    });
    h.join().unwrap();
}
```

Скомпілюйте кожен файл і уважно читайте повідомлення компілятора — воно прямо вкаже, який авто-трейт відсутній.