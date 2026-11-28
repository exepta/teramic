pub(super) type Color = [f32; 3];

#[derive(Clone, Copy)]
pub(super) struct Coat {
    pub body: Color,
    pub breast: Color,
    pub hackle: Color,
    pub lacing: Color,
    pub flights: Color,
    pub tail: Color,
    pub sheen: Color,
    pub barred: bool,
    pub rose_comb: bool,
}

impl Coat {
    const fn new(colors: [u32; 7], barred: bool, rose_comb: bool) -> Self {
        let [body, breast, hackle, lacing, flights, tail, sheen] = colors;
        Self {
            body: srgb(body),
            breast: srgb(breast),
            hackle: srgb(hackle),
            lacing: srgb(lacing),
            flights: srgb(flights),
            tail: srgb(tail),
            sheen: srgb(sheen),
            barred,
            rose_comb,
        }
    }
}

// Authored sRGB triplets, packed only to keep each plumage readable together.
const fn srgb(rgb: u32) -> Color {
    [
        ((rgb >> 16) & 255) as f32 / 255.,
        ((rgb >> 8) & 255) as f32 / 255.,
        (rgb & 255) as f32 / 255.,
    ]
}

pub(super) const HENS: [Coat; 6] = [
    // Light Sussex: silver body, dark-striped collar and black wing tips.
    Coat::new(
        [
            0xdcd8be, 0xf0e8cb, 0xf3e9c7, 0x252b29, 0x383932, 0x192426, 0x49544a,
        ],
        false,
        false,
    ),
    // Partridge: chestnut shoulders, warm breast and dark pencilling.
    Coat::new(
        [
            0x784022, 0x92502e, 0xc78737, 0x44210e, 0x512c1e, 0x2b1b17, 0x594335,
        ],
        false,
        false,
    ),
    // Buff: light straw neck and a soft copper fan.
    Coat::new(
        [
            0xd29b49, 0xdfac62, 0xf3ce79, 0xad7130, 0xac743e, 0x8d582e, 0xcf9147,
        ],
        false,
        false,
    ),
    // Barred Plymouth Rock: graphite and pale transverse bands.
    Coat::new(
        [
            0x3c4246, 0x515858, 0xb0b3a5, 0x292d31, 0x272b30, 0x292e32, 0x8c938d,
        ],
        true,
        false,
    ),
    // Gold-laced Wyandotte: dark scalloped edges, gold centers and rose comb.
    Coat::new(
        [
            0x392c1d, 0x715024, 0xc9973c, 0x302319, 0x81531f, 0x142323, 0x36544b,
        ],
        false,
        true,
    ),
    // Blue-laced: smoky blue flight feathers and pale silver scallops.
    Coat::new(
        [
            0x657484, 0x8d999b, 0xbac3b6, 0x374650, 0x526375, 0x354957, 0x82989f,
        ],
        false,
        true,
    ),
];

pub(super) const ROOSTERS: [Coat; 4] = [
    // Black-breasted red with gold hackles and beetle-green sickles.
    Coat::new(
        [
            0x713019, 0x232825, 0xe4a62e, 0x843709, 0x502619, 0x0d292a, 0x246153,
        ],
        false,
        false,
    ),
    // Silver duckwing with a silver saddle and blue-black sickles.
    Coat::new(
        [
            0x87877a, 0x282d2d, 0xe7e8d6, 0x535657, 0x343a3a, 0x17252e, 0x456071,
        ],
        false,
        false,
    ),
    // Barred cock with more silver on the breast and tail than the hen.
    Coat::new(
        [
            0x5a6263, 0x667071, 0xc9c9ad, 0x3b4243, 0x323a39, 0x384140, 0x969f91,
        ],
        true,
        false,
    ),
    // Blue-red: copper saddle, slate breast and blue-green sickles.
    Coat::new(
        [
            0x9a5736, 0x4a5765, 0xe0aa64, 0x784020, 0x626572, 0x374655, 0x71818e,
        ],
        false,
        true,
    ),
];
