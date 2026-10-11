//! 用 WAT 写的模块测 ABI 校验、资源限制、配额隔离、快照复位和观察类 import。仓库不提交 `.wasm`；wasmi 的 `wat` 特性只在 dev-dependencies 里打开。

use crate::abi::{candidate_field, commit_field, composition_field, event_field, status};
use crate::limits;
use crate::module::compile_unchecked_format;
use crate::*;
use std::sync::{Arc, Mutex};

fn declaration(capabilities: &[Capability], hooks: &[Hook]) -> Declaration {
    Declaration {
        capabilities: capabilities.iter().copied().collect(),
        hooks: hooks.iter().copied().collect(),
        memory_pages: limits::DEFAULT_MEMORY_PAGES,
    }
}

fn compile_wat(wat: &str, declaration: Declaration) -> Result<ScriptModule, ModuleError> {
    compile_unchecked_format(wat.as_bytes(), declaration)
}

/// 最小的合规模块：`imports` 是 import 声明，`body` 是 `msime_on_event` 的函数体（参数 0 是 kind）。
fn module_wat(imports: &str, extra: &str, body: &str) -> String {
    format!(
        r#"(module
            {imports}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            {extra}
            (func (export "msime_on_event") (param $kind i32) (result i32) {body}))"#
    )
}

fn instance(wat: &str, declaration: Declaration) -> ScriptInstance {
    let module = compile_wat(wat, declaration).expect("module compiles");
    ScriptInstance::new(&module, declaration.capabilities, limits::INIT_FUEL)
        .expect("module instantiates")
}

fn input_event() -> Event<'static> {
    let mut event = Event::new(Hook::Input);
    event.scalars.local_mode = Some(0);
    event.scalars.input_class = Some(0);
    event
}

fn failure(outcome: Outcome) -> Failure {
    match outcome {
        Outcome::Failed { failure, .. } => failure,
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn status_of(outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Delivered { status, .. } => status,
        other => panic!("expected a delivery, got {other:?}"),
    }
}

// ---- 校验 ----

#[test]
fn a_minimal_module_compiles_and_runs() {
    let mut script = instance(
        &module_wat("", "", "local.get $kind"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        status_of(script.deliver(1, &input_event())),
        Hook::Input.code()
    );
}

#[test]
fn the_public_entry_point_accepts_only_binary_modules() {
    let wat = module_wat("", "", "i32.const 0");
    assert_eq!(
        validate(wat.as_bytes(), declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::NotBinary
    );
    let oversized = vec![0u8; limits::MAX_MODULE_BYTES + 1];
    assert_eq!(
        validate(&oversized, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::TooLarge
    );
}

#[test]
fn declarations_are_checked_before_the_module() {
    let wat = module_wat("", "", "i32.const 0");
    assert_eq!(
        compile_wat(&wat, declaration(&[], &[])).unwrap_err(),
        ModuleError::NoHooks
    );
    for pages in [0, limits::MAX_MEMORY_PAGES + 1] {
        let mut declared = declaration(&[], &[Hook::Input]);
        declared.memory_pages = pages;
        assert_eq!(
            compile_wat(&wat, declared).unwrap_err(),
            ModuleError::MemoryPages
        );
    }
}

#[test]
fn rejected_proposals_do_not_compile() {
    let cases = [
        (
            "multi-memory",
            r#"(module (memory (export "memory") 1) (memory 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "tail-call",
            r#"(module (memory (export "memory") 1)
                (func $abi (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) return_call $abi))"#,
        ),
        (
            "extended-const",
            r#"(module (memory (export "memory") 1)
                (global i32 (i32.add (i32.const 1) (i32.const 2)))
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "simd",
            r#"(module (memory (export "memory") 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32)
                    v128.const i32x4 1 2 3 4
                    i32x4.extract_lane 0))"#,
        ),
        (
            "memory64",
            r#"(module (memory (export "memory") i64 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "threads",
            r#"(module (memory (export "memory") 1 1 shared)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "start function",
            r#"(module (memory (export "memory") 1)
                (func $start)
                (start $start)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
    ];
    for (proposal, wat) in cases {
        let error = compile_wat(wat, declaration(&[], &[Hook::Input])).unwrap_err();
        assert!(
            matches!(error, ModuleError::Invalid(_)),
            "{proposal} should be rejected by validation, got {error:?}"
        );
    }
}

#[test]
fn allowed_proposals_compile() {
    // MVP 之外允许的：mutable-global、sign-ext、sat-float-to-int、multi-value、bulk-memory、reference-types。
    let wat = r#"(module
        (memory (export "memory") 1)
        (table 1 funcref)
        (global $g (export "g") (mut i32) (i32.const 0))
        (func (export "msime_abi") (result i32) i32.const 1)
        (func $pair (result i32 i32) i32.const 1 i32.const 2)
        (func (export "msime_on_event") (param i32) (result i32)
            (memory.fill (i32.const 0) (i32.const 0) (i32.const 4))
            (drop (table.get 0 (i32.const 0)))
            (drop (i32.trunc_sat_f32_s (f32.const 1.5)))
            (call $pair) i32.add
            i32.extend8_s))"#;
    compile_wat(wat, declaration(&[], &[Hook::Input])).expect("allowed proposals compile");
}

#[test]
fn memory_and_exports_are_required() {
    let no_memory = r#"(module
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert_eq!(
        compile_wat(no_memory, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingMemory
    );
    let too_big = r#"(module (memory (export "memory") 17)
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert_eq!(
        compile_wat(too_big, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::BadMemory
    );
    let no_on_event = r#"(module (memory (export "memory") 1)
        (func (export "msime_abi") (result i32) i32.const 1))"#;
    assert_eq!(
        compile_wat(no_on_event, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingExport(abi::EXPORT_ON_EVENT)
    );
    let bad_init = module_wat("", r#"(func (export "msime_init"))"#, "i32.const 0");
    assert_eq!(
        compile_wat(&bad_init, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::ExportSignature(abi::EXPORT_INIT)
    );
}

#[test]
fn imports_outside_the_whitelist_are_rejected() {
    let cases = [
        r#"(import "env" "event_scalar" (func (param i32) (result i64)))"#,
        r#"(import "msime" "read_clipboard" (func (param i32) (result i64)))"#,
        r#"(import "msime" "event_scalar" (global i32))"#,
        r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
    ];
    for import in cases {
        let error = compile_wat(
            &module_wat(import, "", "i32.const 0"),
            declaration(&[], &[Hook::Input]),
        )
        .unwrap_err();
        assert!(
            matches!(error, ModuleError::ImportNotAllowed { .. }),
            "{import}: {error:?}"
        );
    }
    let imported_memory = r#"(module (import "msime" "memory" (memory 1))
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert!(matches!(
        compile_wat(imported_memory, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingMemory | ModuleError::ImportNotAllowed { .. }
    ));
    let wrong_signature = module_wat(
        r#"(import "msime" "event_scalar" (func (param i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&wrong_signature, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::ImportSignature("event_scalar")
    );
}

#[test]
fn an_import_needs_its_capability() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&wat, declaration(&[], &[Hook::Commit])).unwrap_err(),
        ModuleError::ImportWithoutCapability {
            import: "commit_text",
            capability: Capability::ReadCommit,
        }
    );
    compile_wat(
        &wat,
        declaration(&[Capability::ReadCommit], &[Hook::Commit]),
    )
    .expect("declared capability admits the import");
}

#[test]
fn a_capability_needs_an_import() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(
            &wat,
            declaration(
                &[Capability::ReadCommit, Capability::ReadCandidates],
                &[Hook::Commit]
            )
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::ReadCandidates)
    );
    // 修改类能力同样要导入它自己的函数，读能力的 import 替它不了。
    assert_eq!(
        compile_wat(
            &wat,
            declaration(
                &[Capability::ReadCommit, Capability::RewriteCommit],
                &[Hook::BeforeCommit]
            )
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::RewriteCommit)
    );
}

#[test]
fn capability_rules_hold() {
    let wat = module_wat(
        r#"(import "msime" "rewrite_commit" (func (param i32 i32) (result i32)))
           (import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))
           (import "msime" "composition_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert!(matches!(
        compile_wat(
            &wat,
            declaration(
                &[
                    Capability::ReadCommit,
                    Capability::RewriteCommit,
                    Capability::ReadComposition
                ],
                &[Hook::BeforeCommit]
            )
        )
        .unwrap_err(),
        ModuleError::Capabilities(CapabilityRuleError::Exclusive { .. })
    ));
    let rerank_alone: CapabilitySet = [Capability::RerankCandidates].into_iter().collect();
    assert_eq!(
        rerank_alone.check_rules(),
        Err(CapabilityRuleError::MissingDependency {
            capability: Capability::RerankCandidates,
            requires: Capability::ReadCandidates,
        })
    );
    for capability in Capability::ALL {
        assert_eq!(Capability::parse(capability.as_str()), Some(capability));
    }
    for hook in Hook::ALL {
        assert_eq!(Hook::parse(hook.as_str()), Some(hook));
    }
    assert_eq!(Hook::parse("key_down"), None);
}

// ---- 执行限制 ----

#[test]
fn an_infinite_loop_runs_out_of_its_hook_fuel() {
    let mut script = instance(
        &module_wat("", "", "(loop $spin (br $spin)) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        script.deliver(1, &input_event()),
        Outcome::Failed {
            failure: Failure::OutOfFuel,
            fuel: limits::OBSERVE_FUEL,
        }
    );
    assert!(!script.is_usable());
    assert_eq!(
        failure(script.deliver(2, &input_event())),
        Failure::Unusable
    );
}

#[test]
fn unreachable_traps() {
    let mut script = instance(
        &module_wat("", "", "unreachable"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::Unreachable
    );
    assert!(!script.is_usable());
}

#[test]
fn out_of_bounds_access_traps() {
    let mut script = instance(
        &module_wat("", "", "(i32.load (i32.const 0x7fff_fff0))"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn memory_is_reserved_up_front_and_cannot_grow() {
    let size = instance(
        &module_wat("", "", "memory.size"),
        declaration(&[], &[Hook::Input]),
    )
    .deliver(1, &input_event());
    assert_eq!(status_of(size), limits::DEFAULT_MEMORY_PAGES as i32);

    let mut grower = instance(
        &module_wat("", "", "(drop (memory.grow (i32.const 1))) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(grower.deliver(1, &input_event())),
        Failure::MemoryLimit
    );
}

#[test]
fn a_smaller_declared_maximum_caps_the_reservation() {
    let wat = r#"(module (memory (export "memory") 1 4)
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) memory.size))"#;
    let module = compile_wat(wat, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(module.reserved_memory_bytes(), 4 * limits::PAGE_BYTES);
    let mut script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
    assert_eq!(status_of(script.deliver(1, &input_event())), 4);
}

#[test]
fn deep_recursion_hits_the_stack_limit() {
    let mut script = instance(
        &module_wat(
            "",
            "(func $down (param i32) (result i32) (call $down (i32.add (local.get 0) (i32.const 1))))",
            "(call $down (i32.const 0))",
        ),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::StackOverflow
    );
}

#[test]
fn a_negative_return_is_a_failure_but_keeps_the_instance() {
    let mut script = instance(
        &module_wat("", "", "i32.const -1"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::NegativeReturn
    );
    assert!(script.is_usable());
}

#[test]
fn undeclared_hooks_are_not_delivered() {
    let mut script = instance(
        &module_wat("", "", "unreachable"),
        declaration(&[], &[Hook::Commit]),
    );
    assert_eq!(script.deliver(1, &input_event()), Outcome::NotSubscribed);
    assert_eq!(script.turn_fuel_left(), limits::TURN_FUEL);
}

#[test]
fn initialisation_failures() {
    let wrong_abi = r#"(module (memory (export "memory") 1)
        (func (export "msime_abi") (result i32) i32.const 2)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    let module = compile_wat(wrong_abi, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL)
            .err()
            .map(|error| error.failure),
        Some(Failure::AbiMismatch)
    );

    let failing_init = module_wat(
        "",
        r#"(func (export "msime_init") (result i32) i32.const -5)"#,
        "i32.const 0",
    );
    let module = compile_wat(&failing_init, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL)
            .err()
            .map(|error| error.failure),
        Some(Failure::InitFailed)
    );

    // 初始化同样受 fuel 约束，失败时报告用掉的额度，供宿主计入 focus 的合计。
    let spinning_init = module_wat(
        "",
        r#"(func (export "msime_init") (result i32) (loop $spin (br $spin)) i32.const 0)"#,
        "i32.const 0",
    );
    let module = compile_wat(&spinning_init, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, 40_000).err(),
        Some(InitFailure {
            failure: Failure::OutOfFuel,
            fuel: 40_000,
        })
    );
}

// ---- 配额 ----

/// 每次投递做 `iterations` 次循环、返回 0 的模块。
fn burner(iterations: u32) -> String {
    module_wat(
        "",
        "",
        &format!(
            "(local $i i32)
             (local.set $i (i32.const {iterations}))
             (loop $again
                (local.set $i (i32.sub (local.get $i) (i32.const 1)))
                (br_if $again (local.get $i)))
             i32.const 0"
        ),
    )
}

#[test]
fn the_turn_quota_spans_deliveries_and_refills_on_the_next_turn() {
    let mut script = instance(&burner(2_000), declaration(&[], &[Hook::Input]));
    let mut spent = 0;
    let mut deliveries = 0;
    loop {
        match script.deliver(7, &input_event()) {
            Outcome::Delivered { fuel, .. } => {
                assert!(fuel <= limits::OBSERVE_FUEL);
                spent += fuel;
                deliveries += 1;
            }
            Outcome::Failed { failure, fuel } => {
                assert_eq!(failure, Failure::OutOfFuel);
                spent += fuel;
                break;
            }
            Outcome::NotSubscribed => unreachable!(),
        }
        assert!(deliveries < 1_000, "the turn quota never ran out");
    }
    assert!(deliveries > 1);
    assert!(spent <= limits::TURN_FUEL);
    // 撞上本轮配额的那一次 trap 了，实例要换新的；新实例在新一轮里重新拿到完整配额。
    let mut fresh = instance(&burner(2_000), declaration(&[], &[Hook::Input]));
    assert!(matches!(
        fresh.deliver(8, &input_event()),
        Outcome::Delivered { .. }
    ));
    assert!(fresh.turn_fuel_left() < limits::TURN_FUEL);
    assert!(matches!(
        fresh.deliver(9, &input_event()),
        Outcome::Delivered { .. }
    ));
    assert!(fresh.turn_fuel_left() > limits::TURN_FUEL - limits::OBSERVE_FUEL);
}

#[test]
fn one_plugin_burning_its_quota_does_not_touch_another() {
    let mut greedy = instance(
        &module_wat("", "", "(loop $spin (br $spin)) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    let mut modest = instance(&burner(100), declaration(&[], &[Hook::Input]));
    let alone = match modest.deliver(1, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        failure(greedy.deliver(2, &input_event())),
        Failure::OutOfFuel
    );
    assert_eq!(
        modest.deliver(2, &input_event()),
        Outcome::Delivered {
            status: 0,
            fuel: alone,
        }
    );
    assert_eq!(modest.turn_fuel_left(), limits::TURN_FUEL - alone);
}

#[test]
fn imports_cost_fuel() {
    let plain = instance(
        &module_wat("", "", "i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    let calling = instance(
        &module_wat(
            r#"(import "msime" "event_scalar" (func $scalar (param i32) (result i64)))"#,
            "",
            "(drop (call $scalar (i32.const 0))) i32.const 0",
        ),
        declaration(&[], &[Hook::Input]),
    );
    let fuel = |mut script: ScriptInstance| match script.deliver(1, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert!(fuel(calling) >= fuel(plain) + limits::IMPORT_CALL_FUEL);
}

// ---- 快照复位 ----

const SNAPSHOT_WAT: &str = r#"(module
    (memory (export "memory") 1)
    (global $exported (export "counter") (mut i32) (i32.const 1))
    (global $hidden (mut i32) (i32.const 1))
    (data (i32.const 200) "abc")
    (func (export "msime_abi") (result i32) i32.const 1)
    (func (export "msime_init") (result i32)
        (i32.store8 (i32.const 100) (i32.const 7))
        (i32.store8 (i32.const 200) (i32.const 0))
        (i32.store8 (i32.const 201) (i32.const 0))
        (i32.store8 (i32.const 202) (i32.const 0))
        (global.set $exported (i32.const 5))
        (global.set $hidden (i32.const 5))
        i32.const 0)
    (func (export "msime_on_event") (param $kind i32) (result i32)
        (if (i32.eq (local.get $kind) (i32.const 9))
            (then
                ;; before_commit：把看到的状态编码进返回值，再弄脏它。
                (i32.add
                    (i32.add
                        (i32.mul (i32.load8_u (i32.const 100)) (i32.const 1000))
                        (i32.mul (global.get $exported) (i32.const 100)))
                    (i32.add
                        (i32.mul (global.get $hidden) (i32.const 10))
                        (i32.load8_u (i32.const 200))))
                (i32.store8 (i32.const 100) (i32.const 55))
                (global.set $exported (i32.const 8))
                (global.set $hidden (i32.const 8))
                return))
        ;; 其余事件：在持久实例上留下状态，返回之前的内存值。
        (i32.load8_u (i32.const 100))
        (i32.store8 (i32.const 100) (i32.const 99))
        (i32.store8 (i32.const 200) (i32.const 3))
        (global.set $exported (i32.const 9))
        (global.set $hidden (i32.const 9))))"#;

#[test]
fn before_commit_runs_on_the_post_init_snapshot() {
    let mut script = instance(
        SNAPSHOT_WAT,
        declaration(&[], &[Hook::Commit, Hook::BeforeCommit]),
    );
    let before_commit = Event::new(Hook::BeforeCommit);
    let commit = Event::new(Hook::Commit);
    // 内存 [100] 是 init 写的 7，导出的 global 是 init 设的 5，未导出的 global 回到声明的初值 1，数据段被 init 清掉的 [200] 仍是 0。
    let expected = 7 * 1000 + 5 * 100 + 10;
    assert_eq!(status_of(script.deliver(1, &before_commit)), expected);
    // 观察类事件改了持久实例：它们的状态进不了下一次 before_commit。
    assert_eq!(status_of(script.deliver(2, &commit)), 7);
    assert_eq!(status_of(script.deliver(3, &before_commit)), expected);
    // before_commit 自己的改动也不留下，持久实例看到的仍是它自己上次写的 99。
    assert_eq!(status_of(script.deliver(4, &commit)), 99);
    assert_eq!(status_of(script.deliver(5, &before_commit)), expected);
}

#[test]
fn a_trap_on_the_snapshot_keeps_the_persistent_instance() {
    let wat = module_wat(
        "",
        "",
        "(if (i32.eq (local.get $kind) (i32.const 9)) (then unreachable)) i32.const 0",
    );
    let mut script = instance(&wat, declaration(&[], &[Hook::Commit, Hook::BeforeCommit]));
    assert_eq!(
        failure(script.deliver(1, &Event::new(Hook::BeforeCommit))),
        Failure::Unreachable
    );
    assert!(script.is_usable());
    assert_eq!(status_of(script.deliver(1, &Event::new(Hook::Commit))), 0);
}

// ---- import ----

const READER_IMPORTS: &str = r#"
    (import "msime" "event_scalar" (func $event (param i32) (result i64)))
    (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
    (import "msime" "commit_scalar" (func $commit_scalar (param i32) (result i64)))
    (import "msime" "composition_text" (func $composition_text (param i32 i32 i32) (result i32)))
    (import "msime" "candidate_count" (func $count (result i32)))
    (import "msime" "candidate_text" (func $candidate_text (param i32 i32 i32) (result i32)))
    (import "msime" "candidate_meta" (func $candidate_meta (param i32 i32) (result i64)))
    (import "msime" "log" (func $log (param i32 i32 i32)))
"#;

/// 由导出的 global `$probe` 选择做哪一项探测，结果写进 [0..) 并作为返回值。
fn reader(probe_body: &str) -> String {
    format!(
        r#"(module
            {READER_IMPORTS}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            (func (export "msime_on_event") (param $kind i32) (result i32) {probe_body}))"#
    )
}

const READS: [Capability; 3] = [
    Capability::ReadComposition,
    Capability::ReadCommit,
    Capability::ReadCandidates,
];

fn reader_instance(probe_body: &str, grants: &[Capability]) -> ScriptInstance {
    let module =
        compile_wat(&reader(probe_body), declaration(&READS, &Hook::ALL)).expect("reader compiles");
    ScriptInstance::new(&module, grants.iter().copied().collect(), limits::INIT_FUEL).unwrap()
}

fn commit_event(text: &'static str) -> Event<'static> {
    let mut event = Event::new(Hook::Commit);
    event.scalars.local_mode = Some(0);
    event.scalars.origin = Some(0);
    event.commit.commit = Some(text);
    event.commit.body = Some(text);
    event.commit.seat = Some(2);
    event
}

#[test]
fn event_scalars_need_no_capability() {
    let mut script = reader_instance(
        &format!(
            "(i32.wrap_i64 (i64.add (i64.mul (call $event (i32.const {})) (i64.const 100)) (call $event (i32.const {}))))",
            event_field::KIND,
            event_field::ORIGIN
        ),
        &[],
    );
    assert_eq!(
        status_of(script.deliver(1, &commit_event("你好"))),
        Hook::Commit.code() * 100
    );
    let mut absent = reader_instance(
        &format!(
            "(i32.wrap_i64 (call $event (i32.const {})))",
            event_field::HOW
        ),
        &[],
    );
    assert_eq!(
        failure(absent.deliver(1, &commit_event("你好"))),
        Failure::NegativeReturn
    );
    let mut invalid = reader_instance(
        "(i32.wrap_i64 (i64.add (call $event (i32.const 99)) (i64.const 100)))",
        &[],
    );
    assert_eq!(
        status_of(invalid.deliver(1, &commit_event("你好"))),
        100 + status::INVALID
    );
}

/// 把 commit 文本复制到 [16..) 后返回 `commit_text` 的返回值。
fn copy_commit(cap: i32) -> String {
    format!(
        "(call $commit_text (i32.const {}) (i32.const 16) (i32.const {cap}))",
        commit_field::COMMIT
    )
}

#[test]
fn text_imports_copy_only_what_is_granted() {
    let text = "你好，世界";
    let mut granted = reader_instance(&copy_commit(64), &[Capability::ReadCommit]);
    assert_eq!(
        status_of(granted.deliver(1, &commit_event(text))),
        text.len() as i32
    );
    // 授权里没有 read_commit：DENIED。
    let mut ungranted = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(64)),
        &[Capability::ReadComposition],
    );
    assert_eq!(
        status_of(ungranted.deliver(1, &commit_event(text))),
        -status::DENIED
    );
    // 屏蔽名单里的本地模式（提及）：与没有这个字段一样是 ABSENT。
    let mut blocked = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(64)),
        &[Capability::ReadCommit],
    );
    let mut mention = commit_event(text);
    mention.scalars.local_mode = Some(6);
    assert_eq!(status_of(blocked.deliver(1, &mention)), -status::ABSENT);
    let mut unknown_mode = commit_event(text);
    unknown_mode.scalars.local_mode = None;
    assert_eq!(
        status_of(blocked.deliver(2, &unknown_mode)),
        -status::ABSENT
    );
    // 缓冲区太小：LIMIT，什么都不复制。
    let mut small = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(4)),
        &[Capability::ReadCommit],
    );
    assert_eq!(
        status_of(small.deliver(1, &commit_event(text))),
        -status::LIMIT
    );
}

#[test]
fn copied_text_lands_in_linear_memory() {
    // 复制后把第一个字节和长度一起编码返回：长度 * 1000 + 首字节。
    let body = format!(
        "(i32.add (i32.mul {} (i32.const 1000)) (i32.load8_u (i32.const 16)))",
        copy_commit(64)
    );
    let mut script = reader_instance(&body, &[Capability::ReadCommit]);
    assert_eq!(
        status_of(script.deliver(1, &commit_event("abc"))),
        3 * 1000 + i32::from(b'a')
    );
    let mut seat = reader_instance(
        &format!(
            "(i32.wrap_i64 (call $commit_scalar (i32.const {})))",
            commit_field::SEAT
        ),
        &[Capability::ReadCommit],
    );
    assert_eq!(status_of(seat.deliver(1, &commit_event("abc"))), 2);
}

#[test]
fn an_out_of_bounds_destination_traps() {
    let mut script = reader_instance(
        &format!(
            "(call $commit_text (i32.const {}) (i32.const 65530) (i32.const 64))",
            commit_field::COMMIT
        ),
        &[Capability::ReadCommit],
    );
    // 16 页预留之内是合法的；越过预留才 trap。
    assert!(matches!(
        script.deliver(1, &commit_event("abc")),
        Outcome::Delivered { .. }
    ));
    let mut beyond = reader_instance(
        &format!(
            "(call $commit_text (i32.const {}) (i32.const 0x00ff_fff0) (i32.const 64))",
            commit_field::COMMIT
        ),
        &[Capability::ReadCommit],
    );
    assert_eq!(
        failure(beyond.deliver(1, &commit_event("abc"))),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn composition_and_candidate_reads() {
    let mut composition = reader_instance(
        &format!(
            "(call $composition_text (i32.const {}) (i32.const 0) (i32.const 64))",
            composition_field::PREEDIT
        ),
        &[Capability::ReadComposition],
    );
    let mut update = Event::new(Hook::CompositionUpdate);
    update.scalars.local_mode = Some(0);
    update.composition.preedit = Some("ni hao");
    assert_eq!(status_of(composition.deliver(1, &update)), 6);

    let rows = [
        CandidateRow {
            text: "你好",
            code: "nihao",
            annotation: "",
            source: 0,
        },
        CandidateRow {
            text: "拟好",
            code: "nihao",
            annotation: "",
            source: 3,
        },
    ];
    let mut shown = Event::new(Hook::CandidatesShown);
    shown.scalars.local_mode = Some(0);
    shown.candidates = Some(Candidates {
        rows: &rows,
        page: Some(0),
        highlighted: Some(1),
    });
    let mut candidates = reader_instance(
        &format!(
            "(i32.add (i32.mul (call $count) (i32.const 100))
                (i32.add
                    (i32.mul (i32.wrap_i64 (call $candidate_meta (i32.const 1) (i32.const {}))) (i32.const 10))
                    (i32.wrap_i64 (call $candidate_meta (i32.const 0) (i32.const {})))))",
            candidate_field::SOURCE,
            candidate_field::HIGHLIGHTED
        ),
        &[Capability::ReadCandidates],
    );
    assert_eq!(
        status_of(candidates.deliver(1, &shown)),
        2 * 100 + 3 * 10 + 1
    );
    let mut text = reader_instance(
        "(call $candidate_text (i32.const 1) (i32.const 0) (i32.const 64))",
        &[Capability::ReadCandidates],
    );
    assert_eq!(status_of(text.deliver(1, &shown)), "拟好".len() as i32);
    let mut missing_row = reader_instance(
        "(i32.sub (i32.const 0) (call $candidate_text (i32.const 2) (i32.const 0) (i32.const 64)))",
        &[Capability::ReadCandidates],
    );
    assert_eq!(status_of(missing_row.deliver(1, &shown)), -status::ABSENT);
}

#[test]
fn grants_beyond_the_declaration_are_ignored() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    let module = compile_wat(
        &wat,
        declaration(&[Capability::ReadCommit], &[Hook::Commit]),
    )
    .unwrap();
    let all: CapabilitySet = Capability::ALL.into_iter().collect();
    let script = ScriptInstance::new(&module, all, limits::INIT_FUEL).unwrap();
    assert_eq!(
        script.grants(),
        CapabilitySet::EMPTY.with(Capability::ReadCommit)
    );
}

#[test]
fn modification_imports_are_denied_in_this_phase() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))
           (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))"#,
        "",
        "(i32.sub (i32.const 0) (call $rewrite (i32.const 0) (i32.const 3)))",
    );
    let declared = declaration(
        &[Capability::ReadCommit, Capability::RewriteCommit],
        &[Hook::BeforeCommit],
    );
    let module = compile_wat(&wat, declared).unwrap();
    let mut script =
        ScriptInstance::new(&module, declared.capabilities, limits::INIT_FUEL).unwrap();
    let mut event = commit_event("abc");
    event.hook = Hook::BeforeCommit;
    assert_eq!(status_of(script.deliver(1, &event)), -status::DENIED);
}

#[test]
fn init_sees_no_event_and_texts_do_not_outlive_the_call() {
    // init 把 commit_text 的返回值存到 [0]；事件里返回它。
    let wat = format!(
        r#"(module
            {READER_IMPORTS}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            (func (export "msime_init") (result i32)
                (i32.store (i32.const 0)
                    (call $commit_text (i32.const {commit}) (i32.const 16) (i32.const 64)))
                i32.const 0)
            (func (export "msime_on_event") (param $kind i32) (result i32)
                (if (i32.eq (local.get $kind) (i32.const {focus_in}))
                    (then (return (i32.sub (i32.const 0)
                        (call $commit_text (i32.const {commit}) (i32.const 16) (i32.const 64))))))
                (i32.sub (i32.const 0) (i32.load (i32.const 0)))))"#,
        commit = commit_field::COMMIT,
        focus_in = Hook::FocusIn.code(),
    );
    let module = compile_wat(&wat, declaration(&READS, &Hook::ALL)).unwrap();
    let mut script = ScriptInstance::new(
        &module,
        CapabilitySet::EMPTY.with(Capability::ReadCommit),
        limits::INIT_FUEL,
    )
    .unwrap();
    assert_eq!(
        status_of(script.deliver(1, &commit_event("abc"))),
        -status::ABSENT
    );
    // 上一次 commit 的文本不会留到下一个事件里。
    let mut focus = Event::new(Hook::FocusIn);
    focus.scalars.local_mode = Some(0);
    assert_eq!(status_of(script.deliver(2, &focus)), -status::ABSENT);
}

#[test]
fn log_is_a_no_op_without_a_sink() {
    let wat = reader(
        r#"(i32.store8 (i32.const 0) (i32.const 104))
           (i32.store8 (i32.const 1) (i32.const 105))
           (call $log (i32.const 1) (i32.const 0) (i32.const 2))
           i32.const 0"#,
    );
    let module = compile_wat(&wat, declaration(&READS, &Hook::ALL)).unwrap();
    let mut script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
    assert_eq!(status_of(script.deliver(1, &input_event())), 0);

    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);
    script.set_log_sink(Some(Box::new(move |level, text| {
        sink_lines.lock().unwrap().push((level, text.to_owned()));
    })));
    assert_eq!(status_of(script.deliver(2, &input_event())), 0);
    assert_eq!(*lines.lock().unwrap(), vec![(1, "hi".to_owned())]);
}

#[test]
fn the_text_mode_whitelist_matches_u6() {
    // none、临时英文、临时日文、超级简拼、emoji、颜文字可用；其余（含不认识的 255）屏蔽。
    for mode in 0..=5 {
        assert!(abi::local_mode_allows_text(mode));
    }
    for mode in [6, 7, 8, 9, 10, 11, 12, 255] {
        assert!(!abi::local_mode_allows_text(mode));
    }
}

// ---- 测量（默认不跑）：`cargo test -p msime-script-runtime -- --ignored --nocapture measure` ----

mod measure {
    use super::*;
    use std::time::Instant;

    /// 手工拼一个 `bytes` 字节左右的二进制模块：`functions` 个相同的函数体。这只是测量用的夹具，不是解析器：没有 `wat` crate 的直接依赖，WAT 文本又会把解析耗时混进编译耗时。
    fn synthetic_module(bytes: usize) -> Vec<u8> {
        fn leb(mut value: u32, out: &mut Vec<u8>) {
            loop {
                let byte = (value & 0x7f) as u8;
                value >>= 7;
                if value == 0 {
                    out.push(byte);
                    return;
                }
                out.push(byte | 0x80);
            }
        }
        fn section(id: u8, body: &[u8], out: &mut Vec<u8>) {
            out.push(id);
            leb(body.len() as u32, out);
            out.extend_from_slice(body);
        }
        fn name(text: &str, out: &mut Vec<u8>) {
            leb(text.len() as u32, out);
            out.extend_from_slice(text.as_bytes());
        }
        // 一个 i32 局部变量；循环体是 `local.get 0; i32.load; local.get 0; i32.add; local.set 0`，依赖内存和局部变量，翻译时折叠不掉。
        let mut body = vec![0x01, 0x01, 0x7f];
        while body.len() < 1_000 {
            body.extend_from_slice(&[0x20, 0x00, 0x28, 0x02, 0x00, 0x20, 0x00, 0x6a, 0x21, 0x00]);
        }
        body.extend_from_slice(&[0x20, 0x00, 0x0b]); // local.get 0; end
        let functions = (bytes / (body.len() + 2)).max(2) as u32;

        let mut out = b"\0asm\x01\0\0\0".to_vec();
        // 类型 0：() -> i32；类型 1：(i32) -> i32。
        section(
            1,
            &[0x02, 0x60, 0x00, 0x01, 0x7f, 0x60, 0x01, 0x7f, 0x01, 0x7f],
            &mut out,
        );
        let mut funcs = Vec::new();
        leb(functions, &mut funcs);
        funcs.push(0x00); // msime_abi
        funcs.push(0x01); // msime_on_event
        funcs.extend(std::iter::repeat_n(0x00, functions as usize - 2));
        section(3, &funcs, &mut out);
        section(5, &[0x01, 0x00, 0x01], &mut out); // 一块内存，最小 1 页
        let mut exports = vec![0x03];
        name("memory", &mut exports);
        exports.extend_from_slice(&[0x02, 0x00]);
        name("msime_abi", &mut exports);
        exports.extend_from_slice(&[0x00, 0x00]);
        name("msime_on_event", &mut exports);
        exports.extend_from_slice(&[0x00, 0x01]);
        section(7, &exports, &mut out);
        let mut code = Vec::new();
        leb(functions, &mut code);
        let abi_body = [0x00, 0x41, 0x01, 0x0b];
        leb(abi_body.len() as u32, &mut code);
        code.extend_from_slice(&abi_body);
        for _ in 1..functions {
            leb(body.len() as u32, &mut code);
            code.extend_from_slice(&body);
        }
        section(10, &code, &mut out);
        out
    }

    #[test]
    #[ignore]
    fn measure_compile_and_instantiate_a_512_kib_module() {
        let wasm = synthetic_module(512 * 1024);
        let declared = declaration(&[], &[Hook::Input]);
        let started = Instant::now();
        let before = counting::net();
        let module = compile(&wasm, declared).expect("synthetic module compiles");
        let retained = counting::net() - before;
        let compiled = started.elapsed();
        let started = Instant::now();
        let script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
        let instantiated = started.elapsed();
        println!(
            "module {} bytes: validate + eager compile {compiled:?}, instantiate {instantiated:?}, retained {} bytes ({:.2}x)",
            wasm.len(),
            retained,
            retained as f64 / wasm.len() as f64
        );
        drop(script);
    }

    #[test]
    #[ignore]
    fn measure_a_minimal_hook() {
        let mut script = instance(
            &module_wat("", "", "i32.const 0"),
            declaration(&[], &[Hook::Input]),
        );
        let event = input_event();
        for turn in 0..1_000 {
            script.deliver(turn, &event);
        }
        let rounds = 1_000_000u64;
        let started = Instant::now();
        for turn in 0..rounds {
            script.deliver(turn, &event);
        }
        println!(
            "minimal hook: {:?} per delivery",
            started.elapsed() / rounds as u32
        );
    }

    #[test]
    #[ignore]
    fn measure_snapshot_reset() {
        for pages in [16u32, 64] {
            let wat = format!(
                r#"(module (memory (export "memory") {pages})
                    (func (export "msime_abi") (result i32) i32.const 1)
                    (func (export "msime_init") (result i32)
                        (memory.fill (i32.const 0) (i32.const 1) (i32.const {bytes}))
                        i32.const 0)
                    (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
                bytes = pages as usize * limits::PAGE_BYTES
            );
            let mut declared = declaration(&[], &[Hook::BeforeCommit]);
            declared.memory_pages = pages;
            let module = compile_wat(&wat, declared).unwrap();
            let mut script =
                ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
            let event = Event::new(Hook::BeforeCommit);
            for turn in 0..20 {
                script.deliver(turn, &event);
            }
            let rounds = 200u64;
            let started = Instant::now();
            for turn in 0..rounds {
                assert!(matches!(
                    script.deliver(turn, &event),
                    Outcome::Delivered { .. }
                ));
            }
            println!(
                "snapshot reset, {pages} pages fully dirty: {:?} per before_commit",
                started.elapsed() / rounds as u32
            );
        }
    }
}

/// 只数当前线程的计数分配器，量编译后留下的字节数。别的测试在别的线程上分配，不影响读数。
#[allow(unsafe_code)]
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static NET: Cell<isize> = const { Cell::new(0) };
    }

    struct Counting;

    // SAFETY: 原样转交给系统分配器，只额外记一个线程局部的计数。
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = NET.try_with(|net| net.set(net.get() + layout.size() as isize));
            // SAFETY: 调用方满足 `GlobalAlloc::alloc` 的约定，原样转交。
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let _ = NET.try_with(|net| net.set(net.get() - layout.size() as isize));
            // SAFETY: `ptr` 来自上面的 `alloc`，即系统分配器。
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let _ =
                NET.try_with(|net| net.set(net.get() + new_size as isize - layout.size() as isize));
            // SAFETY: 同 `dealloc`。
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// 当前线程到目前为止净分配的字节数。
    pub fn net() -> isize {
        NET.with(Cell::get)
    }
}
