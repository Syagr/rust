# Fixes applied

Нижче перелік внесених виправлень та посилання на файли у робочому каталозі.

- `lab3/cli/src/main.rs` — покращено формат виводу тегів при додаванні файлу (join with ", ").
  - Файл: [lab3/cli/src/main.rs](lab3/cli/src/main.rs)

- `lab3/core/src/store.rs` — перед записом JSON-файлу створюються батьківські директорії при їх відсутності.
  - Файл: [lab3/core/src/store.rs](lab3/core/src/store.rs)

Як пов'язати з баг-репортами:
- Bug 001 → `issues/bug-001.md` → виправлення: `lab3/cli/src/main.rs`
- Bug 002 → `issues/bug-002.md` → виправлення: `lab3/core/src/store.rs`

Щоб створити реальні issues у GitHub, використайте `gh issue create` або API, приклади в `integration.md`.