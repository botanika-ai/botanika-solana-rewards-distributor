# Botanika Solana Rewards Distributor

Dự án này là smart contract phân phối phần thưởng (Rewards Distributor) dựa trên cơ chế **Merkle Tree** viết bằng **Anchor Framework** trên blockchain Solana.

Tài liệu này hướng dẫn chi tiết từng bước từ cài đặt, build, deploy cho đến cách chạy các script để thiết lập một smart contract phân phối phần thưởng hoàn chỉnh và sẵn sàng sử dụng.

---

## 📋 Yêu Cầu Hệ Thống (Prerequisites)

Trước khi bắt đầu, hãy đảm bảo máy tính của bạn đã cài đặt các công cụ sau:
1. **Node.js** (v18 trở lên) & **Yarn** hoặc **NPM**.
2. **Rust** & **Cargo** (phiên bản ổn định).
3. **Solana CLI Suite** (ví dụ: `solana`, `spl-token`).
4. **Anchor CLI** (phiên bản `0.30.1` để tương thích tốt nhất).

### Thiết lập Ví Solana Local
Đảm bảo bạn đã có ví local để deploy và ký giao dịch:
```bash
# Kiểm tra ví hiện tại
solana address

# Nếu chưa có ví, hãy tạo mới:
solana-keygen new --outfile ~/.config/solana/id.json

# Cấu hình CLI trỏ tới Solana Testnet (hoặc Devnet / Localnet tùy nhu cầu)
solana config set --url https://api.testnet.solana.com
# Hoặc Devnet:
# solana config set --url https://api.devnet.solana.com

# Nhận SOL miễn phí để thanh toán phí giao dịch (Airdrop)
solana airdrop 2
```

---

## 🛠️ Bước 1: Cài đặt & Build Smart Contract

1. **Cài đặt các thư viện Node dependencies:**
   ```bash
   yarn install
   # hoặc: npm install
   ```

2. **Đồng bộ hóa Program ID:**
   Khi bạn clone dự án hoặc tạo ví program mới, hãy chạy lệnh đồng bộ để Anchor tự động cập nhật Program ID chính xác vào source code Rust và file `Anchor.toml`:
   ```bash
   yarn sync
   # hoặc: anchor keys sync
   ```

3. **Biên dịch (Build) dự án:**
   ```bash
   yarn build
   # hoặc: anchor build
   ```
   *Lưu ý:* Lệnh build sẽ tạo ra thư mục `target/idl/` chứa file mô tả giao diện smart contract (IDL) và tự động ghi Program ID chính xác vào trường `address` trong IDL.

---

## 🚀 Bước 2: Deploy Smart Contract lên On-Chain

Chạy lệnh deploy để đưa contract lên mạng Solana (ví dụ: Testnet/Devnet):
```bash
yarn deploy
# Lệnh này tương đương: anchor deploy -- --max-sign-attempts 100 --with-compute-unit-price 50000
```

Sau khi deploy thành công, terminal sẽ hiển thị địa chỉ Program ID của bạn.

---

## 📝 Bước 3: Chạy các Scripts để Thiết Lập Hệ Thống Phân Phối

Các script hỗ trợ được đặt trong thư mục `scripts/`. Để tối ưu hóa và tránh lỗi cấu hình chéo giữa các mạng, hệ thống sử dụng file `scripts/config.json` làm **nguồn dữ liệu cấu hình chung (Single Source of Truth)**.

### ⚡ Phím Tắt: Tự động hóa toàn bộ (Build, Deploy, Public IDL, và thiết lập Bước 1, 2, 3)
Nếu muốn thực hiện nhanh toàn bộ quá trình biên dịch (Build), triển khai smart contract (Deploy), công khai mô tả giao diện contract (Public IDL on-chain), tạo Token mới, khởi tạo Distributor và chuyển token vào Vault chỉ bằng một lệnh duy nhất:
```bash
yarn setup
# hoặc: npm run setup
# hoặc: npx ts-node scripts/setup-distributor.ts
```

---

### Chi tiết từng bước (nếu chạy riêng lẻ):

### 1. Tạo SPL Token (Reward Token)
Script này sẽ tạo ra một token mới có **9 decimals** (yêu cầu bắt buộc của contract Botanika), tạo tài khoản Associated Token Account (ATA) cho admin, mint số lượng token ban đầu, và lưu thông tin vào `scripts/config.json`.
```bash
npx ts-node scripts/create-token.ts
```
*Đầu ra:* file `scripts/config.json` được khởi tạo chứa các khóa `CLUSTER_URL`, `TOKEN_MINT`, `ADMIN_TOKEN_ACCOUNT`, `INITIAL_SUPPLY`.

### 2. Khởi Tạo Reward Distributor
Script này đọc cấu hình token từ `config.json`, tự động lấy Program ID từ file IDL đã được biên dịch, sau đó gửi giao dịch `initialize` lên on-chain để tạo Reward Distributor PDA, một Token Vault và một Lock Vault mới.
```bash
npx ts-node scripts/initialize.ts
```
*Đầu ra:* File `scripts/config.json` sẽ được cập nhật thêm:
* `REWARD_DISTRIBUTOR_PDA`: Địa chỉ PDA quản lý phân phối.
* `TOKEN_VAULT`: Địa chỉ kho chứa token phần thưởng.
* `TOKEN_VAULT_SECRET`: Private key của vault (được ký lúc khởi tạo).
* `LOCK_VAULT`: Địa chỉ kho giữ các claim đã finalize trong thời gian khoá 60 ngày (xem mục "Luồng Claim Chính" bên dưới).
* `PROGRAM_ID`: Địa chỉ Program thực tế.

### 3. Chuyển Token vào Vault của Contract
Để miners có thể claim phần thưởng, Token Vault của contract cần phải có số dư token. Script này thực hiện chuyển 500.000 tokens từ tài khoản admin ATA sang tài khoản `TOKEN_VAULT` của distributor.
```bash
npx ts-node scripts/transfer-to-vault.ts
```
*Đầu ra:* Chuyển khoản thành công và ghi nhận lại số dư xác minh `VAULT_BALANCE` vào `config.json`.

### 4. Tạo Merkle Tree & Proofs (Off-Chain)
Script này tạo dữ liệu phần thưởng giả định cho các thợ đào (Miners) và tính toán cây Merkle (Merkle Tree). Nó sẽ tạo ngẫu nhiên ví cho Miner 1 & Miner 2 (nếu chưa có trong cấu hình), hash các phần thưởng, tính toán root hash và tạo Merkle Proofs cho từng thợ đào.
```bash
npx ts-node scripts/generate-merkle.ts
```
*Đầu ra:* Kết quả được lưu vào file `scripts/merkle-output.json` bao gồm:
* `MERKLE_ROOT`: Mảng byte của Merkle root.
* `PROOFS`: Các mảng chứng thực (proof) cần thiết để mỗi Miner claim token.

### 5. ~~Cập Nhật Merkle Root lên On-Chain~~ — OBSOLETE, đã xoá `scripts/update-root.ts`
`update_root` giờ chỉ chạy được qua **Magic Action** do `botanika-magicblock-contracts`'s
`commit_epoch_state` schedule (Design Freeze v1 §5.1/§6.7) — `root_authority` đã khoá vào
`escrow_auth` PDA của chương trình đó, không ví người nào (kể cả Authority admin) ký được nữa.
Không còn script admin nào gọi trực tiếp instruction này; xem `botanika-magicblock-contracts/scripts/devnet-t1-fault-injection.ts`
để biết luồng gọi thật qua ER. `claim_reward` (bước 6 dưới) vẫn là cold fallback hợp lệ
(PAD v1.0 §11.3) dùng chung Merkle root/proof từ bước 4, không phụ thuộc bước này.

### 6. Kiểm Tra Trạng Thái Hiện Tại (On-Chain Status)
Bạn có thể kiểm tra nhanh thông tin thực tế đang được lưu trên mạng Solana của Reward Distributor PDA (bao gồm: Authority, Reward Mint, Token Vault, Merkle Root hiện tại, tổng số đã phân phối, trạng thái Pause):
```bash
npx ts-node scripts/view-state.ts
```

---

## 🔐 Luồng Claim Chính: `finalize_claim` → Lock 60 ngày → `withdraw_unlocked`

`finalize_claim` (chỉ chạy được qua Magic Action từ `botanika-magicblock-contracts`'s
`commit_reward_balance`) **không chuyển token thẳng vào ví beneficiary** nữa:

1. Chuyển `amount` từ `token_vault` sang `lock_vault` (token account do Reward Distributor PDA làm authority).
2. Tạo `ClaimReceipt` (`paid_at` = thời điểm finalize) và một `ClaimLock` riêng cho claim đó
   (seed `["claim_lock", claim_receipt]`) với `unlock_at = locked_at + 5_184_000` giây (60 ngày, theo clock của chain).
3. Cộng `RewardDistributor.total_locked`.

> **Lưu ý môi trường:** mặc định `LOCK_DURATION_SECS = 5_184_000` (60 ngày, production).
> Bản deploy **local và devnet (dev) cố ý khoá 300 giây (5 phút)** để test được trọn vòng claim → withdraw:
> build bằng feature `short-lock` (`anchor build -- --features short-lock`). Devnet
> `ABqbuKRLFJyshpVj85yNBXqhX1iae2qvTFRYq3VUGh7a` đang chạy bản 5 phút. `reward-service` phải đặt
> `CLAIM_LOCK_DURATION_SECS=300` khi trỏ vào bản này (mặc định 60 ngày), nếu không listener từ chối mọi `ClaimFinalized`.
> Core chỉ dùng lock đã mở khoá ít nhất 5 phút, nên trên dev một claim rút được sau khoảng 10 phút.

Thứ tự account của `finalize_claim`: `reward_distributor (mut), settlement, claim_receipt (init), claim_lock (init),
token_vault (mut), lock_vault (mut), reward_mint, token_program, system_program`, sau đó là `caller_program, escrow_auth, escrow`
do Magic Action thêm vào.

Sau khi hết hạn khoá, **beneficiary tự ký** `withdraw_unlocked(amounts: Vec<u64>)` để rút một phần hoặc toàn bộ:
* Accounts: `reward_distributor (mut), beneficiary (signer), lock_vault (mut), destination_token_account (mut, đúng mint, owner bất kỳ), reward_mint, token_program`;
  `remaining_accounts` = các `ClaimLock` PDA (writable), `amounts[i]` tương ứng với lock thứ `i`.
* Điều kiện on-chain: 1–16 lock, không trùng lock, mỗi lock thuộc đúng beneficiary ký, `now >= unlock_at`,
  `0 < amounts[i] <= amount - withdrawn`, tổng `>= 100 BONSAI` (`MIN_LOCK_WITHDRAW = 100_000_000_000`), chương trình không bị pause.
* Chọn lock nào rút trước (FIFO) là quy tắc off-chain do bên dựng giao dịch quyết định (ví dụ botanika-core cho payout VND).
* Phát event `LockWithdrawn { beneficiary, destination, total, locks, amounts, timestamp }`.

Lưu ý: `claim_reward`/`batch_payout` (legacy, mặc định tắt) vẫn trả thẳng `token_vault → ví miner`;
nếu bật lại qua `set_legacy_claims_enabled` thì các luồng này **bỏ qua cơ chế khoá**.

## 🛠️ Quá Trình Nhận Thưởng Legacy (`claim_reward`, mặc định tắt)

Khi `legacy_claims_enabled` được bật (đã nạp token vào Vault và đã có Merkle Root on-chain), các thợ đào (Miners) có thể nhận thưởng bằng cách gọi transaction `claim_reward` trên client của họ:

1. Đọc dữ liệu chứng thực của miner từ `scripts/merkle-output.json` (phím tương ứng là `"MINER_PUBKEY:NODE_ID"`).
2. Gửi lệnh `claimReward` với các tham số:
   * `node_id_hash`: Hash của ID node (dạng 32 bytes).
   * `cumulative_amount`: Tổng số lượng token tích lũy mà miner được nhận từ trước đến nay (dạng `u64`).
   * `proof`: Mảng các hash bytes chứng thực từ cây Merkle.
3. Smart contract sẽ tự động kiểm tra chứng thực, so khớp lượng token đã claim trước đó và chuyển phần chênh lệch trực tiếp về ví của Miner (không qua lock).

---

## 🔒 Quản Trị & Rút Token Khi Cần Thiết (Sweep/Withdraw)

Smart contract cũng hỗ trợ tính năng **Withdraw Vault** để admin rút lại token chưa được claim từ vault về treasury nhằm mục đích hoàn trả quỹ hoặc tái phân bổ:
* Gọi lệnh `withdraw_vault` ký bởi Authority đã cấu hình.
* Truyền vào `amount` cần rút hoặc `u64::MAX` để rút toàn bộ số dư còn lại trong vault.
* `withdraw_vault` chỉ chấp nhận `token_vault`; `lock_vault` (token của các claim đang khoá/đã mở khoá) không bao giờ rút được bằng lệnh này.