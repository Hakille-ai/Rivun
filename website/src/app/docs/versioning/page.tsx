export default function VersioningPage() {
  return (
    <>
      <h1>Versioning Policy</h1>
      <p className="lead">Understand compatibility boundaries and deprecation policies across ZAP crates, CLI behaviors, and wire formats.</p>

      <h2>1. Crates & CLI SemVer</h2>
      <p>Rust crates and CLI behavior adhere strictly to Semantic Versioning (SemVer):</p>
      <ul>
        <li><strong>Patch (x.y.z):</strong> Non-breaking bug fixes and security patches.</li>
        <li><strong>Minor (x.y.0):</strong> Backwards-compatible features and APIs enhancements.</li>
        <li><strong>Major (x.0.0):</strong> Breaking public API movement or CLI updates.</li>
      </ul>

      <h2>2. Protocol Binary Compatibility</h2>
      <p>The ZAP-Wire header enforces a strict <code>VERSION</code> field. The <code>ZENV</code> envelope contains its own version tags. Changes to these versions occur only when binary formats cannot be parsed safely and unambiguously.</p>
      <p>Binary compatibility rules:</p>
      <ul>
        <li>Never reinterpret existing layout bytes silently.</li>
        <li>Strictly reject unknown required flags or nonzero reserved fields.</li>
        <li>Enforce Golden Vector testing suites to block regression.</li>
        <li>Keep downgrade behaviors and error propagation explicit.</li>
      </ul>

      <h2>3. Minimum Supported Rust Version (MSRV)</h2>
      <p>The MSRV is declared in the workspace root <code>Cargo.toml</code>. Active contributors should pin toolchains using the local <code>rust-toolchain.toml</code> config to guarantee matching build environments.</p>
    </>
  );
}
