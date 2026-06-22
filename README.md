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
Script này đọc cấu hình token từ `config.json`, tự động lấy Program ID từ file IDL đã được biên dịch, sau đó gửi giao dịch `initialize` lên on-chain để tạo Reward Distributor PDA và một Token Vault mới.
```bash
npx ts-node scripts/initialize.ts
```
*Đầu ra:* File `scripts/config.json` sẽ được cập nhật thêm:
* `REWARD_DISTRIBUTOR_PDA`: Địa chỉ PDA quản lý phân phối.
* `TOKEN_VAULT`: Địa chỉ kho chứa token phần thưởng.
* `TOKEN_VAULT_SECRET`: Private key của vault (được ký lúc khởi tạo).
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

### 5. Cập Nhật Merkle Root lên On-Chain
Khi đã có Merkle Root off-chain, quản trị viên (Authority) cần đưa Root này lên smart contract thông qua giao dịch `updateRoot` để kích hoạt đợt phân phối mới.
```bash
npx ts-node scripts/update-root.ts
```
*Đầu ra:* Giao dịch thành công, on-chain state được cập nhật với Merkle Root mới nhất và phiên bản Root Version tăng lên.

### 6. Kiểm Tra Trạng Thái Hiện Tại (On-Chain Status)
Bạn có thể kiểm tra nhanh thông tin thực tế đang được lưu trên mạng Solana của Reward Distributor PDA (bao gồm: Authority, Reward Mint, Token Vault, Merkle Root hiện tại, tổng số đã phân phối, trạng thái Pause):
```bash
npx ts-node scripts/view-state.ts
```

---

## 🛠️ Quá Trình Nhận Thưởng (Claim Reward) Cho Miners

Khi hệ thống đã sẵn sàng (đã nạp token vào Vault và đã cập nhật Merkle Root lên on-chain), các thợ đào (Miners) có thể nhận thưởng bằng cách gọi transaction `claim_reward` trên client của họ:

1. Đọc dữ liệu chứng thực của miner từ `scripts/merkle-output.json` (phím tương ứng là `"MINER_PUBKEY:NODE_ID"`).
2. Gửi lệnh `claimReward` với các tham số:
   * `node_id_hash`: Hash của ID node (dạng 32 bytes).
   * `cumulative_amount`: Tổng số lượng token tích lũy mà miner được nhận từ trước đến nay (dạng `u64`).
   * `proof`: Mảng các hash bytes chứng thực từ cây Merkle.
3. Smart contract sẽ tự động kiểm tra chứng thực, so khớp lượng token đã claim trước đó và chuyển phần chênh lệch trực tiếp về ví của Miner.

---

## 🔒 Quản Trị & Rút Token Khi Cần Thiết (Sweep/Withdraw)

Smart contract cũng hỗ trợ tính năng **Withdraw Vault** để admin rút lại token chưa được claim từ vault về treasury nhằm mục đích hoàn trả quỹ hoặc tái phân bổ:
* Gọi lệnh `withdraw_vault` ký bởi Authority đã cấu hình.
* Truyền vào `amount` cần rút hoặc `u64::MAX` để rút toàn bộ số dư còn lại trong vault.