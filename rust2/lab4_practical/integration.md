# GitHub Issues Integration

We use GitHub Issues as the bug tracking system for this project.

## Configuration
- Issue template: `.github/ISSUE_TEMPLATE/bug_report.md`
- PR template: `.github/PULL_REQUEST_TEMPLATE.md`
- Triage workflow: `.github/workflows/issue-triage.yml`

## Workflow
1. Create an issue with a short summary, steps to reproduce, expected/actual behavior.
2. Add or let the triage workflow add labels (bug, priority:high/medium/low, etc.).
3. Fix the issue on a feature branch.
4. In the PR description, reference the issue with `Fixes #<id>` to auto-close it.
5. Link the PR or commit in the issue before closing.
