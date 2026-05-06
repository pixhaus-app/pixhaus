//! Benchmarks for [`VerbRuntime`] dispatch overhead.
//!
//! Target: verb dispatch (invoke → finish) overhead under 1ms for a
//! no-op verb with no backend. Measured here as pure runtime cost,
//! excluding any backend I/O.
//!
//! Run with:
//! ```text
//! cargo bench -p pixhaus-ai
//! ```
//!
//! Criterion generates an HTML report in `target/criterion/runtime/`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::disallowed_methods,
    missing_docs
)]

use std::any::Any;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use pixhaus_ai::plugin::{
    BackendCapabilities, CostEstimate, EchoVerb, EffectKind, InferenceBackend, PixelData,
    VerbContext, VerbDescriptor, VerbId, VerbInputs, VerbOutput, VerbProgress, VerbRuntime,
};
use pixhaus_core::project::{FrameIndex, ProjectMetadata, SpriteId};
use tokio::runtime::Runtime;

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "bench".into(),
        description: None,
        author: None,
        created_at: 0,
        updated_at: 0,
        editor_version: env!("CARGO_PKG_VERSION").into(),
    }
}

fn ctx_with_sprite() -> VerbContext {
    VerbContext::builder(metadata())
        .with_active_sprite(SpriteId::new(1))
        .with_active_frame(FrameIndex::new(0))
        .build()
}

/// A minimal no-op verb that returns immediately without touching any
/// backend. Isolates pure dispatch overhead.
struct NoopVerb {
    descriptor: VerbDescriptor,
}

impl NoopVerb {
    fn new(id: &str, caps: BackendCapabilities) -> Self {
        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(id),
                display_name: id.into(),
                description: String::new(),
                version: "0.0.1".into(),
                required_capabilities: caps,
                input_schema: serde_json::json!({}),
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer],
                cost_estimate: CostEstimate::free(),
                streaming: false,
                cancellable: false,
                documentation_url: None,
            },
        }
    }
}

#[async_trait::async_trait]
impl pixhaus_ai::plugin::Verb for NoopVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    async fn invoke(
        &self,
        _ctx: VerbContext,
        _inputs: VerbInputs,
        _progress: VerbProgress,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> pixhaus_ai::plugin::Result<VerbOutput> {
        use pixhaus_ai::plugin::ActualCost;
        use std::time::Duration;

        Ok(VerbOutput {
            summary: "noop".into(),
            effects: vec![],
            thumbnail: None,
            actual_cost: ActualCost::free(Duration::ZERO),
            notes: vec![],
        })
    }
}

/// Stub backend for benchmarking the capability-check path.
#[derive(Debug)]
struct StubBackend {
    id: &'static str,
    caps: BackendCapabilities,
}

impl InferenceBackend for StubBackend {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn id(&self) -> &'static str {
        self.id
    }
    fn capabilities(&self) -> BackendCapabilities {
        self.caps
    }
    fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
        CostEstimate::free()
    }
}

// ── Benchmarks ──────────────────────────────────────────────────────────────

/// Measures the overhead of `invoke → finish` for a no-op verb with no
/// backend requirement (no capability check).
///
/// `VerbId` is hoisted outside the loop and `iter_batched` constructs
/// `VerbContext` + `VerbInputs` in the unmeasured setup phase, so the
/// reported time reflects only the runtime's invoke+finish path — not
/// the cost of building the call-site arguments.
fn bench_dispatch_no_backend(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let runtime = VerbRuntime::new();
    runtime
        .register(NoopVerb::new("noop", BackendCapabilities::empty()))
        .unwrap();
    let id = VerbId::new("noop");

    c.bench_function("dispatch/no_backend", |b| {
        b.iter_batched(
            || (ctx_with_sprite(), VerbInputs::empty()),
            |(ctx, inputs)| {
                rt.block_on(async {
                    let inv = runtime.invoke(&id, ctx, inputs).unwrap();
                    inv.finish().await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });
}

/// Measures `invoke → finish` when a backend is registered and
/// capability selection is required. Same hoisting rationale as
/// `bench_dispatch_no_backend`.
fn bench_dispatch_with_backend(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let runtime = VerbRuntime::new();
    runtime
        .register_backend(
            StubBackend {
                id: "stub",
                caps: BackendCapabilities::TEXT_GENERATION,
            },
            0,
        )
        .unwrap();
    runtime
        .register(NoopVerb::new(
            "needs-text",
            BackendCapabilities::TEXT_GENERATION,
        ))
        .unwrap();
    let id = VerbId::new("needs-text");

    c.bench_function("dispatch/with_backend", |b| {
        b.iter_batched(
            || (ctx_with_sprite(), VerbInputs::empty()),
            |(ctx, inputs)| {
                rt.block_on(async {
                    let inv = runtime.invoke(&id, ctx, inputs).unwrap();
                    inv.finish().await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });
}

/// Measures `EchoVerb` end-to-end — includes pixel buffer allocation and
/// progress events — to give a realistic lower bound on verb latency.
fn bench_echo_verb(c: &mut Criterion) {
    use pixhaus_ai::plugin::EchoInputs;

    let rt = Runtime::new().unwrap();
    let runtime = VerbRuntime::new();
    runtime.register(EchoVerb::new()).unwrap();

    let inputs = VerbInputs::from_struct(&EchoInputs {
        pixels: PixelData::rgba8(32, 32, vec![128u8; 32 * 32 * 4]),
        layer_name: Some("bench".into()),
    })
    .unwrap();

    c.bench_function("dispatch/echo_32x32", |b| {
        b.iter(|| {
            rt.block_on(async {
                let inv = runtime
                    .invoke(
                        &VerbId::new(pixhaus_ai::plugin::ECHO_VERB_ID),
                        ctx_with_sprite(),
                        inputs.clone(),
                    )
                    .unwrap();
                let preview = inv.finish().await.unwrap();
                // Consume the effects so the compiler can't elide the work.
                let _ = std::hint::black_box(&preview.output.effects);
            });
        });
    });
}

/// Benchmarks backend selection across various registry sizes to show
/// that linear search on a small (<10) list is O(n) but fast.
fn bench_select_backend(c: &mut Criterion) {
    let mut group = c.benchmark_group("select_backend");

    for n_backends in [1usize, 5, 10] {
        let runtime = VerbRuntime::new();
        for i in 0..n_backends {
            runtime
                .register_backend(
                    StubBackend {
                        id: Box::leak(format!("b{i}").into_boxed_str()),
                        caps: BackendCapabilities::TEXT_GENERATION,
                    },
                    u16::try_from(i).unwrap(),
                )
                .unwrap();
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(n_backends),
            &n_backends,
            |b, _| {
                b.iter(|| {
                    let _ =
                        std::hint::black_box(runtime.select_backend(
                            BackendCapabilities::TEXT_GENERATION,
                            &VerbId::new("v"),
                        ));
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_dispatch_no_backend,
    bench_dispatch_with_backend,
    bench_echo_verb,
    bench_select_backend,
);
criterion_main!(benches);
