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

สถานะ: **Phase 2 เสร็จ** (layout · `initialize` · RBAC · `set_fee` · game registry · boost allowlist · treasury withdraw) — ดู [`specs/designs/04-implementation-plan.md`](specs/designs/04-implementation-plan.md) · devnet only

```bash
anchor build && cargo test -p platform_core   # test โหลด .so ที่ build แล้ว — build ก่อนเสมอ
```
