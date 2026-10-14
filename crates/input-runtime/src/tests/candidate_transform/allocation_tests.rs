use super::*;

#[allow(unsafe_code)]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    struct Counting;

    fn record() {
        let _ = COUNT.try_with(|count| count.set(count.get() + 1));
    }

    // 安全：指针和布局原样委托给 System，只在当前线程累计整数。
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            record();
            unsafe { System.alloc(layout) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            record();
            unsafe { System.alloc_zeroed(layout) }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            record();
            unsafe { System.realloc(pointer, layout, size) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    pub(super) fn count<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        let before = COUNT.with(Cell::get);
        let result = operation();
        (result, COUNT.with(Cell::get) - before)
    }
}

fn large_engine(count: usize) -> TransformEngine {
    TransformEngine {
        words: (0..count).map(|index| format!("合成候选{index}")).collect(),
        ..TransformEngine::default()
    }
}

#[test]
fn unchanged_plugin_refresh_without_recorded_ops_needs_no_identity_allocations() {
    let mut harness = Harness::new(large_engine(1024), vec![CandidateOps::default()]);
    harness.key(b'a');
    let calls = harness.calls();

    let (changed, allocations) = allocations::count(|| harness.runtime.transform_candidates());

    assert!(!changed);
    assert_eq!(harness.calls(), calls);
    assert_eq!(allocations, 0, "未记录操作的旁路刷新不应创建候选身份");
}

#[test]
fn replay_of_one_annotation_has_a_bounded_identity_allocation_budget() {
    let mut measurements = Vec::new();
    for count in [32, 1024, 4096] {
        let mut harness =
            Harness::new(large_engine(count), vec![ops(&[], &[], &[(1, "合成注释")])]);
        harness.key(b'a');
        let calls = harness.calls();

        let (changed, allocations) = allocations::count(|| harness.runtime.transform_candidates());

        eprintln!("rows={count}: identity replay allocations={allocations}");
        assert!(!changed);
        assert_eq!(harness.calls(), calls);
        assert_eq!(
            harness.runtime.script_annotation(1).as_deref(),
            Some("合成注释")
        );
        measurements.push((count, allocations));
    }
    assert!(
        measurements
            .iter()
            .all(|(_, allocations)| *allocations <= 32),
        "身份重放不应逐行复制字符串: {measurements:?}"
    );
}

#[test]
fn duplicate_identity_follows_engine_order_across_the_plugin_window() {
    let mut engine = large_engine(40);
    engine.words[1] = "重复候选".into();
    engine.words[35] = "重复候选".into();
    let mut harness = Harness::new(engine, vec![ops(&[], &[], &[(1, "第一条重复")])]);
    harness.key(b'a');

    // 模拟内置重排：第二条重复进入首屏，已被注释的第一条移到插件窗口之外。
    let mut order = vec![35];
    order.extend((0..40).filter(|seat| *seat != 1 && *seat != 35));
    order.push(1);
    harness.runtime.apply_candidate_order(&order);
    harness.runtime.transform_candidates();
    assert_eq!(harness.runtime.script_annotation(0), None);
    assert_eq!(
        harness.runtime.script_annotation(39).as_deref(),
        Some("第一条重复")
    );

    // 重取快照并插入不同文本，记录仍跟随引擎的第一条重复，不运行插件。
    harness.runtime.engine.words.insert(1, "插入候选".into());
    harness.runtime.refresh_online_candidates().unwrap();
    assert_eq!(
        harness.runtime.script_annotation(2).as_deref(),
        Some("第一条重复")
    );
    assert_eq!(harness.runtime.script_annotation(36), None);
    assert_eq!(harness.calls(), 1);
}
