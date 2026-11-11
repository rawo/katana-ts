//! The signal chain's order. The patch's CHAIN setting picks one of nine
//! orders of BOOSTER, MOD, FX, DELAY and the AMP; EQ 1 and EQ 2 sit next to
//! the AMP by their POSITION; DELAY 2 / REVERB and SOLO come last. From
//! Tone Studio's chain page (docs/settings.md, "Chain orders").

use crate::settings::setting;
use crate::Patch;

/// A card of the chain row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Booster,
    Mod,
    Amp,
    Fx,
    Delay,
    Dly2Rev,
    Eq1,
    Eq2,
    Solo,
}

impl Block {
    /// The `layout::CHAIN` id of the card.
    pub fn id(self) -> &'static str {
        match self {
            Block::Booster => "booster",
            Block::Mod => "mod",
            Block::Amp => "amp",
            Block::Fx => "fx",
            Block::Delay => "delay",
            Block::Dly2Rev => "dly2rev",
            Block::Eq1 => "eq1",
            Block::Eq2 => "eq2",
            Block::Solo => "solo",
        }
    }
}

use Block::{Amp, Booster, Delay, Fx, Mod};

/// The nine orders, by CHAIN value (pedal FX, foot volume and send/return left out).
pub const ORDERS: [[Block; 5]; 9] = [
    [Booster, Amp, Mod, Fx, Delay],
    [Booster, Mod, Amp, Fx, Delay],
    [Booster, Mod, Fx, Amp, Delay],
    [Booster, Mod, Fx, Delay, Amp],
    [Mod, Booster, Amp, Fx, Delay],
    [Mod, Booster, Fx, Amp, Delay],
    [Mod, Booster, Fx, Delay, Amp],
    [Mod, Fx, Booster, Amp, Delay],
    [Mod, Fx, Booster, Delay, Amp],
];

/// The CHAIN values each block may be dragged to, by CHAIN value (Tone
/// Studio's `CHAIN_BASE_TRANSITION`).
pub const MOVES: [&[(Block, &[u8])]; 9] = [
    &[(Mod, &[1, 4])],
    &[(Booster, &[4]), (Mod, &[0, 4]), (Fx, &[2])],
    &[(Booster, &[5, 7]), (Mod, &[5]), (Fx, &[1]), (Delay, &[3])],
    &[(Booster, &[6, 8]), (Mod, &[6]), (Delay, &[2])],
    &[(Booster, &[1]), (Mod, &[1, 0]), (Fx, &[5, 7])],
    &[(Booster, &[2, 7]), (Mod, &[2]), (Fx, &[4, 7]), (Delay, &[6])],
    &[(Booster, &[3, 8]), (Mod, &[3]), (Fx, &[8]), (Delay, &[5])],
    &[(Booster, &[2, 5]), (Fx, &[4, 5]), (Delay, &[8])],
    &[(Booster, &[3, 6]), (Fx, &[6]), (Delay, &[7])],
];

/// A place a dragged card may be dropped: in front of `before`, applying `edit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub before: Block,
    pub edit: (&'static str, u32),
}

/// (CHAIN value, EQ 1 at AMP OUT, EQ 2 at AMP OUT); odd values count as the
/// default order (2) and AMP IN.
fn layout(patch: &Patch) -> (usize, bool, bool) {
    let get = |table, inst, key| setting(table, inst, key).and_then(|d| patch.get(d));
    let chain = get("OTHER", 0, "chain").filter(|&v| v <= 8).unwrap_or(2) as usize;
    (chain, get("EQ_EACH", 0, "position") == Some(1), get("EQ_EACH", 1, "position") == Some(1))
}

fn full_order(chain: usize, eq1_out: bool, eq2_out: bool) -> Vec<Block> {
    let (before, after): (&[Block], &[Block]) = match (eq1_out, eq2_out) {
        (false, false) => (&[Block::Eq1, Block::Eq2], &[]),
        (true, true) => (&[], &[Block::Eq1, Block::Eq2]),
        (false, true) => (&[Block::Eq1], &[Block::Eq2]),
        (true, false) => (&[Block::Eq2], &[Block::Eq1]),
    };
    let mut out = Vec::new();
    for b in ORDERS[chain] {
        if b == Amp {
            out.extend_from_slice(before);
            out.push(Amp);
            out.extend_from_slice(after);
        } else {
            out.push(b);
        }
    }
    out.extend([Block::Dly2Rev, Block::Solo]);
    out
}

/// The cards in processing order.
pub fn order(patch: &Patch) -> Vec<Block> {
    let (chain, eq1, eq2) = layout(patch);
    full_order(chain, eq1, eq2)
}

/// The block after `b` in `order`.
fn next_after(order: &[Block], b: Block) -> Option<Block> {
    order.iter().position(|&x| x == b).and_then(|i| order.get(i + 1)).copied()
}

/// Where `dragged` may be dropped: BOOSTER, MOD, FX and DELAY into the orders
/// Tone Studio allows from the current one; EQ 1 / EQ 2 to the other side of
/// the AMP. Other cards don't move.
pub fn drops(patch: &Patch, dragged: Block) -> Vec<Drop> {
    let (chain, eq1, eq2) = layout(patch);
    match dragged {
        Block::Eq1 | Block::Eq2 => {
            let (k, out) = if dragged == Block::Eq1 { (1, !eq1) } else { (2, !eq2) };
            let target = if k == 1 { full_order(chain, out, eq2) } else { full_order(chain, eq1, out) };
            let edit = (if k == 1 { "eq1-position" } else { "eq2-position" }, u32::from(out));
            next_after(&target, dragged).map(|before| Drop { before, edit }).into_iter().collect()
        }
        _ => MOVES[chain]
            .iter()
            .filter(|(b, _)| *b == dragged)
            .flat_map(|(_, targets)| targets.iter())
            .filter_map(|&t| {
                let before = next_after(&full_order(t as usize, eq1, eq2), dragged)?;
                Some(Drop { before, edit: ("other-chain", u32::from(t)) })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::Block::*;
    use super::*;
    use crate::{find_param, Patch};

    fn patch(chain: u32, eq1: u32, eq2: u32) -> Patch {
        let mut p = Patch::default_preset();
        for (n, v) in [("other-chain", chain), ("eq1-position", eq1), ("eq2-position", eq2)] {
            p.set(find_param(n).unwrap(), v).unwrap();
        }
        p
    }

    #[test]
    fn orders_match_tone_studio() {
        assert_eq!(ORDERS[0], [Booster, Amp, Mod, Fx, Delay]);
        assert_eq!(ORDERS[3], [Booster, Mod, Fx, Delay, Amp]);
        assert_eq!(ORDERS[6], [Mod, Booster, Fx, Delay, Amp]);
        assert_eq!(ORDERS[8], [Mod, Fx, Booster, Delay, Amp]);
        for o in ORDERS {
            let mut s = o.to_vec();
            s.sort_by_key(|b| b.id());
            assert_eq!(s, [Amp, Booster, Delay, Fx, Mod]);
        }
    }

    #[test]
    fn moves_are_one_block_moves() {
        for (v, moves) in MOVES.iter().enumerate() {
            for &(b, targets) in *moves {
                for &t in targets {
                    let without = |o: &[Block; 5]| o.iter().copied().filter(|&x| x != b).collect::<Vec<_>>();
                    assert_eq!(without(&ORDERS[v]), without(&ORDERS[t as usize]), "{v}: {b:?} -> {t}");
                    assert_ne!(ORDERS[v], ORDERS[t as usize]);
                }
            }
        }
    }

    #[test]
    fn order_places_eqs_around_amp() {
        assert_eq!(order(&patch(0, 1, 0)), [Booster, Eq2, Amp, Eq1, Mod, Fx, Delay, Dly2Rev, Solo]);
        assert_eq!(order(&patch(0, 0, 0)), [Booster, Eq1, Eq2, Amp, Mod, Fx, Delay, Dly2Rev, Solo]);
        assert_eq!(order(&patch(3, 1, 1)), [Booster, Mod, Fx, Delay, Amp, Eq1, Eq2, Dly2Rev, Solo]);
        assert_eq!(order(&patch(3, 0, 1)), [Booster, Mod, Fx, Delay, Eq1, Amp, Eq2, Dly2Rev, Solo]);
    }

    #[test]
    fn odd_chain_value_falls_back() {
        for raw in [9u8, 127] {
            let mut p = patch(0, 1, 1);
            p.mem.apply(crate::Address(find_param("other-chain").unwrap().addr), &[raw]);
            assert_eq!(&order(&p)[..5], [Booster, Mod, Fx, Amp, Eq1], "raw {raw}");
        }
    }

    #[test]
    fn mod_in_order_0_can_go_after_booster_or_first() {
        let d = drops(&patch(0, 1, 1), Mod);
        assert_eq!(
            d,
            [Drop { before: Amp, edit: ("other-chain", 1) }, Drop { before: Booster, edit: ("other-chain", 4) }]
        );
        // With EQ 1 at AMP IN, "before the AMP" is before EQ 1.
        assert_eq!(drops(&patch(0, 0, 1), Mod)[0].before, Eq1);
    }

    #[test]
    fn eq_drops_cross_the_amp() {
        assert_eq!(drops(&patch(0, 0, 1), Eq1), [Drop { before: Eq2, edit: ("eq1-position", 1) }]);
        assert_eq!(drops(&patch(0, 1, 1), Eq1), [Drop { before: Amp, edit: ("eq1-position", 0) }]);
    }

    #[test]
    fn fixed_blocks_cannot_be_dragged() {
        for b in [Amp, Dly2Rev, Solo] {
            assert!(drops(&patch(2, 0, 1), b).is_empty(), "{b:?}");
        }
    }
}
