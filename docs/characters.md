# Characters and per-server appearances

The person icon at the lower left of the main menu opens the character editor.
Choose a male or female body, one of **12 male / 14 female hairstyles**, and separate
hair, skin, left/right eye, shirt, trouser, boot and cape colors. The four editor
tabs group body, hairstyles, face and clothing. Body type and skin
color belong to Body; Face offers independent left/right eye colors. Hair contains
hairstyles, hair color and optional colored highlights. Clothing contains garment
styles, colors and materials. Clothing includes a
tunic or undershirt, bare feet or travel boots and an optional explorer cape.
Each garment has an independent **Linen, Canvas, Leather or Wool** material
selector in the Clothing tab. Cloth weave, coarse canvas,
subtle leather grain and wool ribs change the actual surface relief and color
facets; these choices are visible in the editor, portrait and world avatars.
The cape is a closed flared mesh with continuous folds, shoulder yoke and hem;
travel boots have fitted instep straps and side buckles. Hands have forward-facing
knuckles and nails, while the body includes collarbone, shoulder-blade, knee and
calf contours.
These cosmetics grant no equipment statistics. The rotatable live preview uses the same articulated
mesh as the local player, remote players and inventory portrait. Clothing colors
do not recolor skin or equipped armor.

Drag the preview with the left mouse button to rotate the real character; the
arrow buttons also rotate it. Hairstyle thumbnails use the actual models and
selected hair/skin/eye colors. Randomize changes hairstyle, clothing colors and all four garment materials.

**Save** writes the selected local profile. **Cancel** or **Escape** discards the
draft. **Reset** restores the selection present when the editor was opened.
Existing UUIDs, names, named `--profile` files and `--gender=f|m` remain supported;
old profiles receive default colors and a valid hairstyle. Existing shared eye
colors initialize both eyes; highlights are initially disabled. Profiles without
material fields receive a linen top, canvas trousers, leather boots and a wool
cape. Materials remain independent of clothing color and equipped armor.

On the first multiplayer connection, the editor starts with the local appearance.
**Save & join** stores the server choice and then starts the connection. Cancelling
does not connect. Select a saved or discovered server and use **Charakter** in the
server-list footer to change that server's choice later.

Overrides live in `server-characters.toml` beside the client's menu configuration.
They are keyed by local UUID, canonical hostname/IP and port, not the mutable list
entry name. Changing one server never overwrites the local default or another
server's appearance. The selected character is included in the validated join
profile and player snapshots. Client and server require **protocol 133**. Leaving
or failing to start a connection restores the local default.

## Reference NPC cast

The renderer includes the reference sheet's locomotive driver, lumberjack, miner,
unemployed resident, farmer, butcher, bishop, archer, knight, spearman, smith,
seller, travelling merchant and four nomad variants. Older civilian roles remain
available. New role IDs are appended at 13–22; soldiers retain IDs 10–12, so saved
residents do not change into soldiers after upgrading.

NPC faces use a short nose bridge, recessed eye layers, rounded cheeks and
readable voxel-sized mouths; player facial accents follow the chosen skin tone.
Each profession has its own clothing and accessories: peaked uniform cap and
chain, mining lamp, produce basket, cleaver and apron, mitre and crozier, layered
armor, quiver, books, travelling packs, fur trims, braids and hoods. Settlement
generation, names, greetings, resident persistence and bounded navigation use
these registered roles. Merchant appearance does not add a trading economy;
productive jobs and inventory-backed work remain the separate F58/F61 features.

These are authored three-dimensional voxel models reconstructed from the supplied
front-view artwork. Unshown sides and joint articulation are authored geometry;
the image is not used as a flat billboard or represented as an original source model.

The inventory portrait uses the same world model, including equipment. Hold the
left mouse button on the figure and drag horizontally to rotate it. The live
portrait camera is removed when the player inventory closes.

## Development inspection

`TERRA_CHARACTER_GALLERY=players` or `npcs` renders an opt-in native reference
gallery to `target/character-gallery`. Override its directory with
`TERRA_CHARACTER_GALLERY_DIR`. The gallery does not edit a world or profile and
does not run during ordinary startup.
