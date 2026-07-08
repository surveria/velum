use super::JetStreamCase;

macro_rules! file {
    ($path:literal) => {
        concat!("tests/external/jetstream/", $path)
    };
}

const CASES: &[JetStreamCase] = &[
    JetStreamCase::timed(
        "Air",
        &[
            file!("ARES-6/Air/symbols.js"),
            file!("ARES-6/Air/tmp_base.js"),
            file!("ARES-6/Air/arg.js"),
            file!("ARES-6/Air/basic_block.js"),
            file!("ARES-6/Air/code.js"),
            file!("ARES-6/Air/frequented_block.js"),
            file!("ARES-6/Air/inst.js"),
            file!("ARES-6/Air/opcode.js"),
            file!("ARES-6/Air/reg.js"),
            file!("ARES-6/Air/stack_slot.js"),
            file!("ARES-6/Air/tmp.js"),
            file!("ARES-6/Air/util.js"),
            file!("ARES-6/Air/custom.js"),
            file!("ARES-6/Air/liveness.js"),
            file!("ARES-6/Air/insertion_set.js"),
            file!("ARES-6/Air/allocate_stack.js"),
            file!("ARES-6/Air/payload-gbemu-executeIteration.js"),
            file!("ARES-6/Air/payload-imaging-gaussian-blur-gaussianBlur.js"),
            file!("ARES-6/Air/payload-airjs-ACLj8C.js"),
            file!("ARES-6/Air/payload-typescript-scanIdentifier.js"),
            file!("ARES-6/Air/benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "Basic",
        &[
            file!("ARES-6/Basic/ast.js"),
            file!("ARES-6/Basic/basic.js"),
            file!("ARES-6/Basic/caseless_map.js"),
            file!("ARES-6/Basic/lexer.js"),
            file!("ARES-6/Basic/number.js"),
            file!("ARES-6/Basic/parser.js"),
            file!("ARES-6/Basic/random.js"),
            file!("ARES-6/Basic/state.js"),
            file!("ARES-6/Basic/benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "ML",
        &[file!("ARES-6/ml/index.js"), file!("ARES-6/ml/benchmark.js")],
    ),
    JetStreamCase::skipped(
        "Babylon",
        "requires async preload blobs and browser-oriented blob loading",
    ),
    JetStreamCase::timed(
        "cdjs",
        &[
            file!("cdjs/constants.js"),
            file!("cdjs/util.js"),
            file!("cdjs/red_black_tree.js"),
            file!("cdjs/call_sign.js"),
            file!("cdjs/vector_2d.js"),
            file!("cdjs/vector_3d.js"),
            file!("cdjs/motion.js"),
            file!("cdjs/reduce_collision_set.js"),
            file!("cdjs/simulator.js"),
            file!("cdjs/collision.js"),
            file!("cdjs/collision_detector.js"),
            file!("cdjs/benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "first-inspector-code-load",
        &[
            file!("code-load/code-first-load.js"),
            file!("code-load/inspector-payload-minified.js"),
        ],
    ),
    JetStreamCase::timed(
        "multi-inspector-code-load",
        &[
            file!("code-load/code-multi-load.js"),
            file!("code-load/inspector-payload-minified.js"),
        ],
    ),
    JetStreamCase::timed("Box2D", &[file!("Octane/box2d.js")]),
    JetStreamCase::timed("octane-code-load", &[file!("Octane/code-first-load.js")]),
    JetStreamCase::timed("crypto", &[file!("Octane/crypto.js")]),
    JetStreamCase::timed("delta-blue", &[file!("Octane/deltablue.js")]),
    JetStreamCase::timed("earley-boyer", &[file!("Octane/earley-boyer.js")]),
    JetStreamCase::timed(
        "gbemu",
        &[
            file!("Octane/gbemu-part1.js"),
            file!("Octane/gbemu-part2.js"),
        ],
    ),
    JetStreamCase::timed("mandreel", &[file!("Octane/mandreel.js")]),
    JetStreamCase::timed("navier-stokes", &[file!("Octane/navier-stokes.js")]),
    JetStreamCase::timed("pdfjs", &[file!("Octane/pdfjs.js")]),
    JetStreamCase::timed("raytrace", &[file!("Octane/raytrace.js")]),
    JetStreamCase::timed("regexp-octane", &[file!("Octane/regexp.js")]),
    JetStreamCase::timed("richards", &[file!("Octane/richards.js")]),
    JetStreamCase::timed("splay", &[file!("Octane/splay.js")]),
    JetStreamCase::timed(
        "typescript-octane",
        &[
            file!("Octane/typescript-compiler.js"),
            file!("Octane/typescript-input.js"),
            file!("Octane/typescript.js"),
        ],
    ),
    JetStreamCase::skipped(
        "FlightPlanner",
        "requires compressed waypoints preload resource not vendored in the shell snapshot",
    ),
    JetStreamCase::timed(
        "OfflineAssembler",
        &[
            file!("RexBench/OfflineAssembler/registers.js"),
            file!("RexBench/OfflineAssembler/instructions.js"),
            file!("RexBench/OfflineAssembler/ast.js"),
            file!("RexBench/OfflineAssembler/parser.js"),
            file!("RexBench/OfflineAssembler/file.js"),
            file!("RexBench/OfflineAssembler/LowLevelInterpreter.js"),
            file!("RexBench/OfflineAssembler/LowLevelInterpreter32_64.js"),
            file!("RexBench/OfflineAssembler/LowLevelInterpreter64.js"),
            file!("RexBench/OfflineAssembler/InitBytecodes.js"),
            file!("RexBench/OfflineAssembler/expected.js"),
            file!("RexBench/OfflineAssembler/benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "UniPoker",
        &[
            file!("RexBench/UniPoker/poker.js"),
            file!("RexBench/UniPoker/expected.js"),
            file!("RexBench/UniPoker/benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "validatorjs",
        &[
            file!("validatorjs/dist/bundle.es6.min.js"),
            file!("validatorjs/benchmark.js"),
        ],
    ),
    JetStreamCase::timed("hash-map", &[file!("simple/hash-map.js")]),
    JetStreamCase::timed("doxbee-promise", &[file!("simple/doxbee-promise.js")]),
    JetStreamCase::timed("doxbee-async", &[file!("simple/doxbee-async.js")]),
    JetStreamCase::timed("ai-astar", &[file!("SeaMonster/ai-astar.js")]),
    JetStreamCase::timed("gaussian-blur", &[file!("SeaMonster/gaussian-blur.js")]),
    JetStreamCase::timed(
        "stanford-crypto-aes",
        &[
            file!("SeaMonster/sjlc.js"),
            file!("SeaMonster/stanford-crypto-aes.js"),
        ],
    ),
    JetStreamCase::timed(
        "stanford-crypto-pbkdf2",
        &[
            file!("SeaMonster/sjlc.js"),
            file!("SeaMonster/stanford-crypto-pbkdf2.js"),
        ],
    ),
    JetStreamCase::timed(
        "stanford-crypto-sha256",
        &[
            file!("SeaMonster/sjlc.js"),
            file!("SeaMonster/stanford-crypto-sha256.js"),
        ],
    ),
    JetStreamCase::skipped(
        "json-stringify-inspector",
        "requires compressed inspector JSON preload resource not vendored in the shell snapshot",
    ),
    JetStreamCase::skipped(
        "json-parse-inspector",
        "requires compressed inspector JSON preload resource not vendored in the shell snapshot",
    ),
    JetStreamCase::timed(
        "bigint-noble-bls12-381",
        &[
            file!("bigint/web-crypto-sham.js"),
            file!("bigint/noble-bls12-381-bundle.js"),
            file!("bigint/noble-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "bigint-noble-secp256k1",
        &[
            file!("bigint/web-crypto-sham.js"),
            file!("bigint/noble-secp256k1-bundle.js"),
            file!("bigint/noble-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "bigint-noble-ed25519",
        &[
            file!("bigint/web-crypto-sham.js"),
            file!("bigint/noble-ed25519-bundle.js"),
            file!("bigint/noble-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "bigint-paillier",
        &[
            file!("bigint/web-crypto-sham.js"),
            file!("bigint/paillier-bundle.js"),
            file!("bigint/paillier-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "bigint-bigdenary",
        &[
            file!("bigint/bigdenary-bundle.js"),
            file!("bigint/bigdenary-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "proxy-mobx",
        &[
            file!("proxy/common.js"),
            file!("proxy/mobx-bundle.js"),
            file!("proxy/mobx-benchmark.js"),
        ],
    ),
    JetStreamCase::timed(
        "proxy-vue",
        &[
            file!("proxy/common.js"),
            file!("proxy/vue-bundle.js"),
            file!("proxy/vue-benchmark.js"),
        ],
    ),
    JetStreamCase::skipped("mobx-startup", "requires browser startup preload bundle"),
    JetStreamCase::skipped("jsdom-d3-startup", "requires browser startup preload data"),
    JetStreamCase::skipped("web-ssr", "requires browser startup preload bundle"),
    JetStreamCase::timed(
        "raytrace-public-class-fields",
        &[file!("class-fields/raytrace-public-class-fields.js")],
    ),
    JetStreamCase::timed(
        "raytrace-private-class-fields",
        &[file!("class-fields/raytrace-private-class-fields.js")],
    ),
    JetStreamCase::skipped(
        "typescript-lib",
        "requires TypeScript preload project files",
    ),
    JetStreamCase::timed("async-fs", &[file!("generators/async-file-system.js")]),
    JetStreamCase::timed("sync-fs", &[file!("generators/sync-file-system.js")]),
    JetStreamCase::timed(
        "lazy-collections",
        &[file!("generators/lazy-collections.js")],
    ),
    JetStreamCase::timed("js-tokens", &[file!("generators/js-tokens.js")]),
    JetStreamCase::skipped(
        "threejs",
        "requires browser-adjacent Three.js bundle coverage",
    ),
    JetStreamCase::skipped(
        "HashSet-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "quicksort-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "gcc-loops-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "tsf-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "richards-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "sqlite3-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "Dart-flute-complex-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "Dart-flute-todomvc-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "Kotlin-compose-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "transformersjs-bert-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "transformersjs-whisper-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "tfjs-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "tfjs-wasm-simd",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "argon2-wasm",
        "WebAssembly workload is outside the shell JS engine surface",
    ),
    JetStreamCase::skipped(
        "babylonjs-startup-es5",
        "requires browser startup preload bundle",
    ),
    JetStreamCase::skipped(
        "babylonjs-startup-es6",
        "requires browser startup preload bundle",
    ),
    JetStreamCase::skipped(
        "babylonjs-scene-es5",
        "requires browser scene assets and preload data",
    ),
    JetStreamCase::skipped(
        "babylonjs-scene-es6",
        "requires browser scene assets and preload data",
    ),
    JetStreamCase::skipped(
        "bomb-workers",
        "requires Worker APIs outside the current shell harness",
    ),
    JetStreamCase::skipped(
        "segmentation",
        "requires Worker APIs outside the current shell harness",
    ),
    JetStreamCase::skipped(
        "WSL",
        "requires the full WSL multi-file compiler corpus, not yet vendored",
    ),
    JetStreamCase::timed("sunspider-3d-cube", &[file!("SunSpider/3d-cube.js")]),
    JetStreamCase::timed(
        "sunspider-3d-raytrace",
        &[file!("SunSpider/3d-raytrace.js")],
    ),
    JetStreamCase::timed("sunspider-base64", &[file!("SunSpider/base64.js")]),
    JetStreamCase::timed("sunspider-crypto-aes", &[file!("SunSpider/crypto-aes.js")]),
    JetStreamCase::timed("sunspider-crypto-md5", &[file!("SunSpider/crypto-md5.js")]),
    JetStreamCase::timed(
        "sunspider-crypto-sha1",
        &[file!("SunSpider/crypto-sha1.js")],
    ),
    JetStreamCase::timed(
        "sunspider-date-format-tofte",
        &[file!("SunSpider/date-format-tofte.js")],
    ),
    JetStreamCase::timed(
        "sunspider-date-format-xparb",
        &[file!("SunSpider/date-format-xparb.js")],
    ),
    JetStreamCase::timed("sunspider-n-body", &[file!("SunSpider/n-body.js")]),
    JetStreamCase::timed("sunspider-regex-dna", &[file!("SunSpider/regex-dna.js")]),
    JetStreamCase::timed(
        "sunspider-string-unpack-code",
        &[file!("SunSpider/string-unpack-code.js")],
    ),
    JetStreamCase::timed("sunspider-tagcloud", &[file!("SunSpider/tagcloud.js")]),
];

pub const fn cases() -> &'static [JetStreamCase] {
    CASES
}
