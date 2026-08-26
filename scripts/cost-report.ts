import { execSync } from "child_process";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

// ─── Types ───────────────────────────────────────────────────────────────────

interface StageResult {
  name: string;
  balanceBefore: number;
  balanceAfter: number;
  cost: number;
  details: string[];
  txSignatures: string[];
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function lamportsToSOL(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9);
}

function formatLamports(lamports: number): string {
  return lamports.toLocaleString("en-US");
}

async function getBalance(connection: Connection, pubkey: PublicKey): Promise<number> {
  // Retry up to 3 times for testnet reliability
  for (let i = 0; i < 3; i++) {
    try {
      return await connection.getBalance(pubkey, "confirmed");
    } catch (e) {
      if (i === 2) throw e;
      await sleep(2000);
    }
  }
  return 0;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function getRentExemption(connection: Connection, size: number): Promise<number> {
  return await connection.getMinimumBalanceForRentExemption(size);
}

// ─── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log("═══════════════════════════════════════════════════════════════");
  console.log("📊 BÁO CÁO CHI PHÍ TRIỂN KHAI BOTANIKA REWARDS DISTRIBUTOR");
  console.log("═══════════════════════════════════════════════════════════════\n");

  const clusterUrl = "https://api.testnet.solana.com";
  const connection = new Connection(clusterUrl, "confirmed");

  // Load wallet
  const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
  const walletKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  const walletPubkey = walletKeypair.publicKey;
  console.log(`Wallet: ${walletPubkey.toBase58()}`);
  console.log(`Cluster: ${clusterUrl}\n`);

  const projectRoot = path.resolve(__dirname, "..");
  const configPath = path.join(__dirname, "config.json");
  const stages: StageResult[] = [];

  // ─── GIAI ĐOẠN 1: Deploy Program ──────────────────────────────────────────

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("🚀 GIAI ĐOẠN 1: Deploy Program");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

  const soFilePath = path.join(projectRoot, "target/deploy/botanika_solana_rewards_distributor.so");
  const soFileSize = fs.statSync(soFilePath).size;
  // Program data account size = 2x the .so file + header (program uses 2x + offset for buffer)
  const programDataSize = soFileSize * 2;

  const balanceBefore1 = await getBalance(connection, walletPubkey);
  console.log(`  Số dư TRƯỚC: ${formatLamports(balanceBefore1)} lamports (${lamportsToSOL(balanceBefore1)} SOL)`);

  // Get program keypair address
  const programKeypairPath = path.join(projectRoot, "target/deploy/botanika_solana_rewards_distributor-keypair.json");
  const programKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(programKeypairPath, "utf-8")))
  );
  const programId = programKeypair.publicKey;
  console.log(`  Program ID: ${programId.toBase58()}`);
  console.log(`  .so file size: ${soFileSize.toLocaleString()} bytes`);

  try {
    console.log(`  Đang deploy...`);
    const deployOutput = execSync(
      `solana program deploy target/deploy/botanika_solana_rewards_distributor.so --max-sign-attempts 100 --with-compute-unit-price 5000`,
      { cwd: projectRoot, encoding: "utf-8", timeout: 300000 }
    );
    console.log(`  Deploy output: ${deployOutput.trim()}`);
  } catch (e: any) {
    console.error(`  ❌ Deploy thất bại: ${e.stderr || e.message}`);
    process.exit(1);
  }

  await sleep(5000); // Wait for finalization
  const balanceAfter1 = await getBalance(connection, walletPubkey);
  const cost1 = balanceBefore1 - balanceAfter1;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter1)} lamports (${lamportsToSOL(balanceAfter1)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost1)} lamports (${lamportsToSOL(cost1)} SOL)`);

  // Get rent for program data account
  const rentForProgram = await getRentExemption(connection, programDataSize);
  const txFee1 = cost1 - rentForProgram;

  // Try to get actual program data size from chain
  let actualProgramDataSize = programDataSize;
  try {
    const programInfo = await connection.getAccountInfo(programId);
    if (programInfo) {
      console.log(`  Program account on-chain size: ${programInfo.data.length} bytes`);
    }
    // Get program data account (the buffer)
    const programShowOutput = execSync(`solana program show ${programId.toBase58()}`, {
      cwd: projectRoot,
      encoding: "utf-8",
    });
    console.log(`  Program info:\n${programShowOutput}`);

    // Parse the ProgramData address and data length
    const dataLenMatch = programShowOutput.match(/Data Length:\s+([\d,]+)\s/);
    if (dataLenMatch) {
      actualProgramDataSize = parseInt(dataLenMatch[1].replace(/,/g, ""));
    }
  } catch (e) {
    // Ignore
  }

  const actualRentForProgram = await getRentExemption(connection, actualProgramDataSize);
  const actualTxFee1 = cost1 - actualRentForProgram;

  stages.push({
    name: "Giai đoạn 1: Deploy Program",
    balanceBefore: balanceBefore1,
    balanceAfter: balanceAfter1,
    cost: cost1,
    details: [
      `Tiền thuê (Rent-exempt)|Lưu trữ file thực thi chương trình (Program Data Account)|${actualProgramDataSize.toLocaleString()} bytes|${formatLamports(actualRentForProgram)}|${lamportsToSOL(actualRentForProgram)} SOL`,
      `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch deploy trên blockchain|-|${formatLamports(actualTxFee1)}|${lamportsToSOL(actualTxFee1)} SOL`,
    ],
    txSignatures: [],
  });

  console.log("\n");

  // ─── GIAI ĐOẠN 2: Create Token ────────────────────────────────────────────

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("🪙 GIAI ĐOẠN 2: Tạo SPL Token (Mint + ATA + Mint Supply)");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

  const balanceBefore2 = await getBalance(connection, walletPubkey);
  console.log(`  Số dư TRƯỚC: ${formatLamports(balanceBefore2)} lamports (${lamportsToSOL(balanceBefore2)} SOL)`);

  // Reset config.json to only keep CLUSTER_URL so create-token makes a fresh token
  const freshConfig: any = { CLUSTER_URL: clusterUrl };
  fs.writeFileSync(configPath, JSON.stringify(freshConfig, null, 2));

  try {
    console.log("  Đang tạo Token Mint...");
    const createTokenOutput = execSync(`npx ts-node scripts/create-token.ts`, {
      cwd: projectRoot,
      encoding: "utf-8",
      timeout: 120000,
    });
    console.log(createTokenOutput);
  } catch (e: any) {
    console.error(`  ❌ Create Token thất bại: ${e.stderr || e.message}`);
    process.exit(1);
  }

  await sleep(3000);
  const balanceAfter2 = await getBalance(connection, walletPubkey);
  const cost2 = balanceBefore2 - balanceAfter2;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter2)} lamports (${lamportsToSOL(balanceAfter2)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost2)} lamports (${lamportsToSOL(cost2)} SOL)`);

  // Read created token info
  const configAfterToken = JSON.parse(fs.readFileSync(configPath, "utf-8"));
  const tokenMint = configAfterToken.TOKEN_MINT;
  const adminAta = configAfterToken.ADMIN_TOKEN_ACCOUNT;

  // Get rent costs for Mint account (82 bytes) and ATA (165 bytes)
  const rentMint = await getRentExemption(connection, 82); // SPL Token Mint size
  const rentATA = await getRentExemption(connection, 165); // SPL Token Account size
  const txFees2 = cost2 - rentMint - rentATA;

  stages.push({
    name: "Giai đoạn 2: Tạo SPL Token",
    balanceBefore: balanceBefore2,
    balanceAfter: balanceAfter2,
    cost: cost2,
    details: [
      `Tiền thuê (Rent-exempt)|Token Mint Account (\`${tokenMint}\`)|82 bytes|${formatLamports(rentMint)}|${lamportsToSOL(rentMint)} SOL`,
      `Tiền thuê (Rent-exempt)|Admin ATA (\`${adminAta}\`)|165 bytes|${formatLamports(rentATA)}|${lamportsToSOL(rentATA)} SOL`,
      `Phí giao dịch (Tx Fee)|Phí 3 giao dịch: create-token, create-account, mint|3 TXs|${formatLamports(txFees2)}|${lamportsToSOL(txFees2)} SOL`,
    ],
    txSignatures: [],
  });

  console.log("\n");

  // ─── GIAI ĐOẠN 3: Initialize Program ──────────────────────────────────────

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("⚙️  GIAI ĐOẠN 3: Initialize Program (Khởi tạo Cấu hình & Vault)");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

  // Update config with PROGRAM_ID
  configAfterToken.PROGRAM_ID = programId.toBase58();
  fs.writeFileSync(configPath, JSON.stringify(configAfterToken, null, 2));

  const balanceBefore3 = await getBalance(connection, walletPubkey);
  console.log(`  Số dư TRƯỚC: ${formatLamports(balanceBefore3)} lamports (${lamportsToSOL(balanceBefore3)} SOL)`);

  try {
    console.log("  Đang initialize...");
    const initOutput = execSync(`npx ts-node scripts/initialize.ts`, {
      cwd: projectRoot,
      encoding: "utf-8",
      timeout: 120000,
    });
    console.log(initOutput);
  } catch (e: any) {
    console.error(`  ❌ Initialize thất bại: ${e.stderr || e.message}`);
    process.exit(1);
  }

  await sleep(3000);
  const balanceAfter3 = await getBalance(connection, walletPubkey);
  const cost3 = balanceBefore3 - balanceAfter3;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter3)} lamports (${lamportsToSOL(balanceAfter3)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost3)} lamports (${lamportsToSOL(cost3)} SOL)`);

  // Read config for vault info
  const configAfterInit = JSON.parse(fs.readFileSync(configPath, "utf-8"));
  const tokenVault = configAfterInit.TOKEN_VAULT;
  const rewardDistPda = configAfterInit.REWARD_DISTRIBUTOR_PDA;

  // RewardDistributor PDA size calculation (post role-separation, P0-RWD-02):
  // Discriminator(8) + 6 Pubkeys [admin/root/payout/pause/treasury_authority, reward_mint](32*6=192)
  // + current_root(32) + epoch_id(8) + token_vault(32) + bump(1) + is_paused(1) + last_updated_at(8)
  // + total_claimed(8) + total_batch_distributed(8) + _reserved(64, fixed-size array, no length prefix)
  // = 8 + 192 + 32 + 8 + 32 + 1 + 1 + 8 + 8 + 8 + 64 = 362 bytes
  const rewardDistSize = 362; // approximate
  const tokenVaultSize = 165; // SPL token account

  const rentVault = await getRentExemption(connection, tokenVaultSize);
  const rentPda = await getRentExemption(connection, rewardDistSize);
  const txFee3 = cost3 - rentVault - rentPda;

  stages.push({
    name: "Giai đoạn 3: Initialize Program",
    balanceBefore: balanceBefore3,
    balanceAfter: balanceAfter3,
    cost: cost3,
    details: [
      `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch khởi tạo (gồm 2 chữ ký: payer + token_vault)|-|${formatLamports(txFee3)}|${lamportsToSOL(txFee3)} SOL`,
      `Tiền thuê (Rent-exempt)|Token Vault (\`${tokenVault}\`)|${tokenVaultSize} bytes|${formatLamports(rentVault)}|${lamportsToSOL(rentVault)} SOL`,
      `Tiền thuê (Rent-exempt)|Reward Distributor PDA (\`${rewardDistPda}\`)|~${rewardDistSize} bytes|${formatLamports(rentPda)}|${lamportsToSOL(rentPda)} SOL`,
    ],
    txSignatures: [],
  });

  console.log("\n");

  // ─── GIAI ĐOẠN 4: Transfer Token to Vault ─────────────────────────────────

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("💸 GIAI ĐOẠN 4: Chuyển Token vào Vault");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

  const balanceBefore4 = await getBalance(connection, walletPubkey);
  console.log(`  Số dư TRƯỚC: ${formatLamports(balanceBefore4)} lamports (${lamportsToSOL(balanceBefore4)} SOL)`);

  try {
    console.log("  Đang chuyển token vào vault...");
    const transferOutput = execSync(`npx ts-node scripts/transfer-to-vault.ts`, {
      cwd: projectRoot,
      encoding: "utf-8",
      timeout: 120000,
    });
    console.log(transferOutput);
  } catch (e: any) {
    console.error(`  ❌ Transfer thất bại: ${e.stderr || e.message}`);
    process.exit(1);
  }

  await sleep(3000);
  const balanceAfter4 = await getBalance(connection, walletPubkey);
  const cost4 = balanceBefore4 - balanceAfter4;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter4)} lamports (${lamportsToSOL(balanceAfter4)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost4)} lamports (${lamportsToSOL(cost4)} SOL)`);

  stages.push({
    name: "Giai đoạn 4: Chuyển Token vào Vault",
    balanceBefore: balanceBefore4,
    balanceAfter: balanceAfter4,
    cost: cost4,
    details: [
      `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch transfer SPL token|1 TX|${formatLamports(cost4)}|${lamportsToSOL(cost4)} SOL`,
    ],
    txSignatures: [],
  });

  // ─── Generate Markdown Report ─────────────────────────────────────────────

  console.log("\n\n");
  console.log("═══════════════════════════════════════════════════════════════");
  console.log("📝 ĐANG TẠO BÁO CÁO MARKDOWN...");
  console.log("═══════════════════════════════════════════════════════════════\n");

  const totalCost = stages.reduce((sum, s) => sum + s.cost, 0);

  let report = `# Báo cáo Chi phí Triển khai Botanika Rewards Distributor trên Solana

Báo cáo chi tiết số dư và phân bổ chi phí thực tế thu thập từ đợt chạy trên mạng **Solana Testnet** cho Program ID \`${programId.toBase58()}\` (Botanika Rewards Distributor).

- **Ngày chạy:** ${new Date().toISOString().split("T")[0]}
- **Mạng:** Solana Testnet (\`${clusterUrl}\`)
- **Wallet:** \`${walletPubkey.toBase58()}\`

---

## 1. Bảng Tổng hợp Chi phí qua các Giai đoạn

| Giai đoạn | Số dư Trước khi chạy | Số dư Sau khi chạy | Chi phí Tiêu tốn (Lamports) | Chi phí Tiêu tốn (SOL) |
| --- | --- | --- | --- | --- |
`;

  for (const stage of stages) {
    report += `| **${stage.name}** | ${formatLamports(stage.balanceBefore)} lamports | ${formatLamports(stage.balanceAfter)} lamports | ${formatLamports(stage.cost)} lamports | ${lamportsToSOL(stage.cost)} SOL |\n`;
  }

  report += `| **Tổng cộng tất cả giai đoạn** | ${formatLamports(stages[0].balanceBefore)} lamports | ${formatLamports(stages[stages.length - 1].balanceAfter)} lamports | **${formatLamports(totalCost)} lamports** | **${lamportsToSOL(totalCost)} SOL** |\n`;

  report += `\n---\n\n## 2. Chi tiết Phân bổ Chi phí từng Giai đoạn\n\n`;

  for (const stage of stages) {
    report += `### ${stage.name}\n\n`;
    report += `- **Tổng chi phí:** \`${formatLamports(stage.cost)} lamports\` (${lamportsToSOL(stage.cost)} SOL)\n\n`;
    report += `| Phân loại | Tài khoản / Mục đích | Kích thước | Chi phí (Lamports) | Chi phí (SOL) |\n`;
    report += `| --- | --- | --- | --- | --- |\n`;

    for (const detail of stage.details) {
      const parts = detail.split("|");
      report += `| **${parts[0]}** | ${parts[1]} | ${parts[2]} | ${parts[3]} | ${parts[4]} |\n`;
    }

    report += `| **Tổng cộng** |  |  | **${formatLamports(stage.cost)}** | **${lamportsToSOL(stage.cost)} SOL** |\n`;
    report += `\n---\n\n`;
  }

  report += `## 3. Ghi chú\n\n`;
  report += `- **Tiền thuê (Rent-exempt):** Đây là khoản SOL phải ký quỹ để giữ tài khoản trên blockchain vĩnh viễn. Khoản này có thể thu hồi khi đóng tài khoản.\n`;
  report += `- **Phí giao dịch (Tx Fee):** Đây là phí cố định trên Solana, thường là 5,000 lamports/chữ ký. Priority fee có thể tăng thêm tùy theo cấu hình.\n`;
  report += `- Kích thước file .so: ${soFileSize.toLocaleString()} bytes\n`;

  // Save report
  const reportPath = path.join(projectRoot, "COST_REPORT.md");
  fs.writeFileSync(reportPath, report);
  console.log(`✅ Báo cáo đã được lưu tại: ${reportPath}`);
  console.log("\n" + report);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
