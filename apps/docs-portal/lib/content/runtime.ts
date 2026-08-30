import { DocPage } from '../types';

export const RUNTIME_DOCS: DocPage[] = [
  {
    slug: ['runtime', 'wasm-sandboxing'],
    path: '/docs/runtime/wasm-sandboxing',
    title: 'Wasmtime Host Sandboxing',
    description: 'Hardware-level sandboxing, capability-based host call security, and deterministic execution via Wasmtime.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'WASM Runtime',
    headings: [
      { id: 'sandboxing-guarantees', text: 'Sandboxing Guarantees', level: 2 },
      { id: 'host-call-security', text: 'Capability-Based Host Calls', level: 2 },
      { id: 'memory-isolation', text: 'Linear Memory Isolation', level: 2 },
    ],
    callouts: [
      {
        type: 'security',
        title: 'Zero Host Access Invariant',
        content: 'WASM guest drivers have NO access to filesystem, network sockets, or host environment variables unless granted by an explicit CapabilityId token.',
      },
    ],
    rawContent: `
Rivun integrates Bytecode Alliance's **Wasmtime** engine to execute compiled WebAssembly action drivers (\`.wasm\` or \`.zpack\`) with complete isolation.
    `,
  },
  {
    slug: ['runtime', 'driver-abi'],
    path: '/docs/runtime/driver-abi',
    title: 'Driver ABI v1 Specification',
    description: 'Specification of the 3 required exports (rivun_alloc, rivun_dealloc, rivun_execute) and memory layout.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'WASM Runtime',
    headings: [
      { id: 'exported-functions', text: 'Required Exported Functions', level: 2 },
      { id: 'rivun-alloc', text: 'rivun_alloc(len: i32) -> i32', level: 2 },
      { id: 'rivun-dealloc', text: 'rivun_dealloc(ptr: i32, len: i32)', level: 2 },
      { id: 'rivun-execute', text: 'rivun_execute(...) -> i64', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'driver-rust-abi',
        snippets: {
          rust: {
            title: 'lib.rs (WASM Guest)',
            code: `#[no_mangle]\npub extern "C" fn rivun_alloc(len: i32) -> *mut u8 {\n    let mut buf = Vec::with_capacity(len as usize);\n    let ptr = buf.as_mut_ptr();\n    std::mem::forget(buf);\n    ptr\n}\n\n#[no_mangle]\npub unsafe extern "C" fn rivun_dealloc(ptr: *mut u8, len: i32) {\n    let _ = Vec::from_raw_parts(ptr, len as usize, len as usize);\n}\n\n#[no_mangle]\npub unsafe extern "C" fn rivun_execute(\n    action_ptr: *const u8,\n    action_len: i32,\n    payload_ptr: *const u8,\n    payload_len: i32,\n) -> i64 {\n    let action = std::slice::from_raw_parts(action_ptr, action_len as usize);\n    let payload = std::slice::from_raw_parts(payload_ptr, payload_len as usize);\n    \n    // Execute business logic\n    let result_bytes = b"{\\"status\\":\\"SUCCESS\\",\\"code\\":200}";\n    let out_ptr = rivun_alloc(result_bytes.len() as i32);\n    std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), out_ptr, result_bytes.len());\n    \n    // Return packed 64-bit value: (ptr << 32) | len\n    ((out_ptr as u64) << 32) | (result_bytes.len() as u64)\n}`,
          },
        },
      },
    ],
    rawContent: `
All Rivun action drivers must target \`wasm32-wasip1\` and export the **Driver ABI v1** interface:
- \`rivun_alloc(len: i32) -> i32\`: Allocates a linear memory buffer for host payload passing.
- \`rivun_dealloc(ptr: i32, len: i32)\`: Deallocates linear memory buffers.
- \`rivun_execute(action_ptr, action_len, payload_ptr, payload_len) -> i64\`: Main action entrypoint returning a 64-bit packed pointer/length tuple.
    `,
  },
  {
    slug: ['runtime', 'resource-metering'],
    path: '/docs/runtime/resource-metering',
    title: 'Resource Constraints & Fuel Metering',
    description: 'Deterministic fuel budgeting, maximum memory caps (16MB), and epoch-based wall-clock timeouts.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'Resource Control',
    headings: [
      { id: 'fuel-metering', text: 'Deterministic WebAssembly Fuel Metering', level: 2 },
      { id: 'memory-limits', text: 'Linear Memory Hard Caps', level: 2 },
      { id: 'epoch-timeouts', text: 'Epoch Timeout Interrupts', level: 2 },
    ],
    rawContent: `
To prevent infinite loops, memory exhaustion, or denial-of-service, every execution is constrained by:
- **Fuel Metering**: Drivers consume 1 fuel unit per instruction (default budget: 10,000,000 fuel).
- **Linear Memory Limit**: Hard capped at 16 MiB (256 Wasm pages).
- **Epoch Timeout**: Wall-clock watchdog timer fires every 1000ms, triggering immediate trap if exceeded.
    `,
  },
  {
    slug: ['runtime', 'async-pipelines'],
    path: '/docs/runtime/async-pipelines',
    title: 'Async Tokio Driver Pipelines',
    description: 'Non-blocking multi-threaded task scheduling, cooperative yields, and background telemetry queues.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'Async Pipelines',
    headings: [
      { id: 'tokio-architecture', text: 'Tokio Pipeline Architecture', level: 2 },
      { id: 'task-chaining', text: 'Sequential & Concurrent Task Chaining', level: 2 },
    ],
    rawContent: `
The host execution engine coordinates driver execution across asynchronous Tokio workers, separating CPU-bound WASM compilation from I/O-bound sensor streaming.
    `,
  },
  {
    slug: ['runtime', 'spsc-ringbuffers'],
    path: '/docs/runtime/spsc-ringbuffers',
    title: 'Lock-Free SPSC Ring-Buffers',
    description: 'Zero-copy high-throughput streaming I/O with cacheline-aligned atomic head/tail ring buffers.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'Streaming I/O',
    headings: [
      { id: 'ringbuffer-design', text: 'Cacheline-Aligned Lock-Free Design', level: 2 },
      { id: 'zero-copy-streaming', text: 'Zero-Copy Slicing & Modbus Ingestion', level: 2 },
    ],
    rawContent: `
The \`SpscRingBuffer\` achieves over 25,000,000 ops/sec by utilizing 64-byte cacheline padding between producer and consumer atomic indices, preventing false sharing on modern multi-core CPUs.
    `,
  },
  {
    slug: ['runtime', 'inter-driver-ipc'],
    path: '/docs/runtime/inter-driver-ipc',
    title: 'Inter-Driver Zero-Copy IPC',
    description: 'Deterministic chaining of drivers (Perception -> Policy -> Actuator) with aggregate fuel accounting.',
    section: '4. Sandboxed WASM & Streaming',
    subSection: 'Driver IPC',
    headings: [
      { id: 'pipeline-composition', text: 'Composing Driver Pipelines', level: 2 },
      { id: 'fuel-inheritance', text: 'Aggregate Fuel Budget Inheritance', level: 2 },
    ],
    rawContent: `
Drivers can be chained into continuous pipelines where output memory slices from a Perception driver feed directly into a Policy evaluation driver, and then into an Actuator driver with zero intermediate serialization overhead.
    `,
  },
];
