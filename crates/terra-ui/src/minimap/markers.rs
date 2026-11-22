//! Small map silhouettes follow our animal palette without allocating per marker.
use terra_core::creatures::CreatureKind;

const SIDE: usize = 11;
pub(super) const ICON_SIZE: usize = SIDE + 2;
const OUTLINE: [u8; 4] = [24, 29, 30, 255];
const ALERT: [u8; 4] = [188, 61, 52, 255];
type Sprite = [&'static str; SIDE];
const DRAGON: Sprite = [
    "w   w w   w",
    "yy  yey  yy",
    "yyw yyy wyy",
    " ynnyyynny ",
    "  ynyny y  ",
    "  y yyy y  ",
    "   yyyyy   ",
    "   wy yy   ",
    "      yy   ",
    "       yyy ",
    "         yy",
];
const SAND_WORM: Sprite = [
    "    ooo    ",
    "   yyyyy   ",
    "  yeyyyey  ",
    "  ywnnnwy  ",
    "  ynnnnny  ",
    "  yywwwyy  ",
    "   yyyyy   ",
    "   oyyo    ",
    "   yyyyo   ",
    "  yyyyoo   ",
    " yyyyyy    ",
];
const SKELETON: Sprite = [
    "   wwwww   ",
    "  wwwwwww  ",
    "  wewewew  ",
    "   wwww    ",
    "    www    ",
    "  wwwwwww  ",
    " w w w w w ",
    " w  www  w ",
    " w  w w  w ",
    "    w w    ",
    "   ww ww   ",
];
const RAVEN: Sprite = [
    "   bbbbb   ",
    "  bbbbbbb  ",
    "  bwebbbbbb",
    "  bbbbbbbbb",
    "   bbbbb   ",
    "  bbbbbbb  ",
    " bbbbbbbbb ",
    "bbbbbbbbbbb",
    " bbbbbbbbb ",
    "   bb bb   ",
    "  bbb bbb  ",
];
const SLIME: Sprite = [
    "       g   ",
    "   ggggg   ",
    "  ggggggg  ",
    " ggggggggg ",
    " ggegggegg ",
    " ggggggggg ",
    " ggggegggg ",
    " ggggggggg ",
    "ggggggggggg",
    " ggggggggg ",
    "           ",
];
const BUTTERFLY: Sprite = [
    " ppp   ppp ",
    "ppppp ppppp",
    "pwwpp ppwwp",
    "ppwppbppwpp",
    " ppppbpppp ",
    "  pppbppp  ",
    " ppppbpppp ",
    " pwwpbpwwp ",
    "  pppbppp  ",
    "   p b p   ",
    "           ",
];
const GLOOM: Sprite = [
    "     g     ",
    "    gg     ",
    "   wwwww   ",
    "  wwwwwww  ",
    " wwewweww  ",
    "gwwwwwwwwwg",
    "ggwwwwwwwgg",
    "  wwwwwww  ",
    "   wgwgw   ",
    "   gg gg   ",
    "           ",
];
const FOX: Sprite = [
    " w       w ",
    " www   www ",
    " wgwwwwwgw ",
    " wwwwwwwww ",
    "  wewweww  ",
    "   wwwww   ",
    "    wnw    ",
    "   ggggg   ",
    "  ggggggg  ",
    " gg     gg ",
    "           ",
];
const VILLAGER: Sprite = [
    "   bbbbb   ",
    "  bbbbbbb  ",
    "  bpppppb  ",
    "  ppepepp  ",
    "   ppppp   ",
    "   ppppp   ",
    "  ggggggg  ",
    " pgggggggp ",
    " pgggggggp ",
    "   bb bb   ",
    "   bb bb   ",
];
const STALKER: Sprite = [
    "   m b m   ",
    "  bbbbbbb  ",
    " bbeebbeeb ",
    "bb bbbbb bb",
    "b   bbb   b",
    "b   bbb   b",
    "b  bbbbb  b",
    "b  b   b  b",
    "b  b   b  b",
    "bb b   b bb",
    "  bb   bb  ",
];
const SPIDER: Sprite = [
    "b b     b b",
    " b b   b b ",
    "  bbbbbbb  ",
    "bboobobboob",
    "  booooob  ",
    " bbboeobbb ",
    " b boeob b ",
    "b  bbbbb  b",
    "  b bob b  ",
    " b   b   b ",
    "b         b",
];
const HORSE: Sprite = [
    " bb     bb ",
    " bbb   bbb ",
    " bbbbbbbbb ",
    " bbbwbwbbb ",
    "  bbbbbbb  ",
    "  bbbbbbb  ",
    "  bmmmmmb  ",
    "  bmnbnmb  ",
    "   bbbbb   ",
    "    bbb    ",
    "           ",
];

const SHEEP: Sprite = [
    "   wwwww   ",
    "  wwwwwww  ",
    " wwwwwwwww ",
    "eewwwwwwwee",
    " ewwbbbww e",
    "  wbebebw  ",
    "  wbbnbbw  ",
    "   bbbbb   ",
    "   bnnnb   ",
    "    bbb    ",
    "           ",
];
const COW: Sprite = [
    " h       h ",
    " hh     hh ",
    " ebbwwwbbe ",
    "eebbwwwbbee",
    " bbwwwwbbb ",
    " bwewwwbwb ",
    " bwbbbbbwb ",
    " bmmmmmmmb ",
    " bmnbbnmbb ",
    "  bbbbbbb  ",
    "           ",
];
const PIG: Sprite = [
    " pp     pp ",
    " pmp   pmp ",
    " ppppppppp ",
    " ppppppppp ",
    " ppepppepp ",
    " ppppppppp ",
    " ppmmmmppp ",
    " pmnmmnmp  ",
    " pmmmmmmpp ",
    "  ppppppp  ",
    "           ",
];
const BEAR: Sprite = [
    " bb     bb ",
    " bmb   bmb ",
    " bbbbbbbbb ",
    " bbbbbbbbb ",
    " bbebbbebb ",
    " bbbbbbbbb ",
    " bbmmmmmbb ",
    " bbmmnmmmb ",
    "  bmmnmbb  ",
    "   bbbbb   ",
    "           ",
];
const BEE: Sprite = [
    " w       w ",
    " ww     ww ",
    " wwwyyywww ",
    "  wybybyw  ",
    "  yybybyy  ",
    "  yybybyy  ",
    "  yyeyeyy  ",
    "   yyyyy   ",
    "    yyy    ",
    "     b     ",
    "           ",
];
const FISH: Sprite = [
    "           ",
    "      f    ",
    "    ffff   ",
    "f  ffffff  ",
    "ff fffffef ",
    "ffffffffff ",
    "ff fffffef ",
    "f  ffffff  ",
    "    ffff   ",
    "      f    ",
    "           ",
];
const CLOWNFISH: Sprite = [
    "           ",
    "      f    ",
    "    fwwf   ",
    "f  ffwwff  ",
    "ff ffwwfef ",
    "fffffwwfff ",
    "ff ffwwfef ",
    "f  ffwwff  ",
    "    fwwf   ",
    "      f    ",
    "           ",
];
const SQUID: Sprite = [
    "    qqq    ",
    "   qqqqq   ",
    "   qqqqq   ",
    "   qeweq   ",
    "   qqqqq   ",
    "  qqqqqqq  ",
    " q q q q q ",
    " q q q q q ",
    "q  q q q  q",
    "q q  q  q q",
    "           ",
];
const JELLY: Sprite = [
    "    jjj    ",
    "  jjwwwjj  ",
    " jjwwjwwjj ",
    "jjwjjjjjwjj",
    "jjjjjjjjjjj",
    " jwwwwwwwj ",
    "  j j j j  ",
    " j  j j  j ",
    " j j   j j ",
    "  j     j  ",
    "           ",
];
const SHARK: Sprite = [
    "     s     ",
    "    sss    ",
    "    sss    ",
    "  sssssss  ",
    "sssssesssss",
    "  sssssss  ",
    "   swwws   ",
    "    sss    ",
    "    sss    ",
    "   ss ss   ",
    "  ss   ss  ",
];
const HAMMERHEAD: Sprite = [
    "           ",
    " essssssse ",
    "sssssssssss",
    "    sss    ",
    "  sssssss  ",
    "sssssssssss",
    "  sswwwss  ",
    "    sss    ",
    "    sss    ",
    "   ss ss   ",
    "  ss   ss  ",
];
const WHALESHARK: Sprite = [
    "    sss    ",
    "   sswss   ",
    "   wsssw   ",
    "  sssssss  ",
    "sswsseswsss",
    "  swswsws  ",
    "   swwws   ",
    "   sswss   ",
    "    sss    ",
    "   ss ss   ",
    "  ss   ss  ",
];

pub(super) fn creature_icon(
    kind: CreatureKind,
    variant: u8,
    hostile: bool,
) -> [u8; ICON_SIZE * ICON_SIZE * 4] {
    let mut pixels = [0; ICON_SIZE * ICON_SIZE * 4];
    creature(&mut pixels, ICON_SIZE, [6, 6], kind, variant, hostile);
    pixels
}

pub(super) fn player_icon() -> [u8; ICON_SIZE * ICON_SIZE * 4] {
    let mut pixels = [0; ICON_SIZE * ICON_SIZE * 4];
    player(&mut pixels, ICON_SIZE, [6, 6]);
    pixels
}

pub(super) fn creature(
    pixels: &mut [u8],
    size: usize,
    center: [i32; 2],
    kind: CreatureKind,
    variant: u8,
    hostile: bool,
) {
    let sprite = match kind {
        CreatureKind::Sheep => &SHEEP,
        CreatureKind::Cow => &COW,
        CreatureKind::Pig => &PIG,
        CreatureKind::Fish if variant >= 2 => &CLOWNFISH,
        CreatureKind::Fish => &FISH,
        CreatureKind::Bee => &BEE,
        CreatureKind::Bear => &BEAR,
        CreatureKind::Squid => &SQUID,
        CreatureKind::MoonJelly
        | CreatureKind::FireJelly
        | CreatureKind::VoidJelly
        | CreatureKind::SunJelly
        | CreatureKind::CrystalJelly => &JELLY,
        CreatureKind::ReefShark => &SHARK,
        CreatureKind::HammerheadShark => &HAMMERHEAD,
        CreatureKind::WhaleShark => &WHALESHARK,
        CreatureKind::Horse => &HORSE,
        CreatureKind::Stalker => &STALKER,
        CreatureKind::Villager => &VILLAGER,
        CreatureKind::Butterfly => &BUTTERFLY,
        CreatureKind::SpiritGloom => &GLOOM,
        CreatureKind::SpiritFox => &FOX,
        CreatureKind::Skeleton => &SKELETON,
        CreatureKind::Raven => &RAVEN,
        CreatureKind::BloodSlime => &SLIME,
        CreatureKind::BloodStalker => &SPIDER,
        CreatureKind::BloodWisp => &GLOOM,
        CreatureKind::ThornBrute => &STALKER,
        CreatureKind::SandWorm => &SAND_WORM,
        CreatureKind::DarkDragon | CreatureKind::EarthDragon | CreatureKind::WaterDragon => &DRAGON,
        CreatureKind::SmallSpider | CreatureKind::MediumSpider | CreatureKind::LargeSpider => {
            &SPIDER
        }
    };
    let outline = if hostile { ALERT } else { OUTLINE };
    for row in -1..=SIDE as i32 {
        for column in -1..=SIDE as i32 {
            let glyph = sample(sprite, column, row);
            let color = if glyph != b' ' {
                palette(kind, variant, glyph)
            } else if (-1..=1)
                .any(|dy| (-1..=1).any(|dx| sample(sprite, column + dx, row + dy) != b' '))
            {
                outline
            } else {
                continue;
            };
            put(
                pixels,
                size,
                [center[0] + column - 5, center[1] + row - 5],
                color,
            );
        }
    }
}

pub(super) fn player(pixels: &mut [u8], size: usize, center: [i32; 2]) {
    for y in -6_i32..=6 {
        for x in -6_i32..=6 {
            let distance = x.abs() + y.abs();
            let color = match distance {
                7.. => continue,
                6 => OUTLINE,
                5 => [79, 214, 223, 255],
                _ if y == -1 && x.abs() == 1 => [29, 45, 49, 255],
                _ if y >= 2 => [40, 132, 153, 255],
                _ if y <= -2 => [91, 65, 51, 255],
                _ => [231, 194, 157, 255],
            };
            put(pixels, size, [center[0] + x, center[1] + y], color);
        }
    }
}

fn sample(sprite: &Sprite, x: i32, y: i32) -> u8 {
    if !(0..SIDE as i32).contains(&x) || !(0..SIDE as i32).contains(&y) {
        return b' ';
    }
    sprite[y as usize].as_bytes()[x as usize]
}

fn palette(kind: CreatureKind, variant: u8, glyph: u8) -> [u8; 4] {
    let rgb = match (kind, glyph) {
        (CreatureKind::DarkDragon, b'y') => [175, 91, 220],
        (CreatureKind::EarthDragon, b'y') => [208, 148, 75],
        (CreatureKind::WaterDragon, b'y') => [71, 166, 222],
        (CreatureKind::SandWorm, b'y') => [240, 178, 66],
        (CreatureKind::SandWorm, b'o') => [182, 88, 28],
        (CreatureKind::Raven, b'b') => [60, 40, 80],
        (CreatureKind::BloodSlime | CreatureKind::BloodWisp, b'g' | b'w') => [232, 43, 82],
        (CreatureKind::BloodStalker | CreatureKind::ThornBrute, b'b' | b'm' | b'o') => {
            [109, 29, 45]
        }
        (CreatureKind::BloodStalker | CreatureKind::ThornBrute, b'e') => [255, 52, 73],
        (CreatureKind::SpiritGloom, b'g' | b'w') if variant == 1 => [175, 95, 229],
        (CreatureKind::Butterfly, b'p') => [
            [255, 107, 148],
            [245, 240, 255],
            [143, 56, 217],
            [20, 130, 255],
            [255, 171, 20],
            [33, 171, 102],
            [255, 66, 6],
        ][usize::from(variant % 7)],
        (CreatureKind::SpiritGloom | CreatureKind::SpiritFox, b'g') => [111, 222, 250],

        (CreatureKind::Villager, b'p') => [222, 171, 124],
        (CreatureKind::Villager, b'g') => [78, 132, 69],
        (CreatureKind::Villager, b'b') => [83, 61, 43],
        (CreatureKind::Stalker, b'b') => [103, 75, 46],
        (CreatureKind::Stalker, b'm') => [88, 105, 36],
        (CreatureKind::Stalker, b'e') => [255, 191, 47],
        (
            CreatureKind::SmallSpider | CreatureKind::MediumSpider | CreatureKind::LargeSpider,
            b'b',
        ) => [37, 32, 35],
        (
            CreatureKind::SmallSpider | CreatureKind::MediumSpider | CreatureKind::LargeSpider,
            b'e',
        ) => [255, 48, 24],
        (CreatureKind::SmallSpider, b'o') => [240, 96, 34],
        (CreatureKind::MediumSpider, b'o') => [207, 61, 21],
        (CreatureKind::LargeSpider, b'o') => [170, 44, 24],
        (_, b'e' | b'n') => [30, 28, 29],
        (_, b'w' | b'h') => [231, 230, 211],
        (CreatureKind::Sheep, b'b') => [132, 119, 105],
        (CreatureKind::Cow, b'b') => [86, 57, 43],
        (CreatureKind::Cow, b'm') => [190, 144, 127],
        (CreatureKind::Pig, b'p') => [229, 151, 146],
        (CreatureKind::Pig, b'm') => [187, 106, 112],
        (CreatureKind::Bear, b'b') => [115, 79, 53],
        (CreatureKind::Bear, b'm') => [181, 141, 102],
        (CreatureKind::Bee, b'y') => [239, 194, 54],
        (CreatureKind::Bee, b'b') => [66, 48, 30],
        (CreatureKind::Fish, b'f') if variant >= 2 => [239, 123, 45],
        (CreatureKind::Fish, b'f') => [115, 177, 191],
        (CreatureKind::Squid, b'q') => [211, 77, 83],
        (CreatureKind::MoonJelly, b'j') => [113, 187, 244],
        (CreatureKind::FireJelly, b'j') => [245, 123, 53],
        (CreatureKind::VoidJelly, b'j') => [163, 76, 216],
        (CreatureKind::SunJelly, b'j') => [244, 201, 89],
        (CreatureKind::CrystalJelly, b'j') => [68, 224, 232],
        (CreatureKind::ReefShark, b's') => [100, 123, 152],
        (CreatureKind::HammerheadShark, b's') => [145, 129, 109],
        (CreatureKind::WhaleShark, b's') => [57, 103, 157],
        _ => [185, 171, 145],
    };
    [rgb[0], rgb[1], rgb[2], 255]
}

fn put(pixels: &mut [u8], size: usize, point: [i32; 2], color: [u8; 4]) {
    let Ok(x) = usize::try_from(point[0]) else {
        return;
    };
    let Ok(y) = usize::try_from(point[1]) else {
        return;
    };
    if x >= size || y >= size {
        return;
    }
    if let Some(pixel) = y
        .checked_mul(size)
        .and_then(|row| row.checked_add(x))
        .and_then(|index| index.checked_mul(4))
        .and_then(|index| pixels.get_mut(index..index.saturating_add(4)))
    {
        pixel.copy_from_slice(&color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_species_and_fish_variant_have_distinct_markers() {
        let mut variants = Vec::new();
        for (kind, variant) in CreatureKind::ALL
            .into_iter()
            .map(|kind| (kind, 0))
            .chain([(CreatureKind::Fish, 2)])
        {
            let mut pixels = vec![0; 24 * 24 * 4];
            creature(&mut pixels, 24, [12, 12], kind, variant, false);
            assert!(pixels.chunks_exact(4).filter(|p| p[3] == 255).count() > 40);
            assert!(!variants.contains(&pixels));
            variants.push(pixels);
        }
    }

    #[test]
    fn markers_clip_at_image_edges_and_tolerate_invalid_targets() {
        let mut pixels = vec![0; 8 * 8 * 4];
        creature(&mut pixels, 8, [0, 0], CreatureKind::Bear, 0, true);
        player(&mut pixels, 8, [7, 7]);
        assert!(pixels.chunks_exact(4).any(|p| p[3] == 255));
        creature(&mut [], usize::MAX, [0, 0], CreatureKind::Bee, 0, false);
        player(&mut pixels[..3], 1, [0, 0]);
        for sprite in [
            &STALKER,
            &SPIDER,
            &SHEEP,
            &COW,
            &PIG,
            &BEAR,
            &BEE,
            &FISH,
            &CLOWNFISH,
            &SQUID,
            &JELLY,
            &SHARK,
            &HAMMERHEAD,
            &WHALESHARK,
        ] {
            assert!(sprite.iter().all(|row| row.len() == SIDE));
        }
    }
}
