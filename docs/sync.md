# Sync tasks between computers

This guide is for people who use Husker on more than one computer and want
the same lists and tasks on each of them. It uses the Azure CLI (`az`) to
create the storage. Sync is available from Husker 0.15.0.

## What sync does

Husker keeps a copy of your lists and tasks in one file in Azure Blob
Storage, the sync file. Each computer still has its own task database,
`tasks.db`, and Husker reads and writes only that database, so you can add
and edit tasks without a network. When the network is there, Husker merges
your changes with the sync file. For a few hundred tasks, the storage costs
a few cents a month at most.

Sync covers every list and every task in the profile, including tasks in
Trash. It does not cover `config.toml`, so each computer keeps its own
language and shortcuts.

Sync is off until you create a file named `sync.toml` in your profile
directory. Without that file, Husker makes no network request and your
tasks stay on your computer.

## Create the storage in Azure

Do this once. You need an Azure subscription and the Azure CLI. Sign in
with `az login` first.

Replace `<account>` in every command with a name for your storage account.
The name must be unique in all of Azure and can hold 3 to 24 lowercase
letters and digits, such as `huskeranna2026`. The commands use the region
`swedencentral`. Use a region near you if you prefer.

Create a resource group and a storage account:

```text
az group create --name husker --location swedencentral
az storage account create --name <account> --resource-group husker --location swedencentral --sku Standard_LRS --kind StorageV2 --min-tls-version TLS1_2 --allow-blob-public-access false
```

Turn on blob versioning, so Azure keeps every earlier upload of the sync
file. The same command keeps a deleted file for 30 days:

```text
az storage account blob-service-properties update --account-name <account> --resource-group husker --enable-versioning true --enable-delete-retention true --delete-retention-days 30
```

Versions add up, because Azure keeps one for every upload. To delete
versions older than 90 days, save this as `lifecycle.json`:

```json
{
  "rules": [
    {
      "enabled": true,
      "name": "delete-old-versions",
      "type": "Lifecycle",
      "definition": {
        "actions": { "version": { "delete": { "daysAfterCreationGreaterThan": 90 } } },
        "filters": { "blobTypes": ["blockBlob"], "prefixMatch": ["husker/"] }
      }
    }
  ]
}
```

Then apply it. You can skip this step. A few hundred tasks take about
25 KB per version, so a year of versions stays far below one gigabyte:

```text
az storage account management-policy create --account-name <account> --resource-group husker --policy @lifecycle.json
```

Create a container named `husker`. A container is a folder for files in a
storage account:

```text
az storage container create --name husker --account-name <account> --auth-mode key
```

Create a stored access policy named `husker-sync`. A stored access policy
is a set of permissions with an expiry date that you can revoke later. This
policy allows reading, creating, and writing files in the container until
January 1, 2036:

```text
az storage container policy create --container-name husker --name husker-sync --permissions rcw --expiry 2036-01-01T00:00:00Z --account-name <account> --auth-mode key
```

Create the URL of the sync file:

```text
az storage blob generate-sas --account-name <account> --container-name husker --name tasks.json --policy-name husker-sync --https-only --full-uri --auth-mode key -o tsv
```

The last command prints one long URL. It starts with
`https://<account>.blob.core.windows.net/husker/tasks.json?` and ends with
a SAS token. A SAS token is a signed key that gives access to this one
file. Copy the whole URL. You need it in the next section.

The file `tasks.json` does not exist yet. Husker creates it the first time
you sync.

Treat the URL like a password. See [Keep the sync file private](#keep-the-sync-file-private).

## Turn on sync for a computer

Create a file named `sync.toml` in your profile directory. The default
profile directory is `~/.husker`, so the file is `~/.husker/sync.toml`. On
Windows, that is normally `C:\Users\<your-user-name>\.husker\sync.toml`. If
you start Husker with `--profile DIRECTORY`, put the file in that directory.

Put the URL from the last `az` command in the file, inside double quotes:

```toml
url = "https://<account>.blob.core.windows.net/husker/tasks.json?<token>"
```

Then run one sync:

```text
husker sync
```

Husker prints what it received and what it sent:

```text
Synced. Received 0, sent 65.
```

Both numbers count lists and tasks. On the first computer, Husker receives
nothing, because the sync file does not exist yet. It sends every list and
task. A profile with 6 lists and 59 tasks sends 65.

From now on, start `husker` as usual. Husker syncs in the background while
it runs. See [When Husker syncs](#when-husker-syncs).

`sync.toml` accepts these settings:

| Setting | Value |
| --- | --- |
| `url` | Required. The URL of the sync file. It must start with `https://`. |
| `auth` | Optional. `"sas"` when the URL holds a SAS token. This is the default. `"managed-identity"` on an Azure VM. See [Sync from an Azure VM with a managed identity](#sync-from-an-azure-vm-with-a-managed-identity). |
| `client_id` | Optional. The client ID of a user-assigned managed identity. Allowed only with `auth = "managed-identity"`. |

If `sync.toml` has a mistake, `husker`, `husker sync`, and a writable
`husker mcp` server stop at startup with an error that names the file. For
example, `auth = "key"` stops them with this error:

```text
Error: Invalid sync settings in C:\Users\anna\.husker\sync.toml: auth must be "sas" or "managed-identity".
```

Correct the file and start again. Commands such as `husker add` do not read
`sync.toml`, so they keep working.

If you run `husker sync` without `sync.toml`, the command fails with
`Sync is not set up.` and names the file to create.

## Add a second computer

1. Install Husker 0.15.0 or newer on the second computer.
2. Copy `sync.toml` from the first computer to the profile directory of the
   second computer. Create the directory `~/.husker` first if Husker has
   never run there.
3. Run `husker sync`.

On a computer where Husker has never been used, the command prints
something like this:

```text
Synced. Received 65, sent 0.
```

The second computer now has the same lists and tasks, with the same task
IDs. A new profile starts with one empty list named Inbox. That list
becomes the default list from the sync file. For example, if the default
list on your first computer is named Private, list 1 on the second computer
is now named Private and holds the same tasks.

A computer that already has tasks keeps them. Husker merges the two sets
and deletes nothing:

- The default list on this computer merges with the default list in the
  sync file and takes its name. For example, if this computer has `Inbox`
  with 1 task and the sync file has `Private` with 3 tasks, you get one
  list named `Private` with 4 tasks.
- Another list merges with a list of the same name. Case and surrounding
  spaces don't matter, so `work` on this computer merges with `Work` in the
  sync file. The merged list is named `Work`.
- A list that only one side has stays as it is, with its tasks.

After that sync, the first computer receives the added tasks at its next
sync. The incoming tasks can get different IDs on each computer. See
[Task IDs on different computers](#task-ids-on-different-computers).

## When Husker syncs

The terminal interface syncs in the background. You never wait for the
network while you type, and a sync never replaces text you are typing.

| When | What Husker does |
| --- | --- |
| You start `husker`. | Starts a sync at once. Your local tasks appear without waiting for it. |
| 60 seconds pass after the last change to your tasks. | Sends the change. |
| Every 60 seconds. | Checks the sync file for changes from your other computers. |
| You press a key, paste, or use the mouse after 60 seconds without input. | Starts a sync, so you see current tasks when you come back. |
| You press F5. | Starts a sync at once. |
| You quit while changes are unsent. | Shows `Syncing...` and sends them. Husker waits at most 10 seconds. |

A change therefore leaves your computer within about a minute while Husker
is open. If the sync at exit fails, Husker still quits. It prints
`Sync failed:` with the reason and `Your changes stay on this computer.`
The next sync sends the changes.

A writable MCP server (`husker mcp`) syncs on its own schedule:

| When | What the server does |
| --- | --- |
| The client starts the server. | Syncs before it answers the first request. |
| 2 seconds pass after a change. | Sends the change. |
| Every 60 seconds. | Checks the sync file for changes. |
| The client stops the server while changes are unsent. | Sends them. |

A server started with `--read-only` never syncs. See
[Run an agent on another computer](mcp.md#run-an-agent-on-another-computer).

Commands such as `husker add`, `husker edit`, and `husker done` do not
sync. Run `husker sync` after them, or let the next sync from the terminal
or an MCP server send the change. For example:

```text
husker add "Buy milk"
husker sync
```

The second command prints `Synced. Received 0, sent 1.`

## What happens when two computers change the same task

Most of the time, you change a task on one computer and the others receive
it. The rules below apply when two computers change the same task before
either has seen the other's change.

Changes to different fields combine. For example, you mark `Buy milk` done
on your laptop and change its planned date on your desktop. After both
computers sync, the task is done and has the new planned date on both.

For a change to the same field, the value that reached the sync file first
stays. For example, you plan a task for Monday on your laptop and for
Friday on your desktop, and the laptop syncs first. The task is planned for
Monday on both computers. This rule covers the list, the planned date, the
status, Trash, and the position among other tasks. Husker does not compare
clocks, so a wrong clock on one computer changes nothing.

If both computers changed the title or the description, Husker keeps both
texts. The task keeps the text that reached the sync file first. The second
text becomes a new task whose title ends with `#conflict`, placed next to
the original. For example, you rename task 7 to `Buy oat milk` on your
laptop and to `Buy whole milk` on your desktop, and the laptop syncs first.
After the desktop syncs, task 7 is `Buy oat milk` and a new task is named
`Buy whole milk #conflict`. The new task holds everything the desktop had
for that task, including its description and planned date.

The computer that syncs second shows
`Sync kept both versions of a task. Search for #conflict.` If you ran
`husker sync` there, its output ends with `Kept both versions of 1.` To
find the copies later, press `/`, type `#conflict`, and press Enter. Copy what you
need into the original task, then move the copy to Trash. To keep the copy
as a task of its own, remove `#conflict` from its title.

Deleting a task forever on one computer deletes it on the others at their
next sync. Emptying Trash does the same. The exception is a task that
another computer edited before it received the deletion. Husker keeps that
task with the edit, and the task returns to the computer where you deleted
it. Moving a task up or down in its group does not count as an edit, so a
task that was only reordered elsewhere stays deleted.

Lists follow the same rules. If you delete a list on one computer while
another computer adds a task to that list, the list is removed everywhere
and the task moves to the default list. If two computers each create a list
named `Work`, you get one `Work` list with the tasks from both.

## Task IDs on different computers

A task normally has the same ID on every computer. If you create task 61 on
your laptop and sync, your desktop receives it as task 61.

IDs differ when two computers each created tasks before either synced. For
example, both computers create a task while offline, and both tasks get ID
61. After they sync, each computer keeps its own task as 61 and gives the
other task a higher ID. `husker show 61` then shows a different task on
each computer. The same happens when a computer that already has tasks
joins the sync file.

An ID never moves to another task on the same computer. If 61 is `Buy milk`
on your laptop, 61 stays `Buy milk` on your laptop. Use the ID that the
computer you are working on shows, for example in the editor heading or in
the output of `husker tasks`.

A task's `version`, which `--if-version` checks, also belongs to one
computer. Read the version on the computer where you run the edit.

## Read the sync messages in the terminal

The terminal interface shows these messages. Each disappears after five
seconds.

| Message | What it means |
| --- | --- |
| `Synced changes from another device.` | A sync changed lists or tasks on this computer. You don't need to do anything. |
| `Sync kept both versions of a task. Search for #conflict.` | Two computers changed the same title or description. See [What happens when two computers change the same task](#what-happens-when-two-computers-change-the-same-task). |
| `Sync failed:` followed by a reason and `Your changes stay on this computer.` | The sync did not complete. Your tasks are safe in the local database, and you can keep working. |

Husker shows a failure once. It does not repeat the message until a sync
has succeeded again. Meanwhile, it keeps trying every 60 seconds, when you
press F5, and when you return after 60 seconds without input.

A sync that changes nothing on this computer shows no message.

The reason in a failure message tells you what to do:

| Reason | Cause | What to do |
| --- | --- | --- |
| `No connection to the sync storage.` | There is no network, or the storage did not answer within 10 seconds. A misspelled storage account name in `url` gives the same reason, because that address does not exist. | Keep working. Husker sends your changes when the connection is back. Press F5 to try at once. If it never works, check the account name in `url`. |
| `The sync storage refused access.` | The URL in `sync.toml` is wrong, expired, or revoked. On an Azure VM, the managed identity is missing or has no role on the container. | Compare `url` with the URL that `az storage blob generate-sas` printed, or create a new URL. On a VM, check the role assignment. |
| `The sync storage gave an unexpected answer.` | The storage answered with another error, for example because the container in `url` does not exist. | Check the container name in `url`. Run `husker sync` to see the details. |
| `The sync file needs a newer Husker.` | A newer Husker on another computer wrote the sync file in a newer format. | Update Husker on this computer. Until then, this computer keeps its tasks and does not sync. |
| `The sync file is not valid.` | The file at the URL is not a Husker sync file. Someone may have edited or replaced it. | Husker never overwrites a file it cannot read. Restore an earlier version of the file. See [Recover an earlier version of the sync file](#recover-an-earlier-version-of-the-sync-file). |
| `The sync file kept changing.` | Other computers uploaded several times in a row while this one tried to sync. | Nothing. The next sync tries again. |
| `The database could not be updated.` | Husker could not read or write `tasks.db` during the sync. | Run `husker sync` to see the details. |

`husker sync` prints more detail than the terminal message, such as the
HTTP status and the error code from Azure. It never prints the URL.

With Dansk selected, the messages are in Danish. A failure starts with
`Synkronisering mislykkedes:` and the exit message is `Synkroniserer...`.

## Sync from an Azure VM with a managed identity

Use this setup for an agent that runs Husker on an Azure virtual machine. A
managed identity is an account that Azure gives the VM itself. Husker asks
Azure for a short-lived token each time it needs one, so `sync.toml` on the
VM holds no secret.

Give the VM an identity:

```text
az vm identity assign --resource-group <vm-group> --name <vm>
```

Replace `<vm-group>` and `<vm>` with the resource group and name of the VM.
The command prints `systemAssignedIdentity` with an ID. Use that ID as
`<principal-id>` in the next command. `<subscription>` is your subscription
ID, which `az account show --query id -o tsv` prints.

Allow the identity to read and write files in the `husker` container:

```text
az role assignment create --assignee-object-id <principal-id> --assignee-principal-type ServicePrincipal --role "Storage Blob Data Contributor" --scope "/subscriptions/<subscription>/resourceGroups/husker/providers/Microsoft.Storage/storageAccounts/<account>/blobServices/default/containers/husker"
```

Azure can take a few minutes to apply a new role. Until then, sync fails
with `The sync storage refused access.`

On the VM, create `sync.toml` in the profile directory with the URL without
a token:

```toml
url = "https://<account>.blob.core.windows.net/husker/tasks.json"
auth = "managed-identity"
```

If the VM has a user-assigned identity, add its client ID:

```toml
url = "https://<account>.blob.core.windows.net/husker/tasks.json"
auth = "managed-identity"
client_id = "<client-id>"
```

Then run `husker sync` on the VM, or let the MCP server sync when the
agent's client starts it.

Every computer that syncs holds every list. That includes the VM.
`husker mcp --list Work` limits what the agent's tools can reach. It does
not limit what is stored on that machine. An agent that can run shell
commands or read files on the VM can read `tasks.db` there, with all your
lists. Sync cannot send one list only.

## Stop syncing on a computer

Quit Husker, disconnect MCP clients that use the profile, and delete
`sync.toml` from the profile directory. Your tasks stay in the local
database, and Husker makes no further network request.

The sync file and your other computers are not affected. They keep the
tasks this computer sent before you stopped.

To start again, put `sync.toml` back. Husker merges the changes made in the
meantime.

## Recover an earlier version of the sync file

Azure keeps every earlier upload of the sync file, because you turned on
blob versioning when you created the storage. Use an earlier version when
the sync file was damaged, or when a deletion reached every computer and
you want the tasks back.

To restore a version in the Azure portal:

1. Open the storage account, then Containers, then `husker`.
2. Select `tasks.json` and open the Versions tab.
3. Select the version from before the problem and choose Make current version.

You can do the same with the Azure CLI. Each computer takes the restored
file at its next sync. Changes that were synced after that version are
undone on every computer: newer tasks are removed and newer edits are
reverted. A change a computer has not sent yet is kept, unless the restored
version holds another value for the same field of the same task. Then the
restored value wins, and for a title or description Husker keeps your text
as a `#conflict` copy. To look before you
restore, download the earlier version from the portal and open it in a text
editor. It is plain JSON.

If the sync file is deleted, nothing on your computers is deleted. The next
sync from any computer uploads that computer's tasks as a new sync file.
The other computers then merge with the new file, and they also delete
nothing. A change a computer had not sent yet is kept. A task you deleted
on one computer can come back if another computer had not received the
deletion yet.

Deleting the file has one cost. If the computer that uploads first had not
synced for a while, its older planned dates, statuses, and lists replace
the newer ones on the other computers. A title or description it replaces
is kept as a `#conflict` copy. So restore an earlier version when you can,
and delete the file only to start over. For 30 days, you can also restore
the deleted file in the portal.

Sync is not a backup of tasks you delete. See
[Back up your tasks](usage.md#back-up-your-tasks) for a copy that you
control.

## Keep the sync file private

The URL in `sync.toml` is a password. Anyone who has the URL can read and
replace the sync file until January 1, 2036, the expiry date of the
`husker-sync` policy.

- Keep `sync.toml` out of dotfile repositories, screenshots, and chat
  messages. A copy of your profile directory, such as a backup, contains
  `sync.toml` too.
- If the URL may have leaked, delete the `husker-sync` policy. Every URL
  made from that policy stops working at once:

  ```text
  az storage container policy delete --container-name husker --name husker-sync --account-name <account> --auth-mode key
  ```

  Then create the policy and the URL again, as in
  [Create the storage in Azure](#create-the-storage-in-azure), and put the
  new URL in `sync.toml` on each computer.
- The sync file is plain JSON. It holds the titles and descriptions of all
  your tasks, including tasks in Trash. Husker does not encrypt it. Anyone
  who can read files in your storage account can read your tasks.
- Husker never prints the URL in an error message. An error names
  `sync.toml`, so you know where to look.
- Husker accepts `http://` only for `localhost`, `127.0.0.1`, and `[::1]`.
  That is for testing against a server on your own computer. Every other
  URL must start with `https://`.
