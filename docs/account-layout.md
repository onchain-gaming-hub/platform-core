# platform-core — Account Layout v1

> ⚠️ This is an **interface**. After the first deploy, fields are never reordered, seeds never change, enum variants never reorder and accounts never shrink. Consumers (other programs, the relayer, indexers) decode these bytes directly.
> EVM comparison: this is the storage layout that every reader accesses slot by slot — move one slot and every reader breaks.
> Source of truth: [`programs/platform_core/src/state/`](../programs/platform_core/src/state/) and [`idls/platform_core.json`](../idls/platform_core.json). Every size and offset below is pinned by `programs/platform_core/tests/test_layout.rs`.

## 0. General rules

| Rule | Detail |
|---|---|
| Discriminator | First 8 bytes = `sha256("account:<StructName>")[..8]` (Anchor 1.2.1 default). Struct names never change. |
| `version: u8` | Always byte 8. v1 = `1`. Bumped when the meaning of a field changes, not on every deploy. |
| `bump: u8` | Always byte 9. Consumers use `bump = acct.bump` instead of searching for it (saves compute). |
| Fields used with `memcmp` | Placed right after `bump`, so their offsets are fixed. |
| Strings | Only in `DefaultAd`, each with `#[max_len]`, after all fixed-size fields. |
| `_reserved: [u8; N]` | At the end, all zero initially. |
| Encoding | Borsh: no padding · enum = 1 byte (variant index) · `bool` = 1 byte |
| Rent | Figures below are devnet values, `(128 + size) × 5,080` lamports. Code always uses `Rent::get()?.minimum_balance(size)`. |

### Adding a field later without breaking consumers

```rust
// v1                                   // v2 — same total size
pub _reserved: [u8; 64],                pub new_field: u64,          // taken from reserved
                                        pub _reserved: [u8; 56],
```

- Old accounts have zeros there, so v2 reads `new_field = 0`. **Zero must always mean "not set".**
- Consumers still on the v1 IDL read the same bytes as `_reserved` and keep working.
- A new enum variant (e.g. `GameStatus::Retired = 2`) cannot be decoded by consumers on the old IDL, so their transaction fails, i.e. "not Active / no permission". This **fails closed**, but it is announced before the variant is used.

## 1. Enums

```rust
#[repr(u8)]
pub enum Role {               // ⚠️ order never changes — the index is also a seed
    Admin            = 0,
    GameRegistrar    = 1,
    Moderator        = 2,     // ad-pool's moderator Worker
    FeeManager       = 3,
    AllowlistManager = 4,
}

#[repr(u8)]
pub enum GameStatus {         // ⚠️ order never changes — indexers memcmp the value 0
    Active    = 0,
    Suspended = 1,            // nothing new may start; claim / refund / settle / cancel still work
}
```

## 2. `PlatformConfig` — `[b"config"]` (single instance)

EVM comparison: contract-level state variables (`uint16 public feeBps`).

| offset | size | field | type | notes |
|---:|---:|---|---|---|
| 0 | 8 | discriminator | | |
| 8 | 1 | `version` | u8 | = 1 |
| 9 | 1 | `bump` | u8 | |
| 10 | 2 | `boost_fee_bps` | u16 | default **100 = 1 %** · rewards-pool boosts |
| 12 | 2 | `ad_fee_bps` | u16 | default **100 = 1 %** · ad-pool |
| 14 | 2 | `admin_count` | u16 | number of `RoleMember(Admin, *)`; never reaches 0 |
| 16 | 1 | `treasury_bump` | u8 | bump of the Treasury PDA |
| 17 | 8 | `initialized_at` | i64 | unix timestamp |
| 25 | 8 | `updated_at` | i64 | last fee change |
| 33 | 64 | `_reserved` | [u8; 64] | |
| | **97** | | | rent ≈ **1,143,000** lamports |

- Both fees are `≤ MAX_FEE_BPS` (2000 = 20 %), enforced in `initialize` and `set_fee`; they are set independently.
- There is no stored maximum (it is the `#[constant] MAX_FEE_BPS`) and no stored Treasury address (derive it from its seed).

## 3. `RoleMember` — `[b"role", &[role as u8], member]`

EVM comparison: OpenZeppelin `AccessControl`'s `mapping(bytes32 role => mapping(address => bool))`. Each (role, wallet) pair is one account: **exists = has the role**; closed = revoked.

| offset | size | field | type | notes |
|---:|---:|---|---|---|
| 0 | 8 | discriminator | | |
| 8 | 1 | `version` | u8 | = 1 |
| 9 | 1 | `bump` | u8 | |
| 10 | 1 | `role` | Role | same as the seed, readable without knowing it |
| 11 | 32 | `member` | Pubkey | same as the seed |
| 43 | 32 | `granted_by` | Pubkey | the granting Admin · **gets the rent back** on revoke / renounce |
| 75 | 8 | `granted_at` | i64 | |
| 83 | 32 | `_reserved` | [u8; 32] | |
| | **115** | | | rent ≈ **1,234,440** lamports |

`member` is any key that can sign a transaction. Multisig vaults have not been tested.

## 4. `GameEntry` — `[b"game", game_program]`

EVM comparison: `mapping(address game => GameInfo)` in a registry contract.

| offset | size | field | type | notes |
|---:|---:|---|---|---|
| 0 | 8 | discriminator | | `memcmp` filter |
| 8 | 1 | `version` | u8 | = 1 |
| 9 | 1 | `bump` | u8 | |
| **10** | 1 | **`status`** | GameStatus | **`memcmp` offset 10 = `0x00` → Active** |
| 11 | 32 | `game_program` | Pubkey | the game's program ID (executable, upgradeable loader) |
| 43 | 32 | `owner` | Pubkey | **the game owner — the single source in the hub**: receives ad revenue, configures the game's ads and lobby |
| 75 | 32 | `game_authority` | Pubkey | `find_program_address([b"game_authority"], game_program).0` |
| 107 | 1 | `game_authority_bump` | u8 | |
| 108 | 32 | `registered_by` | Pubkey | GameRegistrar / Admin that registered it |
| 140 | 8 | `registered_at` | i64 | |
| 148 | 8 | `updated_at` | i64 | last status / owner change |
| 156 | 64 | `_reserved` | [u8; 64] | |
| | **220** | | | rent ≈ **1,767,840** lamports |

- Games are never deleted, only `Suspended`. A program cannot be registered twice (the PDA already exists).
- ⚠️ `game_authority` is tied to the seed `b"game_authority"`: **every game in the hub uses this seed** for its PDA signer.

## 5. `AllowedMint` — `[b"boost_mint", mint]`

EVM comparison: `mapping(address token => bool) allowed`. **Exists = allowed.**

Allowlisting is per mint. SPL Token and Token-2022 are both supported (`token_program` says which). Mints with unsafe Token-2022 extensions are rejected at `allow_mint`, NFTs included.

| offset | size | field | type | notes |
|---:|---:|---|---|---|
| 0 | 8 | discriminator | | |
| 8 | 1 | `version` | u8 | = 1 |
| 9 | 1 | `bump` | u8 | |
| 10 | 32 | `mint` | Pubkey | |
| 42 | 32 | `token_program` | Pubkey | Token or Token-2022 — use it to derive ATAs |
| 74 | 1 | `decimals` | u8 | snapshot for `transfer_checked` |
| 75 | 32 | `added_by` | Pubkey | gets the rent back on disallow |
| 107 | 8 | `added_at` | i64 | |
| 115 | 32 | `_reserved` | [u8; 32] | |
| | **147** | | | rent ≈ **1,397,000** lamports |

- Native SOL needs no `AllowedMint`.
- `allow_mint` also creates the **Treasury ATA** for the mint — **except for a true 1/1 NFT** (`decimals == 0 && supply == 1 && no mint authority`), which never pays a fee. ATA rent ≈ **1,488,440** lamports for SPL Token (165 bytes); Token-2022 ATAs are larger.

## 6. `Treasury` — `[b"treasury"]` (single instance)

A **program-owned** PDA. EVM comparison: a contract holding ETH that only an `onlyRole(ADMIN)` function can withdraw.

| offset | size | field | type |
|---:|---:|---|---|
| 0 | 8 | discriminator | |
| 8 | 1 | `version` | u8 |
| 9 | 1 | `bump` | u8 |
| 10 | 32 | `_reserved` | [u8; 32] |
| | **42** | | rent ≈ **863,600** lamports |

Why program-owned: anyone can credit it, platform-core debits it directly (`sub_lamports`, no CPI), and it is rent-funded at `initialize`, so the first small fee never fails a consumer's transaction.

**SPL fees** sit in the ATA whose authority is the Treasury PDA: `get_associated_token_address_with_program_id(treasury, mint, token_program)` (in JS, pass `allowOwnerOffCurve = true`).

## 7. `DefaultAd` — `[b"default_ad", width.to_le_bytes(), height.to_le_bytes()]`

Fallback ad per slot size. Seeds use `u16` little-endian, 2 bytes each.

| offset | size | field | type | notes |
|---:|---:|---|---|---|
| 0 | 8 | discriminator | | |
| 8 | 1 | `version` | u8 | = 1 |
| 9 | 1 | `bump` | u8 | |
| 10 | 2 | `width` | u16 | same as the seed |
| 12 | 2 | `height` | u16 | same as the seed |
| 14 | 1 | `enabled` | bool | `false` = frontends show nothing |
| 15 | 32 | `updated_by` | Pubkey | |
| 47 | 8 | `updated_at` | i64 | |
| 55 | ≤564 | `creative` | `AdCreative` | **same layout as ad-pool's `AdCreative`** |
| — | 32 | `_reserved` | [u8; 32] | offset varies with string lengths |
| | **≤651** (fully reserved) | | | rent ≈ **3,957,320** lamports |

```rust
pub struct AdCreative {                      // identical to ad-pool's AdCreative
    #[max_len(200)] pub image_url: String,   // 4 + 200
    pub image_sha256: [u8; 32],              // 32
    #[max_len(120)] pub text: String,        // 4 + 120 bytes — shown when the image cannot load
    #[max_len(200)] pub click_url: String,   // 4 + 200
}
```

- No `DefaultAd` for a size, or `enabled = false` → show nothing.
- When an update shortens the creative, old bytes after the new end are not zeroed. Never read past offset 55 except by decoding `creative`, and **never `memcmp` after offset 55**. New features go into `AdCreative` (coordinated with ad-pool) or a new account type.

## 8. Costs (rent deposited; returned when an account is closed)

| Created by | Per | ≈ lamports | Paid by |
|---|---|---:|---|
| `initialize` (Config + Treasury + first Admin) | once | 3,241,040 | upgrade authority |
| `grant_role` | role pair | 1,234,440 | Admin |
| `register_game` | game | 1,767,840 | GameRegistrar / Admin |
| `allow_mint` (+ Treasury ATA) | mint | 2,885,440 | AllowlistManager / Admin |
| `create_default_ad` | size | 3,957,320 | Moderator / Admin |

## 9. Seeds

```rust
pub const CONFIG_SEED:     &[u8] = b"config";
pub const TREASURY_SEED:   &[u8] = b"treasury";
pub const ROLE_SEED:       &[u8] = b"role";        // + [role as u8] + member
pub const GAME_SEED:       &[u8] = b"game";        // + game_program
pub const BOOST_MINT_SEED: &[u8] = b"boost_mint";  // + mint
pub const DEFAULT_AD_SEED: &[u8] = b"default_ad";  // + width LE(2) + height LE(2)
pub const GAME_AUTHORITY_SEED: &[u8] = b"game_authority"; // seed inside the GAME program, not platform-core
```

All are exported in the IDL with `#[constant]`.
