## MODIFIED Requirements

### Requirement: The sync file holds one snapshot of the profile

Husker SHALL store the synced state of a profile as one UTF-8 JSON object, the sync file, with these members.

| Member | Value |
| --- | --- |
| `format` | The integer 1. |
| `collection` | A random identifier created when the sync file is first written. Later uploads keep it. |
| `default_list` | The `uid` of the default list. |
| `uploads` | The identifiers of the most recent uploads by devices, oldest first, at most 20. Each upload by a device adds one random identifier. |
| `lists` | One object per list with `uid`, `id`, `name`, and `position`. |
| `tasks` | One object per task with `uid`, `id`, `list`, `title`, `description`, `due`, `status`, `status_changed_on`, `status_changed_at`, `deleted`, and `position`. |

These members of a list object or task object have a fixed form.

| Member | Value |
| --- | --- |
| `list` | The `uid` of the task's list. |
| `due`, `status_changed_on` | A `YYYY-MM-DD` string or null. |
| `status_changed_at` | An RFC 3339 UTC string or null. |
| `status` | `open`, `done`, or `cancelled`. |
| `deleted` | True for a task in Trash. |
| `id` | The integer ID the list or task had on the device that first uploaded it. Later uploads keep that value. |

The snapshot includes tasks in Trash and excludes task versions.
The online server also writes the sync file. The online-server spec defines the uploads of the online server. Each upload by the online server changes one list or task and leaves `uploads` unchanged. The online server never creates the sync file.
For a list or task that the online server created, `id` is the value that the online-server requirement `New records get an identifier, an ID hint, and a position` defines.
Husker rejects a downloaded sync file in each of these cases.

| Rejected sync file |
| --- |
| It is not valid JSON. |
| It has an unknown member. |
| It repeats a `uid`. |
| It has an empty `uid`, an empty task title, or a list name that is empty after trimming. |
| It has an empty `collection`. |
| Its `default_list` is not the `uid` of a list in the file. |
| An `id` is negative or above 2^53, or a `position` is outside -2^53 through 2^53. |
| Its `format` is missing or is not 1. |

A rejected sync file changes no local data, and Husker does not replace it.

#### Scenario: Refuse a file from a newer Husker

- **WHEN** the downloaded sync file has `"format": 2`
- **THEN** the sync fails and tells the user to update Husker
- **AND** local tasks and the sync file are unchanged

#### Scenario: Receive a task the online server added

- **WHEN** the online server adds a task to the sync file and a device then syncs
- **THEN** the device stores the task
- **AND** `uploads` in the sync file is the same as before the online server's upload
