# ruma-zk-prover

High-level SDK and CLI for generating trustless proofs of Matrix State Resolution.

## Usage

```bash
cargo run -- demo --limit 1000
```

## Features

- **Trustless Join**: Prove you correctly resolved room state without revealing private data.
- **Efficient**: Optimized for O(N) trace generation using the Topological AIR.
- **Modular**: Built on top of `ruma-zk-topological-air` and `ruma-zk-verifier`.
