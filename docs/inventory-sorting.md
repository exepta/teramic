# Inventory sorting

The icon-only funnel at the bottom right of the backpack opens upward, directly
beside the trash/catalog controls. Open storage, connected workbench chests and
train cargo have their own funnel. Hover the icon to identify its target.

- **A–Z** sorts case-insensitively by the actual item display name.
- **ID** sorts block IDs numerically, then stable item identifiers for other items.
- **Menge** puts larger stacks first, with stable category/item ordering for ties.

All modes first combine identical stacks up to their actual stack limit. Wood,
color, heraldry, water quantity and other item metadata remain distinct. Selecting
an option closes the menu. Clicking the funnel again, clicking outside or pressing
Escape closes it without sorting.

Backpack sorting keeps the nine hotbar slots and the entire contents of each
crafting-reserved slot in place. Storage sorting combines both halves of an open
double chest without moving player items. The train dialog offers separate
buttons for cargo and the player's backpack.

Furnace, crusher, mixer, cauldron and campfire process slots, plus filters and
tunnels, retain their functional slot arrangement; their backpack section can
still be sorted. Put any held cursor stack down before sorting. Buttons remain
disabled while a transaction is pending.

The server validates access, inventory revisions, reservations and every affected
storage half before publishing changes. The client waits for confirmation and
does not locally rearrange item stacks. Client and server require protocol 133.
