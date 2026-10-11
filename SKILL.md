---
name: blockchain-games-hub
description: Entry point for building on blockchain-games-hub — a set of shared Solana (Anchor 1.2.1) programs for fully on-chain games plus a gasless relayer. Explains what each component does, the devnet addresses, the order in which a new game integrates (register in platform-core → lobby → rewards-pool → ad-pool → relayer → embed in the hub), the rules every program in the hub follows, and which component guide to read next. Use when starting a new game or frontend on the hub, deciding which program to call, or looking up a hub-wide rule or address.
---

# blockchain-games-hub — Start Here

> Maintained by the blockchain-games-hub maintainers · version **2026-10-11** · **devnet only** (no mainnet deployment exists)
> This file is a **map**. Each component's own guide is the source of truth for that component, and each program's IDL wins over any prose. If this file and a component guide disagree, the component guide wins. Please report the mismatch (§7).
> Repositories live under the GitHub organisation `solana-games-hub`. Links below point at the `main` branch.

## 0. The hub in 30 seconds

Games run **fully on-chain** as Anchor programs. Instead of rebuilding rooms, prize pools and ads for every game, a game **CPIs into shared programs**, and a game's frontend reads their accounts directly. Players log in with a social login (Web3Auth); a **gasless relayer** pays the fees of their sponsored instructions. Boosters and advertisers use their own wallets and pay their own fees.

| Component | What it does for your game | Devnet program ID | IDL | Guide |
|---|---|---|---|---|
| **platform-core** (this repo) | Game Registry (`GameEntry`), roles (RBAC), platform fees + Treasury, boost token allowlist, default ads | `AJnPSYsn7J2muHfKztRtpZz5kBP94dWkNZLr9aMhu18G` | 0.1.0 | [`skills/platform-core-integration/SKILL.md`](skills/platform-core-integration/SKILL.md) |
| **lobby** | Rooms, quick match, ready check, rematch, bot seats · your game CPIs `begin_match` / `end_match` | `5zdBSvep3yV6v1SVD67uw9N5Wb3eT9Hp9D3eg4YSRqRu` | 0.1.0 (frozen) | [lobby `skills/lobby-integration/SKILL.md`](https://github.com/solana-games-hub/lobby/blob/main/skills/lobby-integration/SKILL.md) |
| **rewards-pool** | Per-match prize pools (SOL / token / NFT), community **Boost**, settle → claim | `HgApfwjAg1L2FJL3fX7A4WX5yfhrVV8k9Ahii5Pn8HVW` | 1.0.0 | [rewards-pool `SKILL.md`](https://github.com/solana-games-hub/rewards-pool/blob/main/SKILL.md) |
| **ad-pool** | Per-game ad slots auctioned in SOL, AI-moderated creatives, 99 % of revenue to the game owner | `6T2vwUm5EPBAUQaZNcPwEkgK4rHc9AjoET6DaTq3LEqk` | 0.1.0 | [ad-pool `SKILL.md`](https://github.com/solana-games-hub/ad-pool/blob/main/SKILL.md) |
| **relayer** | Pays fees (and can front rent) for registered program instructions · Cloudflare Worker | — (off-chain) | — | design rules: [relayer `skills/gasless-relayer-integration/SKILL.md`](https://github.com/solana-games-hub/relayer/blob/main/skills/gasless-relayer-integration/SKILL.md) · deployed values + onboarding: [relayer `SKILL.md`](https://github.com/solana-games-hub/relayer/blob/main/SKILL.md) |
| **hub-bridge** | Lets a game on **any origin** run inside the hub's iframe and use the player's wallet over `postMessage` | — (TS SDK) | — | ⏳ **not published yet** — currently specified inside `solana-games-hub/hub-web` (`packages/hub-bridge`); planned to move to its own public repo (§2 step 8) |

The four program IDs above were checked on devnet on 2026-10-11 (executable, owned by the upgradeable BPF loader). To check them yourself:

```bash
curl -s https://api.devnet.solana.com -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getAccountInfo","params":["<PROGRAM_ID>",{"encoding":"jsonParsed"}]}'
# expect: "executable": true, "owner": "BPFLoaderUpgradeab1e11111111111111111111111"
```

Shared platform-core accounts (devnet):

| Account | Seeds (under platform-core) | Address |
|---|---|---|
| `PlatformConfig` | `[b"config"]` | `Be8StYkiu7eBMb6m3TvG7QzMZpFL69bcn1kGmZwSmrJ7` |
| `Treasury` | `[b"treasury"]` | `CBcvwWyNbXKpk5GK6VczDBqaZrP1qsvBzetv7Ms5A6pz` |
| `GameEntry` of your game | `[b"game", <your_program_id>]` | derive it |

## 1. Which guide do I need?

| You are building… | Read, in this order |
|---|---|
| **A game program** (Anchor) | §2 below → platform-core integration §1–§2 → lobby §5 → rewards-pool §0–§4 → relayer integration §2–§3 |
| **A game frontend** | §2 step 8 (hub-bridge) → lobby §4 → rewards-pool §5 (boost / claim buttons) → ad-pool §4 (show ads) → relayer integration §4 |
| **An advertiser frontend** | ad-pool §5–§6 (external wallet, not the relayer) |
| **A tool that reads hub state** (indexer, dashboard) | platform-core integration §6–§8 → each component's "Events" / "Accounts" section |
| **Another app that wants gasless txs** | relayer `SKILL.md` §4 |
| **Operating the hub** (Admin, roles, fees, Treasury) | platform-core integration §7 · relayer `SKILL.md` §6 |

## 2. Integrating a new game — the order that works

Each step depends on the one before it.

1. **Design for the relayer before you write accounts.** Players sign through the relayer, which only accepts certain transaction shapes and requires that every rent it fronts can come back. Decide who pays rent for each account you create (relayer integration §2 "payer slot" and §3 "tx shape"). This is the step teams most often get wrong late.
2. **Deploy your game program on devnet** with the **upgradeable** loader and use the PDA `[b"game_authority"]` of your program as its signer for every CPI into hub programs. The seed is fixed hub-wide. Fund that PDA with SOL if your game opens rewards-pool pools (it pays their rent; see rewards-pool §0).
3. **Get registered in platform-core.** Ask a hub Admin. `register_game` is signed by a hub account holding `GameRegistrar` or `Admin` **and co-signed by your game's owner wallet**. The owner wallet must be a normal wallet with SOL (e.g. Phantom or CLI; a multisig such as Squads is untested), not a social login: platform-core instructions never go through the relayer. Registration creates your `GameEntry` as `Active` and records `owner` (who receives ad revenue and configures ads/lobby) and `game_authority`. Details: platform-core integration §7.
4. **lobby:** the owner calls `init_lobby(params)` once; your start/finish instructions CPI `begin_match` / `end_match` (lobby §5).
5. **rewards-pool:** CPI `open_pool*` at match start and `settle` at match end; players, boosters and cranks do the rest (rewards-pool §0, §4). Token prizes need the mint on the platform-core allowlist.
6. **ad-pool (optional):** the owner calls `init_ad_config` once to define slots; your frontend renders ads by reading accounts (ad-pool §4, §7).
7. **relayer:** send your program ID, the instructions to sponsor and their account layout (relayer integration §5). A hub admin registers the program in the hub's single relayer project (created when hub-web deploys); the relayer pins your program's deploy slot, so **every upgrade pauses sponsorship until it is reviewed again**.
8. **Embed in the hub (hub-bridge).** Your game page runs in the hub's iframe from its own origin and asks the hub to sign through `postMessage`. ⏳ The hub-bridge SDK and its guide are not published yet (planned repo: `solana-games-hub/hub-bridge`). Until then, a game can be played standalone with its own wallet connection.

## 3. Rules every program in the hub follows

These hold across all components. Component guides add their own rules on top.

1. **Verify every hub account you receive.** Anyone can create an account whose bytes look like a `GameEntry` or `RoleMember`. Use `Account<'info, T>` from `declare_program!` (checks owner + discriminator) **and** `seeds = [...]` with `seeds::program = <owner program>::ID` (checks it is *the* account for this game/role/mint). Never `UncheckedAccount` + manual deserialize for data you trust, unless the guide says so (only for accounts you write lamports into).
2. **Check `Active` / allowlists only when something *starts*** (open a pool, create a room, bid). **Never** when funds go *out* (claim, refund, settle, cancel). Suspending a game or removing a token from the allowlist must never lock players' money.
3. **Suspension is an emergency stop.** Suspending a game = `set_game_status(Suspended)` on platform-core **and** removing the whole game program from the relayer at once. Matches in progress stall. Your game must therefore have its own timeouts and a cancel/refund path that **anyone** can trigger with their own SOL.
4. **Rent always comes back.** Every account a player or the relayer pays rent for needs a close path that returns rent to whoever paid it (crank, cron or the user). The relayer refuses programs that break this.
5. **Fees are platform-wide and snapshotted.** Boost fee and ad fee are each **1 %** today (100 bps), adjustable, **capped at 20 % in code**. Consumers copy the current value into their own account when a pool or auction opens, so later changes never affect running matches. Fees round **down** (the remainder stays with the recipient). No fee on NFTs, and none on a game's base reward.
6. **Players go through the relayer; operators do not.** Social login is for players only. Admin, role holders, the ad Moderator Worker and game owners use their own wallets with SOL.
7. **Use IDL files, not crates.** Copy `idls/<program>.json` from each component's repo into your `idls/` folder and use `declare_program!(<program>)` (Anchor **1.2.1**). Always use the IDL file from the component's repo (platform-core and rewards-pool do not publish their IDLs on-chain). TypeScript: **`@anchor-lang/core` 1.2.1** — not `@coral-xyz/anchor`, which is the legacy 0.x line.
8. **Give your error codes an offset** (lobby recommends `#[error_code(offset = 6100)]`). Hub programs use 6000–6099, and errors from a CPI surface in your transaction logs.
9. **devnet only.** Some Solana CLI installs default to mainnet. Always pass `--url devnet`.

## 4. Versioning and compatibility

- Hub account layouts are **frozen after deploy**: a `version` byte after the discriminator, reserved bytes at the end, fields never reordered, seeds never changed, enum variants never reordered. New fields consume reserved bytes.
- A change to an interface ships as a **new IDL version** and is announced to every consumer before deploy. lobby additionally checks its IDL against a frozen copy in a test.
- When you read accounts without Anchor (e.g. `getProgramAccounts` with `memcmp`), use the byte offsets published in each guide; they are part of the contract.

## 5. Glossary (EVM → Solana)

| EVM | In this hub |
|---|---|
| `hasRole(ROLE, msg.sender)` across contracts | The caller **attaches** the `RoleMember` PDA; your program checks owner + seeds (§3 rule 1) |
| `msg.sender` is the calling contract | No such thing. The calling game proves itself by signing with its `game_authority` PDA |
| Registry `mapping(address => GameInfo)` | One `GameEntry` PDA per game program |
| Contract holds ETH | A program-owned PDA (escrow / Treasury) holds lamports; only its owner program can debit it |
| Storage grows freely | Account size is fixed at creation; rent (refundable deposit) is paid up front |

## 6. Status (2026-10-11)

| Component | State |
|---|---|
| platform-core · lobby · rewards-pool · ad-pool | deployed on devnet |
| relayer | staging Worker on devnet (v3 program actions deployed) · the hub's own relayer project is created when hub-web deploys; hub programs are registered there |
| hub-bridge | spec only · repo not published |
| First game (DoGame) | in design |

## 7. Questions and mismatches

Open an issue in the repository of the component concerned. For anything about this map, the Game Registry, roles, fees or the Treasury, open it in `solana-games-hub/platform-core`.
