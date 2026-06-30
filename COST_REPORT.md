# Báo cáo Chi phí Triển khai & Thiết lập Botanika Rewards Distributor trên Solana

Báo cáo chi tiết số dư và phân bổ chi phí thực tế thu thập từ đợt chạy trên mạng **Solana Testnet** cho Program ID `2rBEttbbLFtXpfkQZuUj3iXoCtE5ZWcMUCtkAARm8yoK` (Botanika Rewards Distributor).

- **Ngày chạy:** 30/6/2026
- **Mạng:** Solana Testnet (`https://api.testnet.solana.com`)
- **Wallet:** `Caamp2A7VYy9M9oUgQyoJWgYZfmC4XFs9W9gyWL53ECA`

---

## 1. Bảng Tổng hợp Chi phí qua các Giai đoạn

| Giai đoạn | Số dư Trước khi chạy | Số dư Sau khi chạy | Chi phí Tiêu tốn (Lamports) | Chi phí Tiêu tốn (SOL) |
| --- | --- | --- | --- | --- |
| **Giai đoạn 1: Deploy Program (Triển khai Smart Contract)** | 9,000,000,000 lamports | 6,622,266,746 lamports | 2,377,733,254 lamports | 2.377733254 SOL |
| **Giai đoạn 2: Tạo SPL Token (Mint + ATA + Mint Supply)** | 6,622,266,746 lamports | 6,618,745,866 lamports | 3,520,880 lamports | 0.003520880 SOL |
| **Giai đoạn 3: Initialize Program (Khởi tạo Cấu hình & Vault)** | 6,618,745,866 lamports | 6,614,232,746 lamports | 4,513,120 lamports | 0.004513120 SOL |
| **Giai đoạn 4: Chuyển Token vào Vault** | 6,614,232,746 lamports | 6,614,227,746 lamports | 5,000 lamports | 0.000005000 SOL |
| **TỔNG CỘNG** | 9,000,000,000 lamports | 6,614,227,746 lamports | **2,385,772,254 lamports** | **2.385772254 SOL** |

---

## 2. Chi tiết Phân bổ Chi phí từng Giai đoạn

### Giai đoạn 1: Deploy Program (Triển khai Smart Contract)

- **Tổng chi phí:** `2,377,733,254 lamports` (2.377733254 SOL)

| Phân loại | Tài khoản / Mục đích | Kích thước | Chi phí (Lamports) | Chi phí (SOL) |
| --- | --- | --- | --- | --- |
| **Tiền thuê (Rent-exempt)** | Lưu trữ file thực thi chương trình (ProgramData Account: `CbATknTLyPofk84w1651s5KeoqA5o1rZwGAmVJGwFzGY`) | 341,032 bytes | 2,374,473,600 | 2.374473600 SOL |
| **Tiền thuê (Rent-exempt)** | Program Account (`2rBEttbbLFtXpfkQZuUj3iXoCtE5ZWcMUCtkAARm8yoK`) | 36 bytes | 1,141,440 | 0.001141440 SOL |
| **Phí giao dịch (Tx Fee)** | Phí xử lý giao dịch deploy trên blockchain (bao gồm priority fee) | - | 2,118,214 | 0.002118214 SOL |
| **Tổng cộng** |  |  | **2,377,733,254** | **2.377733254 SOL** |

---

### Giai đoạn 2: Tạo SPL Token (Mint + ATA + Mint Supply)

- **Tổng chi phí:** `3,520,880 lamports` (0.003520880 SOL)

| Phân loại | Tài khoản / Mục đích | Kích thước | Chi phí (Lamports) | Chi phí (SOL) |
| --- | --- | --- | --- | --- |
| **Tiền thuê (Rent-exempt)** | Token Mint Account (`61Qb25DYPg2cgGzvTSFVgQZF4449HogfCauyd9BC4nUM`) | 82 bytes | 1,461,600 | 0.001461600 SOL |
| **Tiền thuê (Rent-exempt)** | Admin Associated Token Account (`35jECw5mDL3D2NnpxuwEsD3FWvHqtjjAG7QKYh2FbwMd`) | 165 bytes | 2,039,280 | 0.002039280 SOL |
| **Phí giao dịch (Tx Fee)** | Phí 3 giao dịch: create-token, create-account, mint | 3 TXs | 20,000 | 0.000020000 SOL |
| **Tổng cộng** |  |  | **3,520,880** | **0.003520880 SOL** |

---

### Giai đoạn 3: Initialize Program (Khởi tạo Cấu hình & Vault)

- **Tổng chi phí:** `4,513,120 lamports` (0.004513120 SOL)

| Phân loại | Tài khoản / Mục đích | Kích thước | Chi phí (Lamports) | Chi phí (SOL) |
| --- | --- | --- | --- | --- |
| **Phí giao dịch (Tx Fee)** | Phí xử lý giao dịch khởi tạo (gồm 2 chữ ký: `payer` + `token_vault`) | - | 10,000 | 0.000010000 SOL |
| **Tiền thuê (Rent-exempt)** | Token Vault (`2tsz4R7v81k8UQPtDW6Yo2QYPTvfTipdxD96hrPWjeWA`) | 165 bytes | 2,039,280 | 0.002039280 SOL |
| **Tiền thuê (Rent-exempt)** | Reward Distributor PDA (`4yxGZ2hjfqorXNK1sxKMhna9WHsPpwy44YVXdd3uZgxU`) | 226 bytes | 2,463,840 | 0.002463840 SOL |
| **Tổng cộng** |  |  | **4,513,120** | **0.004513120 SOL** |

---

### Giai đoạn 4: Chuyển Token vào Vault

- **Tổng chi phí:** `5,000 lamports` (0.000005000 SOL)

| Phân loại | Tài khoản / Mục đích | Kích thước | Chi phí (Lamports) | Chi phí (SOL) |
| --- | --- | --- | --- | --- |
| **Phí giao dịch (Tx Fee)** | Phí xử lý giao dịch transfer SPL token (1 chữ ký) | 1 TX | 5,000 | 0.000005000 SOL |
| **Tổng cộng** |  |  | **5,000** | **0.000005000 SOL** |

---

## 3. Ghi chú

- **Tiền thuê (Rent-exempt):** Đây là khoản SOL phải ký quỹ để giữ tài khoản trên blockchain vĩnh viễn. Khoản này có thể thu hồi khi đóng tài khoản.
- **Phí giao dịch (Tx Fee):** Đây là phí cố định trên Solana, thường là 5,000 lamports/chữ ký. Priority fee (`--with-compute-unit-price 5000`) sẽ tăng thêm chi phí.
- Kích thước file .so (program binary): **341,032 bytes**
- Deploy signature: `r8QguqznVyeERaREG3Q759mjxHSuj4UJm9oqv4ejc7ia1nqaMNvcHLTddJWzwQh9RgKDG5Ut32iCP4pKhfcW7jp`
