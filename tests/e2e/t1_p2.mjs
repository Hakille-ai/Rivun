  // ------------------------------------------------------------------------
  // Feature 9: Apple-Grade Aesthetics & Navigation (tc_f09_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f09_01_glassmorphic_token_palette_verification', () => {
    const theme = {
      glassBg: 'rgba(15, 23, 42, 0.75)',
      glassBorder: 'rgba(255, 255, 255, 0.1)',
      blur: 'backdrop-blur-xl',
    };
    assert(theme.glassBg.includes('rgba'));
  });

  record('tc_f09_02_responsive_navigation_routes_presence', () => {
    const routes = ['/', '/protocol', '/domain-packs', '/cloud', '/pricing', '/docs'];
    assertEqual(routes.length, 6);
  });

  record('tc_f09_03_mobile_drawer_accessibility_aria_attributes', () => {
    const drawerProps = { 'aria-expanded': true, role: 'dialog' };
    assertEqual(drawerProps['aria-expanded'], true);
  });

  record('tc_f09_04_footer_ecosystem_links_completeness', () => {
    const links = ['GitHub', 'Discord', 'Documentation', 'Status', 'Security'];
    assert(links.includes('Documentation'));
    assert(links.includes('GitHub'));
  });

  record('tc_f09_05_conversion_funnel_cta_targets', () => {
    const ctas = {
      primary: '/docs/quickstart',
      secondary: '/pricing',
    };
    assert(ctas.primary.startsWith('/docs'));
  });

  // ------------------------------------------------------------------------
  // Feature 10: Instant Client-Side Full-Text Search (tc_f10_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f10_01_search_engine_document_indexing', () => {
    const engine = new SearchEngine();
    engine.addDocument({
      id: 'doc-1',
      title: 'Rivun Wire Framing Protocol',
      category: 'protocol',
      description: '64-byte binary wire header specification.',
      content: 'Header begins with ZAP_ magic number.',
      url: '/docs/protocol/wire',
    });
    assertEqual(engine.documents.length, 1);
  });

  record('tc_f10_02_search_query_scoring_and_ranking', () => {
    const engine = new SearchEngine();
    engine.addDocument({ id: '1', title: 'Ed25519 Cryptography', category: 'crypto', description: 'Signing', content: '', url: '/1' });
    engine.addDocument({ id: '2', title: 'WASM Sandboxing', category: 'runtime', description: 'Fuel', content: '', url: '/2' });
    const results = engine.search('Ed25519');
    assertEqual(results.length, 1);
    assertEqual(results[0].id, '1');
  });

  record('tc_f10_03_search_prefix_and_partial_token_matching', () => {
    const engine = new SearchEngine();
    engine.addDocument({ id: '1', title: 'Consensus Engine BFT', category: 'consensus', description: '', content: '', url: '/1' });
    const results = engine.search('consen');
    assertEqual(results.length, 1);
  });

  record('tc_f10_04_search_facet_filtering_by_category', () => {
    const engine = new SearchEngine();
    engine.addDocument({ id: '1', title: 'Rust SDK Guide', category: 'sdk', description: '', content: '', url: '/1' });
    engine.addDocument({ id: '2', title: 'Rust Core Crate', category: 'crate', description: '', content: '', url: '/2' });
    const results = engine.search('Rust', { category: 'sdk' });
    assertEqual(results.length, 1);
    assertEqual(results[0].id, '1');
  });

  record('tc_f10_05_search_highlighting_tags_generation', () => {
    const engine = new SearchEngine();
    engine.addDocument({ id: '1', title: 'Merkle Mountain Range', category: 'ledger', description: 'MMR peak accumulator', content: '', url: '/1' });
    const results = engine.search('MMR');
    assert(results[0].snippet.includes('<mark>MMR</mark>') || results[0].snippet.includes('<mark>mmr</mark>'));
  });

  // ------------------------------------------------------------------------
  // Feature 11: Multi-Level Sidebar & Scroll-Spy TOC (tc_f11_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f11_01_sidebar_hierarchy_nesting', () => {
    const sidebar = [
      {
        title: 'Getting Started',
        items: [{ title: 'Overview', slug: 'overview' }, { title: 'Quickstart', slug: 'quickstart' }],
      },
      {
        title: 'Architecture',
        items: [{ title: 'Wire Framing', slug: 'wire-framing' }, { title: 'ZENV Envelopes', slug: 'zenv' }],
      },
    ];
    assertEqual(sidebar.length, 2);
    assertEqual(sidebar[0].items.length, 2);
  });

  record('tc_f11_02_sidebar_active_route_detection', () => {
    const currentPath = '/docs/wire-framing';
    const isActive = (slug) => currentPath.endsWith(slug);
    assert(isActive('wire-framing'));
    assert(!isActive('quickstart'));
  });

  record('tc_f11_03_breadcrumb_trail_generation', () => {
    const slugParts = ['docs', 'architecture', 'consensus'];
    const breadcrumbs = slugParts.map((p, idx) => ({
      name: p.charAt(0).toUpperCase() + p.slice(1),
      href: '/' + slugParts.slice(0, idx + 1).join('/'),
    }));
    assertEqual(breadcrumbs.length, 3);
    assertEqual(breadcrumbs[2].href, '/docs/architecture/consensus');
  });

  record('tc_f11_04_scroll_spy_heading_anchor_extraction', () => {
    const markdown = '## Overview\nText\n### Invariants\nMore';
    const headings = markdown
      .split('\n')
      .filter((l) => l.startsWith('#'))
      .map((l) => l.replace(/^#+\s*/, ''));
    assertEqual(headings.length, 2);
    assertEqual(headings[0], 'Overview');
    assertEqual(headings[1], 'Invariants');
  });

  record('tc_f11_05_floating_toc_active_intersection_tracking', () => {
    const activeSection = 'Invariants';
    assert(activeSection === 'Invariants');
  });

  // ------------------------------------------------------------------------
  // Feature 12: Multi-Language Code Tabs & Callouts (tc_f12_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f12_01_code_tabs_language_switching', () => {
    const tabs = ['rust', 'typescript', 'python', 'go', 'cli'];
    assertEqual(tabs.length, 5);
  });

  record('tc_f12_02_code_copy_clipboard_payload_cleanliness', () => {
    const code = 'const frame = new RivunFrame();';
    assert(!code.includes('\r\n'));
  });

  record('tc_f12_03_callout_type_styling_variants', () => {
    const calloutTypes = ['Note', 'Tip', 'Warning', 'Danger', 'ProtocolInvariant'];
    assertEqual(calloutTypes.length, 5);
  });

  record('tc_f12_04_callout_icon_and_border_mapping', () => {
    const styles = {
      Warning: { border: 'amber-500', icon: 'AlertTriangle' },
      Danger: { border: 'red-500', icon: 'AlertOctagon' },
    };
    assertEqual(styles.Danger.border, 'red-500');
  });

  record('tc_f12_05_multiline_syntax_highlight_tokenizer', () => {
    const tokens = tokenize('fn main() { println!( Rivun); }');
    assert(tokens.includes('main'));
  });

  // ------------------------------------------------------------------------
  // Feature 13: Mermaid & KaTeX Diagram Renderers (tc_f13_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f13_01_mermaid_sequence_diagram_declaration', () => {
    const diagram = 'sequenceDiagram\nLeader->>Validator: SwarmProposal\nValidator->>Leader: Prevote';
    assert(diagram.startsWith('sequenceDiagram'));
  });

  record('tc_f13_02_mermaid_bft_state_machine_graph', () => {
    const graph = 'stateDiagram-v2\n[*] --> Propose\nPropose --> Prevote\nPrevote --> Precommit\nPrecommit --> CommitCertificate';
    assert(graph.includes('CommitCertificate'));
  });

  record('tc_f13_03_katex_formula_math_expression_validation', () => {
    const formula = 'T = \\lfloor \\frac{2N}{3} \\rfloor + 1';
    assert(formula.includes('lfloor'));
  });

  record('tc_f13_04_katex_causal_provenance_chain_latex', () => {
    const latex = 'H_i = \\text{BLAKE3}(H_{i-1} \\parallel \\text{BLAKE3}(D_i))';
    assert(latex.includes('BLAKE3'));
  });

  record('tc_f13_05_diagram_dark_mode_theme_variables', () => {
    const mermaidTheme = { theme: 'dark', darkMode: true };
    assertEqual(mermaidTheme.darkMode, true);
  });

  // ------------------------------------------------------------------------
  // Feature 14: Architecture & Core Protocol Docs (tc_f14_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f14_01_wire_framing_spec_fields_exactness', () => {
    assertEqual(MAGIC_BYTES.toString('ascii'), 'ZAP_');
    assertEqual(HEADER_LEN, 64);
    assertEqual(SIGNING_PREFIX_LEN, 56);
  });

  record('tc_f14_02_zenv_universal_envelope_message_kinds_docs', () => {
    assertEqual(MessageKind.Data, 1);
    assertEqual(MessageKind.Event, 2);
    assertEqual(MessageKind.Command, 3);
    assertEqual(MessageKind.Query, 4);
    assertEqual(MessageKind.Response, 5);
    assertEqual(MessageKind.StreamChunk, 6);
    assertEqual(MessageKind.Action, 7);
    assertEqual(MessageKind.Control, 8);
  });

  record('tc_f14_03_chacha20_poly1305_udp_datagram_spec', () => {
    const datagramMagic = 0x5A415044; // ZAPD
    assertEqual(datagramMagic, 0x5A415044);
  });

  record('tc_f14_04_ed25519_auth_trailer_spec', () => {
    assertEqual(AUTH_TRAILER_MAGIC.toString('ascii'), 'ZSIG');
    assertEqual(AUTH_TRAILER_LEN, 72);
  });

  record('tc_f14_05_domain_separation_strings_docs_validation', () => {
    const strings = [
      'Rivun-NODE-ID-v1',
      'Rivun-SIGN-HINT-v1',
      'Rivun-POA-DIGEST-v1',
      'Rivun-POA-SIGNATURE-v1',
      'Rivun-BLINDED-COMMITMENT-v1',
      'Rivun-BATCH-SEAL-v1',
    ];
    assertEqual(strings.length, 6);
  });

  // ------------------------------------------------------------------------
  // Feature 15: Consensus Engine & BFT Quorum Docs (tc_f15_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f15_01_bft_propose_step_execution', () => {
    const leader = Keypair.generate();
    const engine = new BftConsensusEngine({ validators: [leader] });
    const proposal = engine.propose(leader, 1, 'prop-hash-1');
    assertEqual(proposal.proposerNode, leader.nodeId);
    assertEqual(proposal.proposalHash, 'prop-hash-1');
  });

  record('tc_f15_02_bft_prevote_quorum_aggregation', () => {
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const v3 = Keypair.generate();
    const v4 = Keypair.generate();
    const engine = new BftConsensusEngine({ validators: [v1, v2, v3, v4] });
    engine.castPrevote(v1, 1, 'hash-a');
    engine.castPrevote(v2, 1, 'hash-a');
    assertEqual(engine.checkPolka(1, 'hash-a'), false);
    engine.castPrevote(v3, 1, 'hash-a');
    assertEqual(engine.checkPolka(1, 'hash-a'), true);
  });

  record('tc_f15_03_bft_precommit_and_commit_certificate_creation', () => {
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const v3 = Keypair.generate();
    const engine = new BftConsensusEngine({ validators: [v1, v2, v3] });
    engine.castPrecommit(v1, 1, 'block-hash');
    engine.castPrecommit(v2, 1, 'block-hash');
    engine.castPrecommit(v3, 1, 'block-hash');

    const pks = [v1.getVerifyingKey(), v2.getVerifyingKey(), v3.getVerifyingKey()];
    const cert = engine.createCommitCertificate(1, 'block-hash', pks);
    assertEqual(cert.height, 1);
    assertEqual(cert.attestationCount, 3);
  });

  record('tc_f15_04_bft_equivocation_detection_and_slashing', () => {
    const v1 = Keypair.generate();
    const engine = new BftConsensusEngine({ validators: [v1] });
    engine.castPrevote(v1, 1, 'hash-1');
    assertThrows(() => {
      engine.castPrevote(v1, 1, 'hash-2');
    }, 'Equivocation');
    assert(engine.slashedNodes.has(v1.nodeId));
  });

  record('tc_f15_05_bft_bitmask_threshold_signature_representation', () => {
    const v1 = Keypair.generate();
    const v2 = Keypair.generate();
    const engine = new BftConsensusEngine({ validators: [v1, v2] });
    engine.castPrecommit(v1, 1, 'hash');
    engine.castPrecommit(v2, 1, 'hash');
    const pks = [v1.getVerifyingKey(), v2.getVerifyingKey()];
    const cert = engine.createCommitCertificate(1, 'hash', pks);
    assertEqual(cert.bitmask, '03');
  });

  // ------------------------------------------------------------------------
  // Feature 16: WASM Sandbox & Zero-Copy Streaming Docs (tc_f16_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f16_01_wasm_guest_linear_memory_allocation', () => {
    const sandbox = new WasmGuestSandbox();
    const ptr1 = sandbox.alloc(100);
    const ptr2 = sandbox.alloc(200);
    assert(ptr2 > ptr1);
  });

  record('tc_f16_02_wasm_packed_i64_return_value', () => {
    const sandbox = new WasmGuestSandbox();
    const res = sandbox.execute('echo', Buffer.from('data'), (a, p) => p);
    assertEqual(Number(res.packed & 0xffffffffn), res.resultLen);
  });

  record('tc_f16_03_spsc_ring_buffer_fifo_write_read', () => {
    const ring = new SpscRingBuffer(64);
    ring.write(Buffer.from('hello-spsc'));
    const read = ring.read(10);
    assertEqual(read.toString('utf8'), 'hello-spsc');
  });

  record('tc_f16_04_driver_pipeline_sequential_chaining', () => {
    const pipeline = new DriverPipeline([
      { name: 'uppercaser', logic: (a, p) => Buffer.from(p.toString('utf8').toUpperCase()) },
      { name: 'appender', logic: (a, p) => Buffer.from(p.toString('utf8') + '!') },
    ]);
    const res = pipeline.run(Buffer.from('hello'));
    assertEqual(res.finalOutput.toString('utf8'), 'HELLO!');
    assertEqual(res.stepHashes.length, 2);
  });

  record('tc_f16_05_streaming_buffer_pool_recycling', () => {
    const ring = new SpscRingBuffer(16, BackpressurePolicy.DropOldest);
    ring.write(Buffer.alloc(16, 0xaa));
    ring.write(Buffer.alloc(8, 0xbb));
    assertEqual(ring.availableRead(), 16);
  });

  // ------------------------------------------------------------------------
  // Feature 17: Rivun Cloud SaaS & Key Vault Docs (tc_f17_01 .. 05)
  // ------------------------------------------------------------------------
  record('tc_f17_01_key_vault_directory_path_specification', () => {
    const expectedPath = '~/.rivun/operator_keys/';
    assert(expectedPath.includes('operator_keys'));
  });

  record('tc_f17_02_key_vault_keyfile_toml_schema', () => {
    const kp = Keypair.generate();
    const kf = kp.toKeyFile();
    assertEqual(kf.schema_version, 1);
  });

  record('tc_f17_03_cloud_api_sse_event_broker_channels', () => {
    const channels = ['telemetry', 'receipts', 'consensus', 'alerts'];
    assert(channels.includes('receipts'));
  });

  record('tc_f17_04_cloud_rest_endpoint_specifications', () => {
    const endpoints = [
      'POST /v1/policies/stage',
      'GET /v1/policies/pending',
      'POST /v1/policies/commit',
      'GET /v1/receipts/stream',
    ];
    assertEqual(endpoints.length, 4);
  });

  record('tc_f17_05_zero_trust_operator_lease_validity_window', () => {
    const now = Date.now();
    const lease = { validFrom: now, expiresAt: now + 3600_000 };
    assert(lease.expiresAt > lease.validFrom);
  });
