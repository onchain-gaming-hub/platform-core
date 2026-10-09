# platform-core

program กลางของ **blockchain-games-hub** (Solana · Anchor) — ของที่ทุก program ใน hub ใช้ร่วมกัน:

- **RBAC** — `RoleMember` PDA ต่อ (role, wallet) · Admin หลาย wallet ได้
- **Game Registry** — `GameEntry` ต่อ game program (`Active` / `Suspended`)
- **PlatformConfig** — fee 1% (boost 1%, ad 1% ปรับได้, เพดาน 20% บังคับใน code)
- **Treasury PDA** — รับ platform fee · ถอนเป็นรายได้ (Admin)
- **Boost mint allowlist** · **โฆษณาพื้นฐาน** · **SDK / IDL กลาง**

| | |
|---|---|
| Spec | [`specs/spec-v1.md`](specs/spec-v1.md) |
| Decisions | [`../DECISIONS.md`](../DECISIONS.md) |
| Owner | `maintainer` (`<internal>`) |

สถานะ: **Phase 0–3 เสร็จ** (layout · `initialize` · RBAC · `set_fee` · game registry · boost allowlist · treasury · default ads · IDL export · consumer kit verified) — ดู [`specs/designs/04-implementation-plan.md`](specs/designs/04-implementation-plan.md) · devnet only

```bash
./scripts/test.sh          # anchor build + build mock consumer + cargo test (test โหลด .so ที่ build แล้ว)
./scripts/export-idl.sh    # หลัง anchor build: อัปเดต idls/platform_core.json ที่ consumer ใช้
```
