# Contributing to LaxaFlow

Thank you for your interest in contributing to LaxaFlow! We welcome help in making this streaming payroll and revenue splitting contract secure and efficient.

## Code of Conduct

Please be respectful and professional in all communications and pull requests.

## Workflow

1. Fork the repository and create your branch from `main`.
2. Ensure your code compiles cleanly and passes all tests:
   ```bash
   cargo test --release
   ```
3. Check code styling and quality lints:
   ```bash
   cargo fmt --check
   cargo clippy --all-targets --release
   ```
4. Document any new features or split matrices.
5. Open a Pull Request detailing the changes and linking relevant issues.

## Testing Guidelines

Every new feature or bug fix must be accompanied by comprehensive tests in `src/test.rs`. We require mock-ledger simulations demonstrating the timeline and balance correctness of streams.
