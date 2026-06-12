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
| `↩` | switch to selected branch |
| `-` | switch to the previously checked-out branch |
| `/` | fuzzy filter (type to narrow, `↑`/`↓` to move, `↩` to switch, `Esc` to leave) |
| `Shift+D` | delete branch (warns if not merged into HEAD) |
| `g`/`G`, `Home`/`End`, `PgUp`/`PgDn` | jump around the list |
| `q`/`Esc` | quit |

If the working tree is dirty when switching, githist asks whether to stash the changes, bring them along, or cancel. Stashed changes are restored automatically the next time you switch back to the branch (look for the ⚑ stashed marker). Filtering to a name that matches nothing offers to create that branch.

### demo



https://github.com/user-attachments/assets/ed8b0be0-563b-47bd-ada5-26b3979f4701


