# Інтеграція — вибір: GitHub Issues

Ми інтегруємо GitHub Issues у проект — це природний вибір якщо репозиторій розміщений на GitHub.

Переваги інтеграції через GitHub:
- Тісна інтеграція з Pull Requests та commits.
- Можливість автоматичного закриття issue через текст в описі PR (`Fixes #123`).
- Доступний REST API та `gh` CLI для автоматизації.
- Projects / Actions / Discussions у тій же платформі.

Швидкий план інтеграції (локально/на сервері):

1) Підготуйте репозиторій і увійдіть через `gh` (GitHub CLI):

```powershell
gh auth login
```

2) Створити issue з CLI (приклад):

```powershell
gh issue create --title "Bug: incorrect tag formatting" --body "CLI prints tags without spaces. Severity: minor" --label bug --assignee @me
```

3) Зв'язати PR з issue (коли створите PR):

- У повідомленні PR напишіть `Fixes #<issue-number>` — після злиття issue закриється автоматично.

4) Автоматизація: можна додати GitHub Actions для автоматичного створення issues з шаблонів або triage:

- Додайте `.github/workflows/issue-triage.yml` для автоматичного присвоєння лейблів за ключовими словами (опціонально).

Примітка: я не маю доступу до вашого облікового запису GitHub, тому нижче у `issues/` збережено приклади баг-репортів у Markdown, які ви можете скопіювати і створити як реальні Issues через веб/CLI.

Команди для створення issue з файлу (CLI):

```powershell
# створити issue з локального markdown файлу
gh issue create --title "$(head -n1 issues/bug-001.md)" --body-file issues/bug-001.md --label bug --assignee @me
```

---

Далі `issues/` містить приклади баг-репортів, а `fixes/` — посилання на зміни в коді, що їх виправляють.