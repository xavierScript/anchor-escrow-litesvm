#[cfg(test)]
mod tests {

    use {
        anchor_lang::{
            prelude::msg,
            solana_program::{instruction::Instruction, program_pack::Pack},
            system_program::ID as SYSTEM_PROGRAM_ID,
            AccountDeserialize, InstructionData, ToAccountMetas,
        },
        anchor_spl::{
            associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
            token::spl_token,
        },
        litesvm::LiteSVM,
        litesvm_token::{
            spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
        },
        solana_account::Account,
        solana_keypair::Keypair,
        solana_message::Message,
        solana_pubkey::Pubkey,
        solana_rpc_client::rpc_client::RpcClient,
        solana_signer::Signer,
        solana_transaction::Transaction,
        std::str::FromStr,
    };

    // Setup function to initialize LiteSVM and create a payer keypair
    fn setup() -> (LiteSVM, Keypair) {
        let program_id = anchor_escrow_q2_2026::id();
        let payer = Keypair::new();
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!("../../../target/deploy/anchor_escrow_q2_2026.so");
        svm.add_program(program_id, bytes).unwrap();
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

        // Example on how to Load an account from devnet
        // LiteSVM does not have access to real Solana network data since it does not have network access,
        // so we use an RPC client to fetch account data from devnet
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");
        let account_address =
            Pubkey::from_str("DRYvf71cbF2s5wgaJQvAGkghMkRcp5arvsK2w97vXhi2").unwrap();
        let fetched_account = rpc_client
            .get_account(&account_address)
            .expect("Failed to fetch account from devnet");

        // Set the fetched account in the LiteSVM environment
        // This allows us to simulate interactions with this account during testing
        svm.set_account(
            payer.pubkey(),
            Account {
                lamports: fetched_account.lamports,
                data: fetched_account.data,
                owner: Pubkey::from(fetched_account.owner.to_bytes()),
                executable: fetched_account.executable,
                rent_epoch: fetched_account.rent_epoch,
            },
        )
        .unwrap();

        msg!("Lamports of fetched account: {}", fetched_account.lamports);

        // Return the LiteSVM instance and payer keypair
        (svm, payer)
    }

    #[test]
    fn test_make() {
        // Setup the test environment by initializing LiteSVM and creating a payer keypair
        let (mut program, payer) = setup();

        // Get the maker's public key from the payer keypair
        let maker = payer.pubkey();

        // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
        // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint B: {}\n", mint_b);

        // Create the maker's associated token account for Mint A
        // This is done using litesvm-token's CreateAssociatedTokenAccount utility
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        // Derive the PDA for the escrow account using the maker's public key and a seed value
        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &anchor_escrow_q2_2026::id(),
        )
        .0;
        msg!("Escrow PDA: {}\n", escrow);

        // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();

        // Create the "Make" instruction to deposit tokens into the escrow
        let make_ix = Instruction {
            program_id: anchor_escrow_q2_2026::id(),
            accounts: anchor_escrow_q2_2026::accounts::Make {
                maker: maker,
                mint_a: mint_a,
                mint_b: mint_b,
                maker_ata_a: maker_ata_a,
                escrow: escrow,
                vault: vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
        };

        // Create and send the transaction containing the "Make" instruction
        let message = Message::new(&[make_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&payer], message, recent_blockhash);

        // Send the transaction and capture the result
        let tx = program.send_transaction(transaction).unwrap();

        // Log transaction details
        msg!("\n\nMake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        // Verify the vault account and escrow account data after the "Make" instruction
        let vault_account = program.get_account(&vault).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);
        assert_eq!(vault_data.owner, escrow);
        assert_eq!(vault_data.mint, mint_a);

        let escrow_account = program.get_account(&escrow).unwrap();
        let escrow_data = anchor_escrow_q2_2026::state::Escrow::try_deserialize(
            &mut escrow_account.data.as_ref(),
        )
        .unwrap();
        assert_eq!(escrow_data.seed, 123u64);
        assert_eq!(escrow_data.maker, maker);
        assert_eq!(escrow_data.mint_a, mint_a);
        assert_eq!(escrow_data.mint_b, mint_b);
        assert_eq!(escrow_data.receive, 10);
    }

    #[test]
    fn test_take() {
        // Setup the test environment
        let (mut program, maker_keypair) = setup();

        let maker = maker_keypair.pubkey();

        // Create a separate taker keypair and fund it
        let taker_keypair = Keypair::new();
        program
            .airdrop(&taker_keypair.pubkey(), 1_000_000_000)
            .unwrap();
        let taker = taker_keypair.pubkey();

        // Create Mint A and Mint B with 6 decimals, maker as authority
        let mint_a = CreateMint::new(&mut program, &maker_keypair)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &maker_keypair)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint B: {}\n", mint_b);

        // Create maker's ATA for Mint A
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &maker_keypair, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        // Mint 1000 tokens of Mint A to maker
        MintTo::new(&mut program, &maker_keypair, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();

        // Create taker's ATA for Mint B
        let taker_ata_b =
            CreateAssociatedTokenAccount::new(&mut program, &taker_keypair, &mint_b)
                .owner(&taker)
                .send()
                .unwrap();
        msg!("Taker ATA B: {}\n", taker_ata_b);

        // Mint 1000 tokens of Mint B to taker
        MintTo::new(&mut program, &maker_keypair, &mint_b, &taker_ata_b, 1000000000)
            .send()
            .unwrap();

        // Derive the escrow PDA
        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &anchor_escrow_q2_2026::id(),
        )
        .0;
        msg!("Escrow PDA: {}\n", escrow);

        // Derive the vault ATA (escrow-owned ATA for Mint A)
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        // ---- MAKE: deposit 10 of Mint A into the vault ----
        let make_ix = Instruction {
            program_id: anchor_escrow_q2_2026::id(),
            accounts: anchor_escrow_q2_2026::accounts::Make {
                maker: maker,
                mint_a: mint_a,
                mint_b: mint_b,
                maker_ata_a: maker_ata_a,
                escrow: escrow,
                vault: vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
        };

        let message = Message::new(&[make_ix], Some(&maker_keypair.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&maker_keypair], message, recent_blockhash);
        program.send_transaction(transaction).unwrap();
        msg!("\n\nMake transaction sucessfull (before take)");

        // ---- TAKE: taker sends 10 of Mint B to maker, receives 10 of Mint A from vault ----

        // Derive taker's ATA for Mint A (will be init_if_needed by the program)
        let taker_ata_a = associated_token::get_associated_token_address(&taker, &mint_a);

        // Derive maker's ATA for Mint B (will be init_if_needed by the program)
        let maker_ata_b = associated_token::get_associated_token_address(&maker, &mint_b);

        let take_ix = Instruction {
            program_id: anchor_escrow_q2_2026::id(),
            accounts: anchor_escrow_q2_2026::accounts::Take {
                taker: taker,
                maker: maker,
                mint_a: mint_a,
                mint_b: mint_b,
                taker_ata_a: taker_ata_a,
                taker_ata_b: taker_ata_b,
                maker_ata_b: maker_ata_b,
                escrow: escrow,
                vault: vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Take {}.data(),
        };

        let message = Message::new(&[take_ix], Some(&taker_keypair.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&taker_keypair], message, recent_blockhash);
        let tx = program.send_transaction(transaction).unwrap();

        msg!("\n\nTake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        // Verify taker received 10 of Mint A
        let taker_ata_a_account = program.get_account(&taker_ata_a).unwrap();
        let taker_ata_a_data =
            spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
        assert_eq!(taker_ata_a_data.amount, 10);
        assert_eq!(taker_ata_a_data.owner, taker);
        assert_eq!(taker_ata_a_data.mint, mint_a);

        // Verify maker received 10 of Mint B
        let maker_ata_b_account = program.get_account(&maker_ata_b).unwrap();
        let maker_ata_b_data =
            spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
        assert_eq!(maker_ata_b_data.amount, 10);
        assert_eq!(maker_ata_b_data.owner, maker);
        assert_eq!(maker_ata_b_data.mint, mint_b);

        // Verify the vault is closed (account should not exist anymore)
        let vault_account = program.get_account(&vault);
        assert!(vault_account.is_none(), "Vault should be closed after take");

        // Verify the escrow is closed (account should not exist anymore)
        let escrow_account = program.get_account(&escrow);
        assert!(
            escrow_account.is_none(),
            "Escrow should be closed after take"
        );
    }

    #[test]
    fn test_refund() {
        // Setup the test environment
        let (mut program, maker_keypair) = setup();

        let maker = maker_keypair.pubkey();

        // Create Mint A and Mint B with 6 decimals, maker as authority
        let mint_a = CreateMint::new(&mut program, &maker_keypair)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &maker_keypair)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint B: {}\n", mint_b);

        // Create maker's ATA for Mint A
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &maker_keypair, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        // Mint 1000 tokens of Mint A to maker
        MintTo::new(&mut program, &maker_keypair, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();

        // Derive the escrow PDA
        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &anchor_escrow_q2_2026::id(),
        )
        .0;
        msg!("Escrow PDA: {}\n", escrow);

        // Derive the vault ATA (escrow-owned ATA for Mint A)
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        // Check maker's balance before make
        let maker_ata_a_account_before = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_data_before =
            spl_token::state::Account::unpack(&maker_ata_a_account_before.data).unwrap();
        let maker_balance_before = maker_ata_a_data_before.amount;
        msg!("Maker balance before make: {}", maker_balance_before);

        // ---- MAKE: deposit 10 of Mint A into the vault ----
        let make_ix = Instruction {
            program_id: anchor_escrow_q2_2026::id(),
            accounts: anchor_escrow_q2_2026::accounts::Make {
                maker: maker,
                mint_a: mint_a,
                mint_b: mint_b,
                maker_ata_a: maker_ata_a,
                escrow: escrow,
                vault: vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
        };

        let message = Message::new(&[make_ix], Some(&maker_keypair.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&maker_keypair], message, recent_blockhash);
        program.send_transaction(transaction).unwrap();
        msg!("\n\nMake transaction sucessfull (before refund)");

        // Verify vault has 10 tokens after make
        let vault_account = program.get_account(&vault).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);

        // ---- REFUND: maker gets their tokens back ----
        let refund_ix = Instruction {
            program_id: anchor_escrow_q2_2026::id(),
            accounts: anchor_escrow_q2_2026::accounts::Refund {
                maker: maker,
                mint_a: mint_a,
                maker_ata_a: maker_ata_a,
                escrow: escrow,
                vault: vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Refund {}.data(),
        };

        let message = Message::new(&[refund_ix], Some(&maker_keypair.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&maker_keypair], message, recent_blockhash);
        let tx = program.send_transaction(transaction).unwrap();

        msg!("\n\nRefund transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        // Verify maker got their tokens back (balance should be same as before make)
        let maker_ata_a_account_after = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_data_after =
            spl_token::state::Account::unpack(&maker_ata_a_account_after.data).unwrap();
        assert_eq!(maker_ata_a_data_after.amount, maker_balance_before);

        // Verify the vault is closed (account should not exist anymore)
        let vault_account = program.get_account(&vault);
        assert!(
            vault_account.is_none(),
            "Vault should be closed after refund"
        );

        // Verify the escrow is closed (account should not exist anymore)
        let escrow_account = program.get_account(&escrow);
        assert!(
            escrow_account.is_none(),
            "Escrow should be closed after refund"
        );
    }
}
