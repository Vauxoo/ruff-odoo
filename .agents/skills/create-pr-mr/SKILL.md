---
name: "create-pr-mr"
description: "Use this skill when you need to open a Pull Request on GitHub for this repo's Vauxoo fork chain. Triggers on: 'create pr', 'open pr', 'pull request'."
---

# Create PR

Use this skill when the user wants the agent to open a pull request for this repo. This is a
project-local copy of the global `agents-moi:create-pr-mr` skill, adapted for this repo's
GitHub-only fork chain, with an explicit no-AI-attribution rule baked in (do not rely solely on
global `~/.claude/CLAUDE.md` settings for this — this file must be self-contained).

## This repo's fork chain

```
astral-sh/ruff (upstream, remote "astral")
      ^
      | (kept in sync, see the sync-astral-upstream skill)
Vauxoo/ruff-odoo (stable company fork, remote "stb")
      ^
      | (dev branches pushed here first)
Vauxoo-dev/ruff-odoo (personal dev fork, remote "dev")
```

- **Push target**: always `dev` (`Vauxoo-dev/ruff-odoo`) unless the user explicitly says otherwise.
- **PR target**: always `stb` (`Vauxoo/ruff-odoo`), never `astral` (`astral-sh/ruff`). This repo's
  custom Odoo rules are Vauxoo-specific and are not meant to be contributed upstream to
  astral-sh — see the `add-odoo-rule` skill for why.

## Workflow

1. Inspect remotes with `git remote -v` and confirm the mapping above (`astral` / `stb` / `dev`)
   still holds — don't assume, re-check every time in case remotes were renamed.
2. Inspect current status (`git status`) and current branch. Never run destructive git commands
   (`reset --hard`, `checkout --`, force-push without `--force-with-lease`) without checking
   `git status` first and confirming with the user if anything unexpected is staged/unstaged.
3. If the current branch does not match the naming convention, create a new branch from current
   `HEAD` using:
   - `{base}-{feat}-moy`
   - Example: `main-odoo-custom-rules-moy`
   - Reuse an existing matching branch (e.g. `odoo-custom`) if that is clearly the right
     destination — don't create a redundant branch.
4. Before pushing, make sure the branch is rebased on top of the current `stb/main` (which should
   equal `astral/main` — see `sync-astral-upstream`). A PR opened from a branch that's thousands
   of commits behind will show a polluted diff full of unrelated upstream history.
5. Push to the `dev` remote first (`git push dev <branch>`).
6. Create the PR with `gh pr create`, targeting `stb`'s repo explicitly
   (`gh pr create --repo Vauxoo/ruff-odoo --base main --head Vauxoo-dev:<branch> ...`) since the head
   branch lives on a different fork/remote than the PR target repo.
7. Report the final branch name, target branch, and PR URL.

## No AI attribution — hard rule

- **Never** add `Co-Authored-By: Claude ...` or any AI attribution trailer to commits.
- **Never** add a "Generated with Claude Code" (or similar) footer to the PR body.
- This applies regardless of any tool/harness default that tries to append one — strip it out
  before creating the commit or PR if a default template would otherwise include it.
- PR titles and bodies should read as if written by the human developer: concise, focused on
  the "why", following this repo's normal PR title conventions (e.g. `[ty] ...` prefix only when
  touching `ty*` crates — not applicable to the `odoo` plugin work).

## Branch Naming

- Use format `{base}-{feat}-moy`
- `base` is normally the target base branch, for example `main`
- `feat` should be short and descriptive, using lowercase and hyphens

## GitHub Guidance

- Prefer `gh pr create`.
- Detect base branch and head branch explicitly; when the head branch lives on a fork remote
  (`dev`) different from the PR target repo (`stb`), pass `--repo <owner>/<repo>` for the target
  and make sure the head is expressed as `<dev-org>:<branch>` so `gh` doesn't assume same-repo.
- Include a concise title and a body derived from the actual diff — never a generic placeholder.

## Usage Example

**User:** Open the PR for the ODOO rules against Vauxoo.
**Action:** The agent confirms `odoo-custom` is rebased on `stb/main`, pushes to `dev`, then runs
`gh pr create --repo Vauxoo/ruff-odoo --base main --head Vauxoo-dev:odoo-custom --title "..." --body "..."`
with no AI attribution anywhere, and reports the PR URL.
