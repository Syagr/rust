# Практична робота 3 — Розділення на бібліотечний та бінарний крейти

Структура:
- `lab3/core` — бібліотечний крейт `files_index_core` з реалізацією `IndexStore`, `JsonStore`, `SqliteStore` та власним типом помилок `CoreError` (через `thiserror`).
- `lab3/cli` — бінарний крейт `lab3_cli`, використовує `clap` для CLI і обробляє помилки у `main` через `anyhow`.

Запуск (PowerShell):
```powershell
cd C:\It\projects\rust\rust2
setx FILES_INDEX_PATH "json:C:\tmp\files_index.json"
$env:FILES_INDEX_PATH = "json:C:\tmp\files_index.json"

cargo run --manifest-path lab3\cli\Cargo.toml -- add --path C:\Images\photo.jpg --tags cat,holiday
cargo run --manifest-path lab3\cli\Cargo.toml -- get --tags cat
```

Контрольні питання (коротко):
- `thiserror` — дозволяє визначати явні типи помилок у бібліотеках; `anyhow` — зручний для бінарників як контейнер для будь-яких помилок під час виконання.
- У бібліотеках краще експортувати typed errors (для програм-клієнтів), у бінарниках — зручно використовувати `anyhow` для агрегування помилок і короткого оброблення.
- Паніка у бібліотеках погана, бо вона завершить виконання програми; бібліотека повинна повертати помилку і дозволити споживачу вирішити, як обробити її.
