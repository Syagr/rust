# Практична робота 2 (lab2_fresh)

Файл-індекс CLI з двома варіантами збереження: JSON та SQLite.

Конфіг (змінна оточення):
```
FILES_INDEX_PATH=json:C:\path\to\index.json
FILES_INDEX_PATH=sqlite:C:\path\to\index.sqlite
```

Приклади:
```powershell
setx FILES_INDEX_PATH "json:C:\tmp\files_index.json"
$env:FILES_INDEX_PATH = "json:C:\tmp\files_index.json"

# add
cargo run --manifest-path lab2_fresh\Cargo.toml -- add --path C:\Images\photo.jpg --tags cat,holiday

# get
cargo run --manifest-path lab2_fresh\Cargo.toml -- get --tags cat
```

Контрольні питання (коротко):
- Статичний поліморфізм (generics) — zero-cost, бо компілятор спеціалізує код під типи під час компіляції.
- Динамічний поліморфізм (`dyn Trait`) використовує vtable і виклики через інтерфейс під час виконання.
- Не можна зробити об'єктом (`dyn Trait`) trait, який має методи з `Self` або generic методи або не є object-safe.
- `T` передається з відомим розміром; `&dyn T` — fat pointer (ptr + vtable) та виклики йдуть через vtable.
