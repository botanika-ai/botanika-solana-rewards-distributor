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
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function lamportsToSOL(lamports: number): string {
  return (lamports / 1_000_000_000).toFixed(9);
}

function formatLamports(lamports: number): string {
  return lamports.toLocaleString("en-US");
}

async function getBalance(connection: Connection, pubkey: PublicKey): Promise<number> {
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
  console.log("📊 BÁO CÁO CHI PHÍ - TIẾP TỤC TỪ GIAI ĐOẠN 2");
  console.log("═══════════════════════════════════════════════════════════════\n");

  const clusterUrl = "https://api.testnet.solana.com";
  const connection = new Connection(clusterUrl, "confirmed");

  const walletPath = path.resolve(os.homedir(), ".config/solana/id.json");
  const walletKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(walletPath, "utf-8")))
  );
  const walletPubkey = walletKeypair.publicKey;
  const projectRoot = path.resolve(__dirname, "..");
  const configPath = path.join(__dirname, "config.json");

  console.log(`Wallet: ${walletPubkey.toBase58()}`);

  // ─── Data from Stage 1 (already completed) ────────────────────────────────

  const programId = new PublicKey("2rBEttbbLFtXpfkQZuUj3iXoCtE5ZWcMUCtkAARm8yoK");
  const stage1: StageResult = {
    name: "Giai đoạn 1: Deploy Program (Triển khai Smart Contract)",
    balanceBefore: 9_000_000_000,
    balanceAfter: 6_622_266_746,
    cost: 2_377_733_254,
    details: [],
  };

  // Compute stage 1 details from on-chain data
  const programDataSize = 341032; // from solana program show
  const soFileSize = 341032;
  const rentForProgram = await getRentExemption(connection, programDataSize);
  // Program account itself (36 bytes) also needs rent
  const rentForProgramAccount = await getRentExemption(connection, 36);
  const txFee1 = stage1.cost - rentForProgram - rentForProgramAccount;
  
  stage1.details = [
    `Tiền thuê (Rent-exempt)|Lưu trữ file thực thi chương trình (ProgramData Account: \`CbATknTLyPofk84w1651s5KeoqA5o1rZwGAmVJGwFzGY\`)|${formatLamports(programDataSize)} bytes|${formatLamports(rentForProgram)}|${lamportsToSOL(rentForProgram)} SOL`,
    `Tiền thuê (Rent-exempt)|Program Account (\`${programId.toBase58()}\`)|36 bytes|${formatLamports(rentForProgramAccount)}|${lamportsToSOL(rentForProgramAccount)} SOL`,
    `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch deploy trên blockchain (bao gồm priority fee)|-|${formatLamports(txFee1)}|${lamportsToSOL(txFee1)} SOL`,
  ];

  const stages: StageResult[] = [stage1];

  // ─── GIAI ĐOẠN 2: Create Token ────────────────────────────────────────────

  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
  console.log("🪙 GIAI ĐOẠN 2: Tạo SPL Token (Mint + ATA + Mint Supply)");
  console.log("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

  // Reset config.json
  const freshConfig: any = { CLUSTER_URL: clusterUrl };
  fs.writeFileSync(configPath, JSON.stringify(freshConfig, null, 2));

  const balanceBefore2 = await getBalance(connection, walletPubkey);
  console.log(`  Số dư TRƯỚC: ${formatLamports(balanceBefore2)} lamports (${lamportsToSOL(balanceBefore2)} SOL)`);

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

  await sleep(5000);
  const balanceAfter2 = await getBalance(connection, walletPubkey);
  const cost2 = balanceBefore2 - balanceAfter2;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter2)} lamports (${lamportsToSOL(balanceAfter2)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost2)} lamports (${lamportsToSOL(cost2)} SOL)`);

  const configAfterToken = JSON.parse(fs.readFileSync(configPath, "utf-8"));
  const tokenMint = configAfterToken.TOKEN_MINT;
  const adminAta = configAfterToken.ADMIN_TOKEN_ACCOUNT;

  const rentMint = await getRentExemption(connection, 82);
  const rentATA = await getRentExemption(connection, 165);
  const txFees2 = cost2 - rentMint - rentATA;

  stages.push({
    name: "Giai đoạn 2: Tạo SPL Token (Mint + ATA + Mint Supply)",
    balanceBefore: balanceBefore2,
    balanceAfter: balanceAfter2,
    cost: cost2,
    details: [
      `Tiền thuê (Rent-exempt)|Token Mint Account (\`${tokenMint}\`)|82 bytes|${formatLamports(rentMint)}|${lamportsToSOL(rentMint)} SOL`,
      `Tiền thuê (Rent-exempt)|Admin Associated Token Account (\`${adminAta}\`)|165 bytes|${formatLamports(rentATA)}|${lamportsToSOL(rentATA)} SOL`,
      `Phí giao dịch (Tx Fee)|Phí 3 giao dịch: create-token, create-account, mint|3 TXs|${formatLamports(txFees2)}|${lamportsToSOL(txFees2)} SOL`,
    ],
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

  await sleep(5000);
  const balanceAfter3 = await getBalance(connection, walletPubkey);
  const cost3 = balanceBefore3 - balanceAfter3;
  console.log(`  Số dư SAU: ${formatLamports(balanceAfter3)} lamports (${lamportsToSOL(balanceAfter3)} SOL)`);
  console.log(`  💰 Chi phí: ${formatLamports(cost3)} lamports (${lamportsToSOL(cost3)} SOL)`);

  const configAfterInit = JSON.parse(fs.readFileSync(configPath, "utf-8"));
  const tokenVault = configAfterInit.TOKEN_VAULT;
  const rewardDistPda = configAfterInit.REWARD_DISTRIBUTOR_PDA;

  // RewardDistributor account size:
  // discriminator(8) + authority(32) + reward_mint(32) + current_root(32) + epoch_id(8) + 
  // token_vault(32) + bump(1) + is_paused(1) + last_updated_at(8) + total_distributed(8) + 
  // _reserved vec length prefix(4) + reserved data(64) = 230 bytes
  const rewardDistSize = 230;
  const tokenVaultSize = 165;

  const rentVault = await getRentExemption(connection, tokenVaultSize);
  const rentPda = await getRentExemption(connection, rewardDistSize);
  const txFee3 = cost3 - rentVault - rentPda;

  stages.push({
    name: "Giai đoạn 3: Initialize Program (Khởi tạo Cấu hình & Vault)",
    balanceBefore: balanceBefore3,
    balanceAfter: balanceAfter3,
    cost: cost3,
    details: [
      `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch khởi tạo (gồm 2 chữ ký: \`payer\` + \`token_vault\`)|-|${formatLamports(txFee3)}|${lamportsToSOL(txFee3)} SOL`,
      `Tiền thuê (Rent-exempt)|Token Vault (\`${tokenVault}\`)|${tokenVaultSize} bytes|${formatLamports(rentVault)}|${lamportsToSOL(rentVault)} SOL`,
      `Tiền thuê (Rent-exempt)|Reward Distributor PDA (\`${rewardDistPda}\`)|~${rewardDistSize} bytes|${formatLamports(rentPda)}|${lamportsToSOL(rentPda)} SOL`,
    ],
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

  await sleep(5000);
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
      `Phí giao dịch (Tx Fee)|Phí xử lý giao dịch transfer SPL token (1 chữ ký)|1 TX|${formatLamports(cost4)}|${lamportsToSOL(cost4)} SOL`,
    ],
  });

  // ─── Generate Markdown Report ─────────────────────────────────────────────

  console.log("\n\n");
  console.log("═══════════════════════════════════════════════════════════════");
  console.log("📝 ĐANG TẠO BÁO CÁO MARKDOWN...");
  console.log("═══════════════════════════════════════════════════════════════\n");

  const totalCost = stages.reduce((sum, s) => sum + s.cost, 0);

  let report = `# Báo cáo Chi phí Triển khai & Thiết lập Botanika Rewards Distributor trên Solana

Báo cáo chi tiết số dư và phân bổ chi phí thực tế thu thập từ đợt chạy trên mạng **Solana Testnet** cho Program ID \`${programId.toBase58()}\` (Botanika Rewards Distributor).

- **Ngày chạy:** ${new Date().toLocaleDateString("vi-VN")}
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

  report += `| **TỔNG CỘNG** | ${formatLamports(stages[0].balanceBefore)} lamports | ${formatLamports(stages[stages.length - 1].balanceAfter)} lamports | **${formatLamports(totalCost)} lamports** | **${lamportsToSOL(totalCost)} SOL** |\n`;

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
  report += `- **Phí giao dịch (Tx Fee):** Đây là phí cố định trên Solana, thường là 5,000 lamports/chữ ký. Priority fee (\`--with-compute-unit-price 5000\`) sẽ tăng thêm chi phí.\n`;
  report += `- Kích thước file .so (program binary): **${soFileSize.toLocaleString()} bytes**\n`;
  report += `- Deploy signature: \`r8QguqznVyeERaREG3Q759mjxHSuj4UJm9oqv4ejc7ia1nqaMNvcHLTddJWzwQh9RgKDG5Ut32iCP4pKhfcW7jp\`\n`;

  const reportPath = path.join(projectRoot, "COST_REPORT.md");
  fs.writeFileSync(reportPath, report);
  console.log(`✅ Báo cáo đã được lưu tại: ${reportPath}`);
  console.log("\n" + report);
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
