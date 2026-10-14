use super::allocations;
use super::*;

fn seated_runtime(rows: usize, source: u8, fixed: usize) -> Runtime<Fixture> {
    let mut sources = vec![0; rows];
    sources[1] = source;
    let mut positions = vec![0; rows];
    for &seat in [3, 7, 12, 20, 25].iter().take(fixed) {
        sources[seat] = 4;
        positions[seat] = (seat + 1) as u8;
    }
    Runtime::new(
        Fixture {
            words: (0..rows).map(|index| format!("合成候选{index}")).collect(),
            codes: (0..rows).map(|index| format!("k{index}")).collect(),
            sources,
            positions,
            text: "a".into(),
            ..Fixture::default()
        },
        5,
    )
    .unwrap()
}

#[test]
fn online_seating_reserves_only_the_fixed_english_rows() {
    let mut measurements = Vec::new();
    for rows in [32, 1024, 4096] {
        for source in [2, 3, 12] {
            for fixed in [0, 1, 5] {
                let mut runtime = seated_runtime(rows, source, fixed);
                let (snapshot, snapshot_allocations, snapshot_bytes) =
                    allocations::count_bytes(|| runtime.engine.snapshot().unwrap());
                let (_, refresh_allocations, refresh_bytes) =
                    allocations::count_bytes(|| runtime.refresh().unwrap());
                // 列表已经坐好：刷新只需要排列缓冲，以及非空固定英文行缓冲。
                assert_eq!(runtime.cached.candidates, snapshot.candidates);
                assert_eq!(runtime.cached.candidate_codes, snapshot.candidate_codes);
                assert_eq!(
                    runtime.cached.candidate_annotations,
                    snapshot.candidate_annotations
                );
                assert_eq!(runtime.cached.candidate_sources, snapshot.candidate_sources);
                assert_eq!(
                    runtime.cached.candidate_positions,
                    snapshot.candidate_positions
                );
                assert_eq!(
                    runtime.cached.candidate_corrected,
                    snapshot.candidate_corrected
                );
                assert_eq!(
                    runtime.cached.candidate_answers_key,
                    snapshot.candidate_answers_key
                );
                let budget = (rows + fixed) * std::mem::size_of::<usize>();
                let allocation_budget = 1 + usize::from(fixed > 0);
                eprintln!("rows={rows} source={source} fixed={fixed}: extra_allocations={} extra_requested_bytes={}",
                    refresh_allocations.saturating_sub(snapshot_allocations),
                    refresh_bytes.saturating_sub(snapshot_bytes));
                measurements.push((
                    rows,
                    source,
                    fixed,
                    refresh_allocations.saturating_sub(snapshot_allocations),
                    refresh_bytes.saturating_sub(snapshot_bytes),
                    allocation_budget,
                    budget,
                ));
            }
        }
    }
    assert!(
        measurements
            .iter()
            .all(
                |(_, _, _, allocations, bytes, allocation_budget, budget)| allocations
                    <= allocation_budget
                    && bytes <= budget
            ),
        "固定英文缓冲不应按完整候选列表预留: {measurements:?}"
    );
}

#[test]
fn fixed_english_seats_keep_stable_ties_clamping_and_the_ai_tail() {
    let mut runtime = Runtime::new(
        Fixture {
            words: [
                "本地甲",
                "英文甲",
                "英文乙",
                "英文丙",
                "本地乙",
                "云",
                "AI首行",
                "AI末行",
            ]
            .map(str::to_owned)
            .to_vec(),
            codes: (0..8).map(|index| format!("k{index}")).collect(),
            sources: vec![0, 4, 4, 4, 0, 2, 3, 3],
            positions: vec![0, 3, 3, 255, 0, 0, 0, 0],
            text: "a".into(),
            ..Fixture::default()
        },
        9,
    )
    .unwrap();
    runtime.focus(true).unwrap();
    type_key(&mut runtime);
    assert_eq!(
        runtime.cached.candidates,
        [
            "本地甲",
            "云",
            "英文乙",
            "英文甲",
            "AI首行",
            "本地乙",
            "英文丙",
            "AI末行"
        ]
    );
    assert_eq!(
        runtime.cached.candidate_codes,
        ["k0", "k5", "k2", "k1", "k6", "k4", "k3", "k7"]
    );
    let english = runtime
        .view()
        .candidates
        .iter()
        .find(|candidate| candidate.text == "英文乙")
        .unwrap()
        .id;
    assert_eq!(
        runtime
            .dispatch(Action::Select(english))
            .unwrap()
            .commit
            .as_deref(),
        Some("英文乙")
    );
}
