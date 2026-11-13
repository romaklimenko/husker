## Context

Compute the visible heading and task rows together before rendering them.
Read from source: navigation currently lets Ratatui scroll a flat list of headings and tasks to the selected task.
The offset can land on a task, hiding its heading.

## Goals / Non-goals

Keep date context visible in long ordinary task views and History.
Preserve task selection and align mouse targets and page movement with the visible rows.
Storage, grouping rules, date labels, list pickers, and Trash headings are out of scope.

## Decisions

Keep the saved scroll offset in the full row sequence.
Build one viewport row mapping that includes the first visible group's heading when its natural position is above the viewport.
Render that mapping with a temporary selection state, so Ratatui cannot independently change the saved offset.
Use the same mapping for mouse targets and page counts.

Reuse the existing heading row, including its localized label, date, count, and style.
When there is only one available row, show the selected task.
Simple scroll padding was rejected because it cannot retain a heading while scrolling deep into a large group.

## Risks / Trade-offs

A retained heading uses one task row of height. Keep the selected task visible and test small viewports and resizing.
Changing the row mapping can misalign mouse targets. Test actual rendered cells and heading clicks.
Test repeated frames to detect offset drift.
No migration is needed. Reinstall the prior executable to roll back.
