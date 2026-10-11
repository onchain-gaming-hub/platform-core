// platform-core — register a game program in the Game Registry (devnet).
// Usage:
//   bun run register-game.ts <game_program_id> <admin-keypair.json> [owner-keypair.json]
//     sends `register_game` signed by the Admin (fee payer) and the owner (co-signer).
//     If no owner keypair is given, the Admin key is also the game owner.
//   DRY_RUN=1 bun run register-game.ts <game_program_id> <admin_pubkey|keypair> [owner_pubkey|keypair]
//     builds the same transaction and simulates it without signatures. Sends nothing.
//   env RPC_URL (default https://api.devnet.solana.com)
// Refuses to run on anything but devnet (genesis-hash check). Builds the instruction from
// idls/platform_core.json, so it always matches the deployed interface.
import {
  Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction,
  VersionedTransaction, sendAndConfirmTransaction,
} from "@solana/web3.js";
import { existsSync, readFileSync } from "node:fs";

const DEVNET_GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
const BPF_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");
const ROLE_ADMIN = 0;

const idl = JSON.parse(readFileSync(new URL("../../idls/platform_core.json", import.meta.url), "utf8"));
const PROGRAM_ID = new PublicKey(idl.address);
const dryRun = process.env.DRY_RUN === "1";

const [gameArg, adminArg, ownerArg] = process.argv.slice(2);
if (!gameArg || !adminArg) {
  throw new Error("usage: bun run register-game.ts <game_program_id> <admin-keypair.json> [owner-keypair.json]");
}
const loadKeypair = (p: string) => Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(p, "utf8"))));
// Dry run accepts a pubkey or a keypair file; a real run needs keypair files.
const signerOrKey = (arg: string): { key: PublicKey; kp?: Keypair } => {
  if (existsSync(arg)) { const kp = loadKeypair(arg); return { key: kp.publicKey, kp }; }
  if (!dryRun) throw new Error(`not a keypair file: ${arg} (pubkeys are accepted only with DRY_RUN=1)`);
  return { key: new PublicKey(arg) };
};
const gameProgram = new PublicKey(gameArg);
const admin = signerOrKey(adminArg);
const owner = ownerArg ? signerOrKey(ownerArg) : admin;

const rpcUrl = process.env.RPC_URL ?? "https://api.devnet.solana.com";
const conn = new Connection(rpcUrl, "confirmed");
if ((await conn.getGenesisHash()) !== DEVNET_GENESIS) throw new Error("refusing: RPC is not devnet");

const pda = (...seeds: (Buffer | Uint8Array)[]) => PublicKey.findProgramAddressSync(seeds, PROGRAM_ID)[0];
const adminRole = pda(Buffer.from("role"), Uint8Array.of(ROLE_ADMIN), admin.key.toBuffer());
const gameEntry = pda(Buffer.from("game"), gameProgram.toBuffer());
const [gameAuthority] = PublicKey.findProgramAddressSync([Buffer.from("game_authority")], gameProgram);

console.log({
  mode: dryRun ? "DRY RUN (simulate only)" : "SEND", program: PROGRAM_ID.toBase58(),
  game_program: gameProgram.toBase58(), admin: admin.key.toBase58(), owner: owner.key.toBase58(),
  game_entry: gameEntry.toBase58(), game_authority: gameAuthority.toBase58(),
});

// --- pre-checks (the program enforces these too; failing early gives clearer messages) ---
const gp = await conn.getAccountInfo(gameProgram);
if (!gp?.executable || !gp.owner.equals(BPF_UPGRADEABLE)) {
  throw new Error("game program must be deployed with the upgradeable loader (executable)");
}
if (!(await conn.getAccountInfo(adminRole))) throw new Error(`admin ${admin.key.toBase58()} holds no Admin RoleMember`);
if (await conn.getAccountInfo(gameEntry)) throw new Error("game is already registered (GameEntry exists)");
const ownerInfo = await conn.getAccountInfo(owner.key);
if (ownerInfo && !ownerInfo.owner.equals(SystemProgram.programId)) {
  console.warn("⚠️  owner is not a system-owned wallet — it should be (it receives ad revenue)");
}

// --- build register_game(auth_role_kind = Admin) from the IDL ---------------------------
const ixIdl = idl.instructions.find((i: any) => i.name === "register_game");
const data = Buffer.concat([Buffer.from(ixIdl.discriminator), Uint8Array.of(ROLE_ADMIN)]);
const byName: Record<string, PublicKey> = {
  signer: admin.key, auth_role: adminRole, owner: owner.key, game_program: gameProgram,
  game_entry: gameEntry, system_program: SystemProgram.programId,
};
const keys = ixIdl.accounts.map((a: any) => ({ pubkey: byName[a.name], isSigner: !!a.signer, isWritable: !!a.writable }));
if (keys.some((k: any) => !k.pubkey) || keys.length !== Object.keys(byName).length) {
  throw new Error("IDL account list changed — update script");
}
const tx = new Transaction().add(new TransactionInstruction({ programId: PROGRAM_ID, keys, data }));
tx.feePayer = admin.key;

if (dryRun) {
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  const sim = await conn.simulateTransaction(new VersionedTransaction(tx.compileMessage()),
    { sigVerify: false, replaceRecentBlockhash: true });
  console.log(sim.value.logs?.join("\n"));
  if (sim.value.err) throw new Error(`simulation failed: ${JSON.stringify(sim.value.err)}`);
  console.log("✅ simulation succeeded — nothing was sent");
  process.exit(0);
}

const signers = [admin.kp!, ...(owner.kp && !owner.key.equals(admin.key) ? [owner.kp] : [])];
const sig = await sendAndConfirmTransaction(conn, tx, signers);
console.log("register_game tx:", sig);

// --- verify the GameEntry (offsets from docs/account-layout.md §4) ----------------------
const acct = await conn.getAccountInfo(gameEntry);
if (!acct || !acct.owner.equals(PROGRAM_ID) || acct.data.length !== 220) throw new Error("GameEntry missing or wrong");
const d = acct.data;
const got = {
  status: d[10] === 0 ? "Active" : `other(${d[10]})`,
  game_program: new PublicKey(d.subarray(11, 43)).toBase58(),
  owner: new PublicKey(d.subarray(43, 75)).toBase58(),
  game_authority: new PublicKey(d.subarray(75, 107)).toBase58(),
};
console.log("GameEntry:", got);
if (got.status !== "Active" || got.game_program !== gameProgram.toBase58()
    || got.owner !== owner.key.toBase58() || got.game_authority !== gameAuthority.toBase58()) {
  throw new Error("GameEntry fields do not match the call");
}
console.log("✅ registered and Active");
