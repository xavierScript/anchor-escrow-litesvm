use {
    anchor_lang::{
        prelude::msg,
        solana_program::instruction::Instruction,
        system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
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

/// Holds all the common accounts/keys that every escrow test needs after setup.
pub struct EscrowEnv {
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub maker_ata_a: Pubkey,
    pub escrow: Pubkey,
    pub vault: Pubkey,
    pub seed: u64,
    pub deposit: u64,
    pub receive: u64,
}

/// Initialize LiteSVM with the escrow program loaded and create a funded payer keypair.
pub fn setup() -> (LiteSVM, Keypair) {
    let program_id = anchor_escrow_q2_2026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../../target/deploy/anchor_escrow_q2_2026.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Load an account from devnet so the payer has realistic on-chain state
    let rpc_client = RpcClient::new("https://api.devnet.solana.com");
    let account_address =
        Pubkey::from_str("DRYvf71cbF2s5wgaJQvAGkghMkRcp5arvsK2w97vXhi2").unwrap();
    let fetched_account = rpc_client
        .get_account(&account_address)
        .expect("Failed to fetch account from devnet");

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

    (svm, payer)
}

/// Create mints, maker ATA, escrow PDA, vault ATA, and mint tokens to the maker.
/// Returns an `EscrowEnv` with all the derived addresses.
pub fn create_escrow_env(
    svm: &mut LiteSVM,
    maker_keypair: &Keypair,
    seed: u64,
    deposit: u64,
    receive: u64,
    mint_amount: u64,
) -> EscrowEnv {
    let maker = maker_keypair.pubkey();

    // Create Mint A and Mint B
    let mint_a = CreateMint::new(svm, maker_keypair)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint A: {}\n", mint_a);

    let mint_b = CreateMint::new(svm, maker_keypair)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    msg!("Mint B: {}\n", mint_b);

    // Create maker's ATA for Mint A
    let maker_ata_a = CreateAssociatedTokenAccount::new(svm, maker_keypair, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();
    msg!("Maker ATA A: {}\n", maker_ata_a);

    // Mint tokens to the maker's ATA
    MintTo::new(svm, maker_keypair, &mint_a, &maker_ata_a, mint_amount)
        .send()
        .unwrap();

    // Derive the escrow PDA
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
        &anchor_escrow_q2_2026::id(),
    )
    .0;
    msg!("Escrow PDA: {}\n", escrow);

    // Derive the vault ATA
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    msg!("Vault PDA: {}\n", vault);

    EscrowEnv {
        mint_a,
        mint_b,
        maker_ata_a,
        escrow,
        vault,
        seed,
        deposit,
        receive,
    }
}

/// Build and send the "Make" instruction. Returns the transaction metadata.
pub fn send_make_ix(
    svm: &mut LiteSVM,
    maker_keypair: &Keypair,
    env: &EscrowEnv,
) -> litesvm::types::TransactionMetadata {
    let maker = maker_keypair.pubkey();

    let make_ix = Instruction {
        program_id: anchor_escrow_q2_2026::id(),
        accounts: anchor_escrow_q2_2026::accounts::Make {
            maker,
            mint_a: env.mint_a,
            mint_b: env.mint_b,
            maker_ata_a: env.maker_ata_a,
            escrow: env.escrow,
            vault: env.vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: anchor_escrow_q2_2026::instruction::Make {
            deposit: env.deposit,
            seed: env.seed,
            receive: env.receive,
        }
        .data(),
    };

    let message = Message::new(&[make_ix], Some(&maker_keypair.pubkey()));
    let recent_blockhash = svm.latest_blockhash();
    let transaction = Transaction::new(&[maker_keypair], message, recent_blockhash);
    svm.send_transaction(transaction).unwrap()
}
