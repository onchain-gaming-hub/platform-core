// platform-core — devnet initialize + verification (runbook step 3–4).
// Usage: bun run initialize.ts <deployer-keypair.json> [first_admin_pubkey]
//   env RPC_URL (default https://api.devnet.solana.com) · BOOST_FEE_BPS / AD_FEE_BPS (default 100)
// Refuses to run on anything but devnet (genesis-hash check). Builds the instruction from
// idls/platform_core.json, so it always matches the deployed interface.
import {
  Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";

const DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
const BPF_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

const idl = JSON.parse(readFileSync(new URL("../../idls/platform_core.json", import.meta.url), "utf8"));
const PROGRAM_ID = new PublicKey(idl.address);

const [deployerPath, firstAdminArg] = process.argv.slice(2);
if (!deployerPath) throw new Error("usage: bun run initialize.ts <deployer-keypair.json> [first_admin]");
const authority = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(deployerPath, "utf8"))));
const firstAdmin = firstAdminArg ? new PublicKey(firstAdminArg) : authority.publicKey;
const boostFee = Number(process.env.BOOST_FEE_BPS ?? 100);
const adFee = Number(process.env.AD_FEE_BPS ?? 100);

const rpcUrl = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const conn = new Connection(rpcUrl, "confirmed");
const genesis = await conn.getGenesisHash();
// Only exception: a local solana-test-validator dry run (`LOCAL_DRY_RUN=1` + localhost URL).
const localDryRun = process.env.LOCAL_DRY_RUN === "1" && /^http:\/\/(127\.0\.0\.1|localhost):/.test(rpcUrl);
if (genesis !== DEVNET_GENESIS && !localDryRun) throw new Error(`refusing: RPC genesis ${genesis} is not devnet`);
if (localDryRun) console.log("⚠️  LOCAL DRY RUN against", rpcUrl);

const pda = (...seeds: (Buffer | Uint8Array)[]) => PublicKey.findProgramAddressSync(seeds, PROGRAM_ID);
const [config] = pda(Buffer.from("config"));
const [treasury, treasuryBump] = pda(Buffer.from("treasury"));
const [adminRole] = pda(Buffer.from("role"), Buffer.from([0]), firstAdmin.toBuffer());
const [programData] = PublicKey.findProgramAddressSync([PROGRAM_ID.toBuffer()], BPF_UPGRADEABLE);

console.log({ program: PROGRAM_ID.toBase58(), authority: authority.publicKey.toBase58(),
  firstAdmin: firstAdmin.toBase58(), boostFee, adFee });

if (await conn.getAccountInfo(config)) {
  console.log("config already exists — skipping initialize, verifying only");
} else {
  const ixIdl = idl.instructions.find((i: any) => i.name === "initialize");
  const data = Buffer.alloc(8 + 2 + 2 + 32);
  Buffer.from(ixIdl.discriminator).copy(data, 0);
  data.writeUInt16LE(boostFee, 8);
  data.writeUInt16LE(adFee, 10);
  firstAdmin.toBuffer().copy(data, 12);
  const byName: Record<string, PublicKey> = {
    authority: authority.publicKey, program: PROGRAM_ID, program_data: programData,
    config, treasury, admin_role: adminRole, system_program: SystemProgram.programId,
  };
  const keys = ixIdl.accounts.map((a: any) => ({
    pubkey: byName[a.name], isSigner: !!a.signer, isWritable: !!a.writable,
  }));
  if (keys.some((k: any) => !k.pubkey)) throw new Error("IDL account list changed — update script");
  const sig = await sendAndConfirmTransaction(conn,
    new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data })),
    [authority]);
  console.log("initialize tx:", sig);

  // --- emit! verification on the live cluster ------------------------------------
  const tx = await conn.getTransaction(sig, { commitment: "confirmed", maxSupportedTransactionVersion: 0 });
  const logs = tx?.meta?.logMessages ?? [];
  console.log(`logs: ${logs.length} lines · ${logs.join("\n").length} bytes · cu ${tx?.meta?.computeUnitsConsumed}`);
  const evIdl = idl.events.find((e: any) => e.name === "Initialized");
  const wantDisc = Buffer.from(evIdl.discriminator);
  const sha = createHash("sha256").update("event:Initialized").digest().subarray(0, 8);
  if (!sha.equals(wantDisc)) throw new Error("IDL event discriminator != sha256(event:Initialized)");
  const data64 = logs.filter((l) => l.startsWith("Program data: ")).map((l) => Buffer.from(l.slice(14), "base64"));
  const ev = data64.find((b) => b.subarray(0, 8).equals(wantDisc));
  if (!ev) throw new Error(`Initialized event NOT found in logs:\n${logs.join("\n")}`);
  const evDecoded = {
    authority: new PublicKey(ev.subarray(8, 40)).toBase58(),
    first_admin: new PublicKey(ev.subarray(40, 72)).toBase58(),
    boost_fee_bps: ev.readUInt16LE(72),
    ad_fee_bps: ev.readUInt16LE(74),
  };
  console.log("Initialized event:", evDecoded);
  if (evDecoded.authority !== authority.publicKey.toBase58() || evDecoded.first_admin !== firstAdmin.toBase58()
      || evDecoded.boost_fee_bps !== boostFee || evDecoded.ad_fee_bps !== adFee) {
    throw new Error("event fields do not match the call");
  }
  console.log("✅ emit! event present and decodes correctly");
}

// --- account verification (offsets from docs/account-layout.md) -------------------------
const disc = (name: string) => Buffer.from(idl.accounts.find((a: any) => a.name === name).discriminator);
const cfg = (await conn.getAccountInfo(config))!;
const c = cfg.data;
const checks: [string, boolean][] = [
  ["config owner = program", cfg.owner.equals(PROGRAM_ID)],
  ["config size 97", c.length === 97],
  ["config discriminator", c.subarray(0, 8).equals(disc("PlatformConfig"))],
  ["config version 1", c[8] === 1],
  [`boost_fee_bps ${c.readUInt16LE(10)}`, c.readUInt16LE(10) === boostFee],
  [`ad_fee_bps ${c.readUInt16LE(12)}`, c.readUInt16LE(12) === adFee],
  [`admin_count ${c.readUInt16LE(14)}`, c.readUInt16LE(14) >= 1],
  ["treasury_bump", c[16] === treasuryBump],
];
const t = (await conn.getAccountInfo(treasury))!;
checks.push(["treasury owner = program", t.owner.equals(PROGRAM_ID)]);
checks.push(["treasury size 42", t.data.length === 42]);
checks.push(["treasury rent-exempt", t.lamports >= (await conn.getMinimumBalanceForRentExemption(42))]);
const r = await conn.getAccountInfo(adminRole);
checks.push(["admin RoleMember exists", !!r && r.owner.equals(PROGRAM_ID)]);
checks.push(["admin role byte 0", !!r && r.data[10] === 0]);
checks.push(["admin member = first_admin", !!r && new PublicKey(r.data.subarray(11, 43)).equals(firstAdmin)]);
for (const [name, ok] of checks) console.log(ok ? "✅" : "❌", name);
if (checks.some(([, ok]) => !ok)) process.exit(1);
console.log(`\nconfig ${config.toBase58()} · treasury ${treasury.toBase58()} · admin_role ${adminRole.toBase58()}`);
