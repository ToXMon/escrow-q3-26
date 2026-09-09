# Homework 2 — Vault and Trustless Escrow

Anchor programs for the Turbin3 Homework 2 requirements:

1. A native-SOL vault with `initialize`, `deposit`, `withdraw`, and `close`.
2. A token escrow with `make`, `take`, `refund`, and `update`.
3. Rust LiteSVM tests covering the required lifecycle and balance transitions.

## Architecture

### Vault program

The vault uses deterministic, user-scoped PDAs:

- State PDA: `["state", user]`
- SOL vault PDA: `["vault", user]`

The vault PDA is a system-owned account that holds native SOL. The state account stores both PDA bumps. Only the user who owns the state PDA can deposit, withdraw, or close.

`close` refuses to close while funds above the rent reserve remain. Withdraw first, then close.

### Escrow program

The escrow PDA is derived from:

```text
["escrow", maker, seed]
```

The maker deposits Token A into an associated token account owned by the escrow PDA. The escrow state records:

- maker
- Token A mint
- Token B mint
- requested Token B amount
- seed and bump
- expiration field

The `take` instruction atomically:

1. Transfers Token B from the taker to the maker.
2. Transfers all deposited Token A from the PDA-controlled vault to the taker.
3. Closes the vault and escrow account.

The PDA signer seeds ensure the maker cannot bypass escrow logic and directly withdraw the vault.

## Instructions

### Vault

```text
initialize()                 Create state and rent-reserve vault PDAs
deposit(amount)              Deposit native SOL
withdraw(amount)             Withdraw available SOL, preserving rent
close()                      Close an empty vault and return rent
```

### Escrow

```text
make(seed, deposit, receive, expiration)
take()
refund()
update(receive, expiration)
```

`update` is maker-only and changes the requested Token B amount and expiration terms while leaving the maker, mints, seed, and deposited amount immutable.

## Testing

The test suite uses Rust and LiteSVM, not a live validator or devnet wallet.

### Submission evidence

The captured passing run is preserved in:

- [`docs/test-output.txt`](docs/test-output.txt) — test and quality-gate output
- [`docs/test-results-1.jpeg`](docs/test-results-1.jpeg) — terminal screenshot of the full test run
- [`docs/test-results-2.jpeg`](docs/test-results-2.jpeg) — terminal screenshot of the escrow negative-path tests
- [`docs/test-results-3.jpeg`](docs/test-results-3.jpeg) — terminal screenshot of the vault negative-path tests

The evidence records six passing workspace test binaries covering 17 test cases: vault IDs, vault lifecycle, vault negative paths, escrow make/refund, escrow update/take, and escrow negative paths.


Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --tests
```

Current coverage includes:

- Vault initialize, deposit, partial withdraw, rejection of close while non-empty, final withdraw, and close.
- Escrow make and refund with vault/escrow closure.
- Escrow update followed by take.
- Token A delivery to the taker.
- Token B payment to the maker.
- Closure of the escrow and token vault after take.
- PDA-derived account and token-balance assertions.

The LiteSVM tests load compiled SBF artifacts from `target/deploy`. On this macOS environment, the default Solana `v1.42` platform bundled Cargo 1.75 cannot parse the Edition-2024 dependency graph. The compatible cached `v1.52` platform uses Cargo 1.89:

```bash
cargo-build-sbf --tools-version v1.52 \
  --manifest-path programs/escrowq32026/Cargo.toml \
  --sbf-out-dir target/deploy

cargo-build-sbf --tools-version v1.52 \
  --manifest-path programs/q3_26_vault/Cargo.toml \
  --sbf-out-dir target/deploy
```

Then run the tests:

```bash
cargo test --workspace --tests
```

## Security invariants

- Vault and escrow authorities are verified through PDA seeds and bumps.
- Escrow vault authority is the escrow PDA, not the maker.
- Token accounts are checked against their expected mint and owner relationships.
- `take` uses checked token transfers and performs both sides in one atomic transaction.
- Refund is maker-authorized.
- Update is maker-authorized.
- Closed escrow accounts cannot be reused.
- Tests assert balances and account closure, not just transaction success.
- No devnet transactions or wallet keypairs are required to run the tests.

## Source references

- Anchor: https://www.anchor-lang.com/docs
- LiteSVM: https://www.litesvm.com/docs/getting-started
- Solana documentation: https://solana.com/docs
- Course escrow deck: `../spl-nft-q326/docs/Solana_Developer_Course___Deck_05___Escrow__DeFi___NFTs.pdf`
- Course programs/CPI deck: `../spl-nft-q326/docs/Solana_Developer_Course___Deck_04___Programs___Frontend.pdf`
- Course token deck: `../spl-nft-q326/docs/Solana_Developer_Course___Deck_03___Tokens.pdf`
- Vault reference: https://github.com/moses7054/anchor_vault_starter_q3_26
- Escrow starter: https://github.com/ShrinathNR/escrow-q3-26

## Scope

Required Homework 2 tasks are implemented. Timed escrow using the Clock sysvar and the advanced non-custodial vault extension are intentionally not included in this required-scope implementation.
