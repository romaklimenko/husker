## ADDED Requirements

### Requirement: Keep the first visible group's heading on screen

Husker SHALL show the first visible task group's heading above its tasks in ordinary task views and History when the task area has at least two rows.
When the heading's natural position scrolls out of view, it occupies the top row of the task area.
The heading uses its existing localized label, date, count, and styling without a duplicate at its natural position.
Selection remains visible. Heading selection, mouse targets, and page counts follow the existing navigation rules.
With only one available row, Husker shows the selected task. Trash remains without headings.

#### Scenario: Return to the first Tomorrow task in a long list

- **WHEN** a user scrolls a list taller than the screen and returns to its first task, planned for tomorrow
- **THEN** Tomorrow, its date, and its group count appear above that task
- **AND** Danish uses the existing localized heading

#### Scenario: Scroll inside and between groups

- **WHEN** the viewport begins partway through a group
- **THEN** that group's heading appears once above its visible tasks
- **AND** when the first visible task belongs to the next group, the top heading identifies that group

#### Scenario: Preserve interaction after scrolling or resizing

- **WHEN** a user scrolls or resizes a grouped view and clicks a visible task
- **THEN** the displayed task is selected and clicking the retained heading changes nothing
- **AND** PageUp and PageDown count only displayed task rows

#### Scenario: Retain a History heading

- **WHEN** History scrolls into the middle of a status-date group
- **THEN** its existing status-date heading appears above the visible tasks

#### Scenario: Use a one-row task area

- **WHEN** a grouped view has only one row available for tasks
- **THEN** the selected task is visible without a retained heading

#### Scenario: Scroll Trash

- **WHEN** a user scrolls a Trash view taller than the screen
- **THEN** task rows appear without group headings
