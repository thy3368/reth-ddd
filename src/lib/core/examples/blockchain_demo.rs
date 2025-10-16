// Blockchain 功能演示示例
// 运行命令: cargo run --example blockchain_demo

use core::blockchain::Blockchain;

fn main() {
    println!("====================================");
    println!("    区块链 PoW (工作量证明) 演示");
    println!("====================================\n");

    // 1. 创建新的区块链（难度为3）
    println!("🔧 步骤 1: 创建新区块链（难度=3）");
    let mut blockchain = Blockchain::new(3);
    println!("✅ 区块链已创建，包含创世区块\n");

    // 2. 添加第一个区块（模拟 Alice 给 Bob 转账）
    println!("⛏️  步骤 2: 挖掘新区块 - Alice 给 Bob 转账 50 BTC");
    println!("   正在挖矿...");
    blockchain.add_block("Transaction: Alice -> Bob 50 BTC".to_string());
    let block1 = blockchain.last_block().unwrap();
    println!("   ✅ 挖矿成功！");
    println!("   - Nonce: {}", block1.nonce);
    println!("   - Hash: {}\n", block1.hash);

    // 3. 添加第二个区块
    println!("⛏️  步骤 3: 挖掘新区块 - Bob 给 Charlie 转账 30 BTC");
    println!("   正在挖矿...");
    blockchain.add_block("Transaction: Bob -> Charlie 30 BTC".to_string());
    let block2 = blockchain.last_block().unwrap();
    println!("   ✅ 挖矿成功！");
    println!("   - Nonce: {}", block2.nonce);
    println!("   - Hash: {}\n", block2.hash);

    // 4. 添加第三个区块
    println!("⛏️  步骤 4: 挖掘新区块 - Charlie 给 David 转账 15 BTC");
    println!("   正在挖矿...");
    blockchain.add_block("Transaction: Charlie -> David 15 BTC".to_string());
    let block3 = blockchain.last_block().unwrap();
    println!("   ✅ 挖矿成功！");
    println!("   - Nonce: {}", block3.nonce);
    println!("   - Hash: {}\n", block3.hash);

    // 5. 验证区块链的完整性
    println!("🔍 步骤 5: 验证区块链完整性");
    let is_valid = blockchain.is_valid();
    println!("   验证结果: {}", if is_valid { "✅ 有效" } else { "❌ 无效" });
    println!();

    // 6. 打印完整的区块链信息
    println!("📊 步骤 6: 显示完整区块链");
    blockchain.print_chain();

    // 7. 展示哈希链的连续性
    println!("🔗 步骤 7: 验证哈希链连续性");
    for i in 1..blockchain.len() {
        let current = blockchain.get_block(i).unwrap();
        let previous = blockchain.get_block(i - 1).unwrap();

        println!("区块 {} -> 区块 {}", i - 1, i);
        println!("  前区块哈希: {}", previous.hash);
        println!("  当前previous_hash: {}", current.previous_hash);
        println!(
            "  匹配: {}",
            if current.previous_hash == previous.hash {
                "✅"
            } else {
                "❌"
            }
        );
        println!();
    }

    // 8. 展示 PoW 的工作原理
    println!("⚙️  步骤 8: PoW (工作量证明) 解释");
    println!("难度 = 3 意味着哈希必须以 '000' 开头");
    println!();
    for (i, block) in blockchain.chain.iter().enumerate() {
        println!("区块 #{}", i);
        println!("  哈希: {}", block.hash);
        println!("  前3位: {}", &block.hash[0..3]);
        println!(
            "  符合难度要求: {}",
            if block.hash.starts_with("000") {
                "✅ 是"
            } else {
                "❌ 否"
            }
        );
        println!();
    }

    // 9. 统计信息
    println!("📈 步骤 9: 区块链统计");
    println!("  总区块数: {}", blockchain.len());
    println!("  难度级别: {}", blockchain.difficulty);

    let total_nonce: u64 = blockchain.chain.iter().map(|b| b.nonce).sum();
    println!("  总计算次数: {}", total_nonce);
    println!("  平均每区块计算次数: {}", total_nonce / blockchain.len() as u64);
    println!();

    println!("====================================");
    println!("           演示完成！");
    println!("====================================");
}
