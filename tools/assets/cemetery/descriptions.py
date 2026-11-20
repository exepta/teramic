"""Player-facing English tooltips; translations live in the UI locale source."""

def describe(d):
    f=d['family']
    if f.startswith('clay_'):return f"{d['name']} for colorful walls, floors and ornamental patterns."
    if d['id']<=343:return f"{d['name']} for the weathered walls of old graves and crypts."
    if 'slab' in f:return f"{d['name']} for cemetery paths and crypt floors."
    if f in ('column','ornate_column','capital','base','arch','moss_arch'):return f"{d['name']} for monumental entrances, arcades and vaulted halls."
    if f in ('relief','skull_relief','cross_relief'):return f"{d['name']} carved with a memorial ornament."
    if f=='niche':return 'A recessed stone alcove lit by a group of warm candles.'
    if f in ('banner','torn_banner','curtain'):return f"{d['name']} in faded purple cloth with golden trim."
    if f.startswith('gravestone_') or f=='cross':return f"{d['name']} with a carved memorial and a weathered stone base."
    if f=='bordered_grave':return 'A stone-bordered resting place with flowers growing in the earth.'
    if f=='sarcophagus':return 'A sealed stone sarcophagus decorated with golden bands.'
    if f=='open_sarcophagus':return 'An open stone sarcophagus with an old lid and scattered bones.'
    if f=='chest':return 'An ornate chest for storing valuables in a crypt.'
    if f in ('lantern','torch','candles','chandelier'):return f"{d['name']} casts warm light over paths and chambers."
    if f=='brazier':return 'A stone brazier filled with violet spirit light.'
    if f=='glow_mushrooms':return 'Purple mushrooms that cast a faint light among old graves.'
    if f in ('grass','thorn','ivy','moss','mushrooms','vines'):return f"{d['name']} from the overgrown grounds of old burial sites."
    if f in ('skeleton','bones','urn','books','skull','chain'):return f"{d['name']} for furnishing ancient graves and crypt chambers."
    if f in ('plate','lever','switch','portcullis','gate','spikes','arrows'):return f"An ornamental {d['name'].lower()} for crypt entrances and dungeon chambers."
    if f in ('crystal_small','crystal_large'):return 'A cluster of soul crystals glowing with violet light.'
    if f=='rune':return 'Carved runes glow with violet spirit energy.'
    if f=='inactive_rune':return 'Ancient runes lie dark on the surface of this stone.'
    if f=='mist':return 'A translucent violet stone for mystical decorations.'
    raise ValueError(f)
