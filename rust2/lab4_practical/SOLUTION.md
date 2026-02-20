# Практична робота 4 — Рішення

Проєкт: `rust2`
Виконавець: (вкажіть своє ім'я)

## Знайдені баги та виправлення

1) Bug 001 — CLI: tags formatting
- Симптом: при додаванні тега вивід був `cat,holiday` замість `cat, holiday`.
- Файл: `lab3/cli/src/main.rs`.
- Виправлення: зміна форматування `join(",")` -> `join(", ")`.

2) Bug 002 — JsonStore: missing parent dir
- Симптом: запис індексу падав при вказанні шляху у неіснуючій директорії.
- Файл: `lab3/core/src/store.rs`.
- Виправлення: перед записом викликається `create_dir_all` для батьківської директорії.

## Команди (коротко)
```powershell
git checkout -b fix/bug-001
git add lab3/cli/src/main.rs
git commit -m "fix(cli): format tags with comma+space"
git push -u origin fix/bug-001

git checkout -b fix/bug-002
git add lab3/core/src/store.rs
git commit -m "fix(store): create parent dir before writing index.json"
git push -u origin fix/bug-002
```

Після створення PR вкажіть в описі `Fixes #<issue-number>`.

---

Додайте тут реальні посилання після створення Issue/PR:

- Issue Bug 001: __________________
- PR Bug 001: __________________
- Issue Bug 002: __________________
- PR Bug 002: __________________
