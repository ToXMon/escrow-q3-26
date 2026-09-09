# PDAs, ATAs & Token Metadata — visual tutor notes

_2026-09-09 · AdaL · builds on Solana Bootcamp Lesson 1_

**TL;DR**: Lesson 1 stopped at PDAs/CPIs and deliberately deferred token
accounts. This doc + three diagrams close that gap: how a **PDA** becomes an
address, how an **ATA** is "just a special PDA" for token accounts, and how a
bare **mint** gets a name/image (metadata) and, for NFTs, a supply-of-one
guarantee (master edition). Every claim below is grounded in code already in
this repo (`programs/escrowq32026/src/instructions/make.rs`), so you can jump
from the diagram straight to real Anchor constraints.

Diagrams (open in any browser, ☀/☾ toggle, ⏯ pause):
- `docs/adal/learning/pda_derivation.html`
- `docs/adal/learning/ata_derivation.html`
- `docs/adal/learning/token_metadata.html`

---

## 1. Recap: where Lesson 1 left you

```text
ACCOUNT = where things exist
PROGRAM = logic
INSTRUCTION = request to execute specific logic
TRANSACTION = package of one or more instructions
PDA = deterministic address controlled through a program
CPI = one program calling another program
```

You already know: **a PDA is an address, not automatically an account.**
This doc builds three layers on top of that:

```text
PDA                  →  a program-controlled address
   ↓ (special case)
ATA                  →  a PDA specifically for holding a token balance
   ↓ (attached to a mint, not a wallet)
Metadata / Edition   →  PDAs that decorate a mint with name/image/NFT rules
```

---

## 2. PDA derivation, precisely (diagram: `pda_derivation.html`)

### Intuition
You want an address that (a) always comes out the same for the same inputs,
and (b) nobody holds a private key for — so only the *program* can "sign" for
it. Solana gets this by deliberately searching for an address that falls
**off** the ed25519 curve (where valid keypairs live). An address with no
matching private key can't be forged by a human, but the program can still
prove authority over it because it knows the exact recipe that produced it.

### Precise definition
```text
PDA = the first (highest bump, canonical) address such that
      sha256(seeds || bump || program_id || "ProgramDerivedAddress")
      does NOT land on the ed25519 curve
```
Practically, the runtime function `find_program_address(seeds, program_id)`
tries `bump = 255, 254, 253, …` until it finds an off-curve result. **Flagging
a Lesson-1 transcript inconsistency**: one part of the transcript says the
search starts at "256," another says "255 down to 0" — a `u8` bump only has
range 0–255, so **255 is the correct starting point**; treat "256" as a
transcription slip, not a rule to memorize.

### Why it exists
Ordinary accounts need a keypair (public+private key) so a human/wallet can
sign for them. Program-controlled state needs an address too, but you don't
want a private key floating around for a vault or an escrow — whoever held
that key could drain it directly, bypassing your program's rules entirely. A
PDA solves this: no private key exists, so the *only* way to "sign" on its
behalf is by having your program call `invoke_signed` with the exact seeds
that derived it. Authority moves from "who holds the key" to "who can present
the correct seeds inside the correct program."

### Connects to what you already know
- Still just an **address** — Lesson 1's account/address distinction applies
  identically. Deriving the PDA does **not** create an account; something
  (usually the System Program, via Anchor's `init`) still has to create an
  account at that address.
- The **canonical bump** matters for the same reason ownership boundaries
  matter (Lesson 1 §7/§31): if your program accepted *any* valid bump instead
  of just the canonical one, an attacker could construct a different,
  unexpected-but-technically-valid PDA and try to get your program to treat
  it as if it were the "real" one for a given seed set. Always derive with
  `find_program_address` (which finds the canonical bump) and store/verify
  that exact bump — never accept an attacker-supplied bump as-is.

### Concrete example — from this repo
`programs/escrowq32026/src/instructions/make.rs`:
```rust
seeds = [ESCROW_SEED, maker.key().as_ref(), seed.to_le_bytes().as_ref()],
bump
```
Inputs: the program's own `ESCROW_SEED` constant, the maker's wallet pubkey,
and a caller-supplied `u64 seed` (lets one maker open multiple escrows).
Anchor's `bump` (no value given) means "find the canonical bump for me during
`init`." On later instructions (`take.rs`, `refund.rs`, `update.rs`) you'll
see `bump = escrow.bump` — reusing the *stored* bump rather than re-deriving,
which is both cheaper and enforces "must be the canonical one we created."

### Active recall
> If two different bumps (say 253 and 251) both happen to produce valid
> off-curve addresses for the same seeds, why should your program only ever
> accept the one found by `find_program_address` (the canonical bump), and
> never let a client supply an arbitrary bump?

*(Hint: think about what your program treats as "the" escrow for a given
maker+seed — what happens if there are two technically-valid addresses that
could both claim to be it?)*

---

## 3. ATAs (diagram: `ata_derivation.html`)

### Intuition
You already know a wallet holding SOL is just an account owned by the System
Program. But a wallet holding *USDC* needs somewhere to put that balance —
SOL and USDC can't live in the same account, because a System-Program-owned
account can't hold SPL Token data. The Token Program defines a **token
account** type for this. But if every app invented its own convention for
"where does Alice's USDC-holding account live?", every integration would need
a lookup table. The **Associated Token Account (ATA) Program** fixes this by
making that address *itself* a PDA — deterministic, so anyone can compute it
from just `(wallet, mint)` without asking anyone.

### Precise definition
```text
ATA = PDA derived from
      seeds = [wallet, token_program_id, mint]
      program_id = associated_token_program_id
```
It is a **token account** (owned by the Token Program, holding a balance of
one specific mint) whose *address* happens to be a PDA of the ATA Program.
This is exactly the same derive → maybe-create → use pattern from §2, just
with a fixed seed convention everyone agrees on.

### Why it exists
Without ATAs, "give Alice some USDC" requires knowing which specific token
account address Alice wants to receive into — she'd have to create one and
tell you about it first. With ATAs, you derive `(Alice, USDC_mint)` yourself,
and if it doesn't exist yet you can create it for her (anyone can pay the
rent — the ATA program's `create` instruction is permissionless and
idempotent, meaning calling it twice is safe, the second call is a no-op).
This is the "predictable address, no database lookup" idea from Lesson 1 §28
(deterministic user state), specialized to token balances.

### Connects to what you already know
- ATA derivation is **PDA derivation** (§2) with a fixed seed layout — same
  canonical-bump mechanics, same "address ≠ account until created" rule.
- Ownership (Lesson 1 §6): the ATA account's `owner` field is the **Token
  Program** (it's a token account), but its Token-Program-level `authority`
  is the wallet — two different "ownership" concepts stacked. Don't confuse
  Solana account ownership (who can write account data — always the Token
  Program for a token account) with the SPL-Token-level authority field
  (who can authorize transfers out — the wallet, or a PDA in escrow's case).

### Concrete example — from this repo
`programs/escrowq32026/src/instructions/make.rs`:
```rust
#[account(
    init,
    payer = maker,
    associated_token::mint = mint_a,
    associated_token::authority = escrow,   // ← not maker!
    associated_token::token_program = token_program
)]
pub vault: InterfaceAccount<'info, TokenAccount>,
```
This is the escrow's **vault** — an ATA whose derivation seeds use the
`escrow` PDA as the "wallet" slot instead of a human wallet. That's the whole
trick behind program-controlled token custody: the escrow PDA has no private
key (per §2), but it can be the `authority` of a token account, and the
escrow program proves that authority via `invoke_signed` with the escrow's
seeds when it needs to move tokens out (see `take.rs`'s
`CpiContext::new_with_signer`).

Compare `maker_ata_a` in the same file — a completely ordinary ATA where the
authority is the maker's real wallet:
```rust
#[account(
    mut,
    associated_token::mint = mint_a,
    associated_token::authority = maker,
)]
pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,
```

### Active recall
> The escrow's `vault` account and Alice's personal USDC ATA are both "ATAs."
> What's the one derivation input that differs between them, and why does
> that one difference let the *program* — not a human — control the vault?

---

## 4. Token metadata & NFTs (diagram: `token_metadata.html`)

### Intuition
An SPL mint by itself is just numbers: `decimals`, `supply`, a couple of
authority pubkeys. It has no name, no symbol, no picture. If you've ever
opened a wallet and seen a token show up with a name and logo, that
information didn't come from the mint — it came from a **separate account**
that some other program (Metaplex's Token Metadata program) created and
attached to that mint by derivation, the exact same "derive a PDA from known
inputs" trick as ATAs.

### Precise definition
```text
Metadata PDA = derive(['metadata', token_metadata_program_id, mint])
```
That account holds `name`, `symbol`, `uri` (usually pointing at an
off-chain JSON file with the image + traits), and `creators`. It is created
by calling the Token Metadata program's `create_metadata_account` instruction
— itself just another program the mint's authority CPIs into (or you CPI
into directly), same composability idea as Lesson 1 §34.

### NFTs are a special case, not a separate token type
There is no "NFT program" distinct from SPL Token. An NFT is simply:
```text
mint with decimals = 0 AND supply = 1
+ a Metadata account (name/image)
+ a Master Edition account (supply-cap enforcement + print authority)
```
The **Master Edition** account is what actually prevents anyone from ever
minting a second unit of that token — it's a second PDA (also derived from
the mint) that the Token Metadata program checks before allowing any further
`mint_to`. A fungible SPL token (e.g. your own reward token, or the escrow's
`mint_a`/`mint_b` in this repo's tests) has a Metadata account too if you want
it to show a name in wallets, but it has **no** Master Edition — nothing caps
its supply at 1, because it isn't meant to be unique.

### Why it exists
Keeping "is this an NFT" as a *derived fact* (supply=1, decimals=0, edition
account present) rather than a special account type keeps the Token Program
itself completely generic — it never needs to know about names, images, or
"NFT-ness" at all. All of that composability lives one layer up, in a
separate program, attached via PDAs. This is the same
programs-should-be-stateless, state-lives-in-accounts principle from Lesson 1
§11, just applied to the metadata layer instead of your own app logic.

### Connects to what you already know
- Metadata PDA and Master Edition PDA are both **PDAs** (§2) — derived, not
  automatically existing, and using a fixed, publicly-known seed scheme so
  any wallet/explorer can find them without a lookup table (same idea as
  ATAs in §3, just decorating a mint instead of a wallet+mint pair).
- The mints in this repo's escrow tests (`programs/escrowq32026/tests/`) are
  created via `litesvm_token::CreateMint` with **no** metadata account at
  all — perfectly valid; metadata is optional decoration, not required for a
  mint to function as a transferable SPL token.

### Active recall
> A token has `decimals = 0` and `supply = 1`. Is it necessarily an NFT? What
> additional account would you need to check for before treating it as "a
> real, supply-capped NFT" rather than just "a fungible token that happens to
> currently have 1 unit minted"?

---

## 5. Updated mastery model

```text
Accounts:      (carry over from Lesson 1)
Programs:      (carry over from Lesson 1)
Instructions:  (carry over from Lesson 1)
Transactions:  (carry over from Lesson 1)
PDAs:          this doc deepens derivation + canonical bump reasoning
CPIs:          this doc's escrow vault example is a CPI-signed-by-PDA case
ATAs:          NEW — PDA specialization for token accounts
Token metadata: NEW — PDA specialization for mint decoration + NFT rules
```

## 6. Five flashcards
1. A PDA is found by searching bumps 255→0 for the first **off-curve**
   result — why off-curve, specifically?
2. What are the three derivation inputs for an ATA?
3. Why must a program always verify/store the **canonical** bump rather than
   accept a caller-supplied one?
4. What Solana-level `owner` does every token account (including ATAs) have,
   regardless of who its Token-Program-level `authority` is?
5. What two conditions on a mint, plus what additional PDA, together make
   something "actually" an NFT rather than just a low-supply fungible token?

## 7. Three architecture questions
1. Design a subscription service: monthly payments pull from a user's ATA
   into a program vault ATA. Which PDAs do you need, and whose authority is
   the vault ATA under?
2. A marketplace lists NFTs for sale. What has to be true about a mint before
   your program should treat it as sellable-as-a-unique-item versus
   sellable-as-a-fungible-lot?
3. You want one wallet to hold both USDC and a game-specific SPL token.
   Explain why these can coexist without any extra program-level bookkeeping
   on your part.

## 8. One security challenge
Your program accepts a `metadata` account from the client without verifying
its address was actually derived via `['metadata', token_metadata_program_id,
mint]` for the specific `mint` account also passed in. What could an attacker
substitute, and what would your program wrongly believe as a result?

## 9. One practical build exercise
Extend this repo's escrow: after `make()`, allow the maker to optionally
attach a Metadata account read (not creation — just verify address) so `take()`
can display the token's name/symbol before a taker accepts the swap. Derive
the Metadata PDA for `mint_a` inside the instruction and add an
`Account<'info, MetadataAccount>` constraint that checks the derivation
matches `mint_a` — don't trust a client-supplied metadata address blindly.
