# Lab 5 — Розпаралелення важких операцій

Розміщено в `rust2/lab5` — реалізація задач лабораторної роботи 5.

Команди:

Скомпілювати:
```bash
cargo build --release
```

Приклади запуску:

Matrix (тест з меншим розміром для перевірки):
```bash
cargo run -- matrix --size 256 --count 2
```

Encrypt директорії (згенерувати ключ автоматично):
```bash
cargo run -- encrypt --dir ./some_folder
```

Process images (порівняння single vs threaded):
```bash
cargo run -- process-images --dir ./images --out ./out --width 800 --workers 4
```

Опис:
- `matrix`: генератор матриць + два воркери, які паралельно обчислюють суму елементів (з використанням `rayon`).
- `encrypt`: продюсер читає файли, 3 consumer-шифрувальника записують `<filename>.data` і інкрементують лічильник.
- `process-images`: декодування, зміна розміру та кодування виконуються або в одному потоці (single) або рознесені по робітниках (threaded). Знято час виконання та обчислено відносну зміну.
