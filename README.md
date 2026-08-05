# Githist &emsp; [![Latest Version]][crates.io]

[Latest Version]: https://img.shields.io/crates/v/githist.svg
[crates.io]: https://crates.io/crates/githist

A text user interface for moving between recent branches in a git repository.

usage: `githist` followed by an optional path to a repo, defaulting to the working directory.

Branches are ordered by how recently you checked them out (from the reflog), then by last commit time. Remote-only branches are listed dimmed; selecting one creates a local tracking branch.

### keys

| key | action |
|-----|--------|
| `↓`/`↑` or `j`/`k` | choose branch |
| `↩` | switch to selected branch, or print the worktree path when the branch is checked out elsewhere |
| `-` | switch to the previously checked-out branch |
| `/` | fuzzy filter (type to narrow, `↑`/`↓` to move, `↩` to switch, `Esc` to leave) |
| `Shift+D` | delete branch (warns if not merged into HEAD) |
| `g`/`G`, `Home`/`End`, `PgUp`/`PgDn` | jump around the list |
| `q`/`Esc` | quit |

If the working tree is dirty when switching, githist asks whether to stash the changes, bring them along, or cancel. Stashed changes are restored automatically the next time you switch back to the branch (look for the ⚑ stashed marker). Filtering to a name that matches nothing offers to create that branch.

Branches checked out in another git worktree show a magenta `W` gutter marker and a truncated path; press `↩` on one to exit and print its absolute path on stdout (for `cd "$(githist)"` wrappers). The TUI renders on `/dev/tty` so stdout stays clean for that path.

### Agent sessions

Linking Cursor Agent CLI sessions to branches is opt-in. With no links saved, githist looks and behaves exactly as before — no extra markers, help text, or prompts.

Once you link sessions:

| key | action |
|-----|--------|
| `Shift+A` | link the selected branch to an agent session (recent sessions for this repo, or paste a session id) |
| `a` | switch to the selected branch, then show a skippable resume picker if that branch has links |
| `↩` | unchanged — switch or print worktree path; never opens the resume picker |

Branches with linked sessions show a dim cyan `a` gutter marker (when not `*` or `W`). Help mentions `a` / `Shift+A` only when this repo has any links or the selected row has links.

In the resume picker: `↩` runs `agent --resume <id>` in the target directory; `Esc`/`q` skips resume and exits with the usual farewell or worktree path; `u` unlinks the selected session. Resuming does not print on stdout, so `cd "$(githist)"` wrappers keep working.

Links are stored per repository at `~/.config/githist/<repo-id>/agents.json` (repo id from normalized `origin` URL, or a hash of the toplevel path when there is no remote).

Optional environment variables:

- `CURSOR_AGENT_SESSION_ID` — when set, prepends that session as the first link candidate
- `GITHIST_CONFIG_DIR` — override the config directory (default `~/.config`)

### demo



https://github.com/user-attachments/assets/ed8b0be0-563b-47bd-ada5-26b3979f4701


