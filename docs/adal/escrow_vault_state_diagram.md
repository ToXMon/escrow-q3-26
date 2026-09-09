# Escrow & Vault — State Diagrams and Test Coverage

_2026-09-09 · AdaL_

**TL;DR**: Two Anchor programs, two independent state machines. `escrowq32026`
runs a trustless token-swap escrow (make → update ⟲ → take | refund).
`q3_26_vault` runs a personal SOL vault (initialize → deposit ⟲ → withdraw ⟲ →
close). Animated diagram: `docs/adal/escrow_vault_state_diagram.html` (open in
any browser, ☀/☾ toggle top-right, passes `check_diagram.py` with 0
violations).

## 1. Escrow (`escrowq32026`) — token swap

**State account**: `Escrow { seed, maker, mint_a, mint_b, receive, bump, expiration }`,
PDA seeds `["escrow", maker, seed]`. The token vault is an ATA owned by the
`escrow` PDA, holding token A.

| State | Meaning |
|---|---|
| **No escrow** | PDA not yet created for this `(maker, seed)` pair. |
| **Escrow open** | `escrow` account exists, vault ATA holds the maker's deposited token A. |
| **Taken & closed** | Swap completed; `escrow` + `vault` accounts closed, rent returned to maker. |
| **Refunded & closed** | Maker pulled back token A; accounts closed, rent returned. |

### Transitions

- **`make(seed, deposit, receive, expiration)`** — No escrow → Escrow open.
  Guards: `deposit > 0 && receive > 0` (`ErrorCode::InvalidAmount`); PDA
  `init` fails if `seed` was already used by this maker (can't reinit an
  existing account).
- **`update(receive, expiration)`** — Escrow open ⟲ Escrow open (self-loop).
  Guards: `has_one = maker` (maker-only signer), `receive > 0`.
- **`take()`** — Escrow open → Taken & closed. Guards: mint/owner checks on
  every ATA (`taker_ata_a.mint == mint_a`, `.owner == taker`, etc.), taker
  must hold ≥ `escrow.receive` of token B. On success: taker pays token B to
  maker, vault releases all token A to taker, vault + escrow both closed
  (`close = maker`).
- **`refund()`** — Escrow open → Refunded & closed. Guard: `has_one = maker`
  (maker-only). Vault's token A returns to maker, vault + escrow closed.
- Once **Taken & closed** or **Refunded & closed**, the PDA is gone — any
  further `take`/`refund`/`update` on that `(maker, seed)` fails (account no
  longer exists).

### Test coverage (`programs/escrowq32026/tests/`)

| Test | Covers |
|---|---|
| `test_take.rs::update_then_take_transfers_both_sides_and_closes_accounts` | Full happy path: make → update (self-loop) → take → both transfers + account closure verified. |
| `test_negative.rs::make_rejects_zero_deposit_or_receive` | `make` guard: `deposit>0 && receive>0`. |
| `test_negative.rs::make_rejects_reused_seed` | `make` guard: PDA `init` can't reuse a seed. |
| `test_negative.rs::update_rejects_non_maker` | `update` guard: `has_one = maker`. |
| `test_negative.rs::update_rejects_zero_receive` | `update` guard: `receive>0`. |
| `test_negative.rs::refund_rejects_non_maker` | `refund` guard: `has_one = maker`. |
| `test_negative.rs::take_rejects_insufficient_taker_balance` | `take` guard: taker must hold enough token B; vault left untouched. |
| `test_negative.rs::take_rejects_wrong_mint_b` | `take` guard: mint_b account-constraint mismatch. |
| `test_negative.rs::take_rejects_after_refund` | Terminal-state guard: `take` fails once escrow already closed by `refund`. |

## 2. Vault (`q3_26_vault`) — SOL vault

**State account**: `VaultState { vault_bump, state_bump }`, PDA seeds
`["state", user]`. The vault itself is a plain `SystemAccount` PDA
(`["vault", user]`) holding lamports.

| State | Meaning |
|---|---|
| **No vault** | State/vault PDAs not yet created for this user. |
| **Initialized** | `vault_state` exists; vault funded to exactly rent-exempt minimum, 0 spendable. |
| **Funded** | Vault balance > rent-exempt minimum (has spendable SOL). |
| **Closed** | `vault_state` account closed (rent returned to user); vault emptied. |

### Transitions

- **`initialize()`** — No vault → Initialized. Transfers rent-exempt minimum
  from user to vault PDA, stores both bumps. PDA `init` blocks re-running for
  the same user (`initialize_rejects_reuse`).
- **`deposit(amount)`** — Initialized → Funded, or Funded ⟲ Funded. Guard:
  `amount > 0` (`ErrorCode::InvalidAmount`); signer must be the PDA-deriving
  user (wrong user ⇒ seed mismatch/`has_one` fails).
- **`withdraw(amount)`** — Funded ⟲ Funded, or Funded → Initialized (draining
  back to exactly rent). Guards: `amount > 0`, `amount <= balance - rent`
  (`ErrorCode::InsufficientFunds`); user-only signer.
- **`close()`** — Initialized → Closed. Guard: `vault.lamports() <= rent`
  (`ErrorCode::VaultNotEmpty`) — must drain spendable funds via `withdraw`
  first; user-only signer. Sweeps any residual rent lamports to user, then
  closes `vault_state` (rent-exempt reserve returned).

### Test coverage (`programs/q3_26_vault/tests/`)

| Test | Covers |
|---|---|
| `test_lifecycle.rs::vault_initialize_deposit_withdraw_close` | Full happy path: initialize → deposit → partial withdraw → close-while-funded rejected → withdraw remainder → close succeeds. |
| `test_negative.rs::deposit_rejects_zero` | `deposit` guard: `amount>0`. |
| `test_negative.rs::withdraw_rejects_overdraw` | `withdraw` guard: `amount <= balance - rent`. |
| `test_negative.rs::withdraw_rejects_zero` | `withdraw` guard: `amount>0`. |
| `test_negative.rs::initialize_rejects_reuse` | `initialize` guard: PDA `init` can't re-run for same user. |
| `test_negative.rs::close_rejects_wrong_user` | `close` guard: PDA seeds tie to signer; wrong signer fails. |
| `test_negative.rs::withdraw_rejects_wrong_user` | `withdraw` guard: same PDA-ownership check. |
| `test_negative.rs::deposit_rejects_wrong_user` | `deposit` guard: same PDA-ownership check. |

## 3. Diagram

Open `docs/adal/escrow_vault_state_diagram.html` in any browser:
- Two flow lanes (Escrow / Vault), each a pill-to-pill state path.
- Cyan comet dot traces the escrow happy path (make → take); violet comet
  dot traces the vault happy path (initialize → deposit → withdraw → close).
- Self-loop edges (`update`, repeated `deposit`/`withdraw`) render as `↻`
  annotations on the node.
- ☀/☾ toggle top-right, ⏯ pause toggle, honors `prefers-reduced-motion`.
- Verified with `check_diagram.py`: **0 violations**.
