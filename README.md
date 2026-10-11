# platform-core

The shared Solana program (Anchor 1.2.1) of **blockchain-games-hub**: everything every other program in the hub relies on.

- **Game Registry** — one `GameEntry` per game program (`Active` / `Suspended`), with the game's owner and its `game_authority` PDA signer
- **RBAC** — one `RoleMember` PDA per (role, wallet): Admin, GameRegistrar, Moderator, FeeManager, AllowlistManager · several Admins allowed, the last one can never be removed
- **PlatformConfig** — boost fee and ad fee (1 % each today), adjustable, **capped at 20 % in code**
- **Treasury PDA** — receives platform fees (SOL and tokens) · only an Admin can withdraw
- **Boost mint allowlist** — which tokens may be used for boosts and token prizes (unsafe Token-2022 extensions rejected)
- **Default ads** — fallback ads shown when an ad slot has nothing approved

**New to the hub? Start with [`SKILL.md`](SKILL.md)** — what each component does, devnet addresses and the order to integrate a game.
**Integrating with platform-core:** [`skills/platform-core-integration/SKILL.md`](skills/platform-core-integration/SKILL.md).

| | |
|---|---|
| Program ID (devnet) | `AJnPSYsn7J2muHfKztRtpZz5kBP94dWkNZLr9aMhu18G` (deploy slot 509128032) |
| IDL | [`idls/platform_core.json`](idls/platform_core.json), version **0.1.0** |
| Cluster | **devnet only** |
| Design | [`specs/designs/`](specs/designs/) (account layout · instructions · consumer integration) · spec [`specs/spec-v1.md`](specs/spec-v1.md) |
| Deploy | [`specs/deploy/devnet-runbook.md`](specs/deploy/devnet-runbook.md) |

> ⚠️ **Not sponsored by the gasless relayer.** Every platform-core instruction is signed by an operator (Admin / role holder / moderator Worker) or a game owner from a wallet with SOL. Social login is for players only.

```bash
./scripts/test.sh          # anchor build + mock consumer build + cargo test (tests load the built .so)
./scripts/export-idl.sh    # after anchor build: refresh idls/platform_core.json for consumers
```
