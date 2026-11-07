use super::*;

#[test]
fn terminal_draw_confirms_only_the_actual_buffer_to_consumer_path_even_when_it_empties() {
    for draw_from_head in [false, true] {
        let (mut chunks, mut journal, terrain) = fixture();
        let head = [0, 3, 0];
        let middle = [1, 3, 0];
        let tap = [2, 3, 0];
        put(&mut chunks, &mut journal, head, GLASS_PIPE, 0, 8);
        put(&mut chunks, &mut journal, middle, GLASS_PIPE, 0, 0);
        put(&mut chunks, &mut journal, tap, FAUCET, 1, 0);
        put(&mut chunks, &mut journal, [2, 2, 0], 3, 0, 0);
        let mut runtime = Runtime::default();
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        assert_eq!(journal.plumbing_supply_at(tap).unwrap().route.len(), 3);
        // The same live route can draw its existing local buffer or upstream
        // admitted water. Clear earlier activity to isolate this actual draw.
        for cell in [head, middle, tap] {
            let mut state = journal.plumbing_at(cell).unwrap().clone();
            state.water.amount = if cell == if draw_from_head { head } else { tap } {
                8
            } else {
                0
            };
            state.water.flow = [0; 2];
            state.flowing = false;
            journal.set_plumbing(cell, state).unwrap();
        }
        assert_eq!(total(&journal), 8);
        assert!(take_tap(&chunks, &mut journal, tap, 8).unwrap());
        assert_eq!(total(&journal), 0);
        for cell in [head, middle, tap] {
            let expected = cell == tap || draw_from_head;
            assert_eq!(journal.plumbing_at(cell).unwrap().flowing, expected);
            let view = visual_data(&chunks, &journal, cell).unwrap();
            assert_eq!(view.amount, 0);
            assert_eq!(
                view.flowing, expected,
                "a consumed buffer is not proof that transit stopped"
            );
        }
        step(&mut runtime, &mut chunks, &mut journal, &terrain);
        for cell in [head, middle, tap] {
            assert!(!visual_data(&chunks, &journal, cell).unwrap().flowing);
        }
    }
}
