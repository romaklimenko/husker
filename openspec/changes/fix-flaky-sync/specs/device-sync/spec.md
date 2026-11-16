## MODIFIED Requirements

### Requirement: A text conflict keeps both versions

Husker SHALL keep both texts when the local database and the sync file changed `title` or `description` of one task to different values.
The task keeps the text from the sync file. Husker also creates a conflict copy, which is a new task.
The `uid` of a conflict copy is derived from the `uid` of the task and the title and description the copy keeps.
Two devices that keep the same text of one task therefore create one conflict copy, not two.
Husker adds no conflict copy when a task with that `uid`, title, and description already exists.
The conflict copy has every field of the local version except `position`, with ` #conflict` appended to its title. A title that already ends with `#conflict` stays as it is.
The conflict copy gets the position that the merge gives the original task, even when the sync file moved the original.
The requirement `Positions are unique after a merge` then places the conflict copy directly after the original task.
A conflict in any other field creates no conflict copy.

#### Scenario: Keep two titles

- **WHEN** device A renames task 7 to `Buy oat milk`, device B renames it to `Buy whole milk`, and A syncs first
- **THEN** after B syncs, task 7 is titled `Buy oat milk` and a new task is titled `Buy whole milk #conflict`
- **AND** after A syncs again, A shows both tasks

#### Scenario: Place the copy after the original

- **WHEN** task 3 has position 3 and is the task with the highest position, device A renames it, device B renames it to `Email the bank`, and A syncs first
- **THEN** B's sync gives `Email the bank #conflict` position 4, leaves task 3 at position 3, and reports `Received 2, sent 1`

#### Scenario: Follow an original that the other device moved

- **WHEN** task 7 has position 7 and task 8 has position 8, device A renames task 7 and swaps it with task 8, device B renames task 7, and A syncs first
- **THEN** after B syncs, task 8 has position 7, task 7 has position 8, and the conflict copy has position 9

#### Scenario: Take the file's date without a copy

- **WHEN** device A plans task 7 for Monday, device B plans it for Friday, and A syncs first
- **THEN** after B syncs, task 7 is planned for Monday on both devices and no task is added

### Requirement: Positions are unique after a merge

Husker SHALL order tasks by `position` and then `uid` after a merge and raise each position that does not exceed the previous one to the previous position plus one.
A conflict copy that the merge created takes its original task's `position` and `uid` in this order and comes directly after the original task.
Lists receive the same rule without conflict copies. Positions that are already unique do not change.

| Tasks with equal position | Order after the merge |
| --- | --- |
| Two tasks without a conflict copy from this merge | By `uid`. |
| An original task and the conflict copy this merge created | The original task, then the copy. |
| An original task, its new conflict copy, and another task | The original and its copy stay together. The other task comes before both or after both, by the `uid` of the original. |
| A conflict copy from an earlier merge and another task | By `uid`. |

#### Scenario: Reorder two tasks created on two devices

- **WHEN** devices A and B each create a task with position 60 before syncing
- **THEN** after both sync, the two tasks have positions 60 and 61 on both devices in the same order

#### Scenario: Keep the original before its copy

- **WHEN** a merge creates a conflict copy of the task at position 60, and the copy's `uid` sorts before the task's `uid`
- **THEN** the task keeps position 60 and the copy gets position 61

#### Scenario: Keep the copy next to its original

- **WHEN** device B moved task 7 to position 9 and device A added a task with position 9, both devices renamed task 7, and A syncs first
- **THEN** after B syncs, the conflict copy's position is one above task 7's position, and A's task is not between them
