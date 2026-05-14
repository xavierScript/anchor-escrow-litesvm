mod helpers;

use anchor_lang::{
    prelude::msg,
    solana_program::{instruction::Instruction, program_pack::Pack},
    system_program::ID as SYSTEM_PROGRAM_ID,
    InstructionData, ToAccountMetas,
};
use anchor_spl::{
    associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
    token::spl_token,
};
use litesvm_token::{spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, MintTo};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

use helpers::{create_escrow_env, send_make_ix, setup};

#[test]
fn test_take() {
    let (mut svm, maker_keypair) = setup();
    let maker = maker_keypair.pubkey();

    // Create a separate taker keypair and fund it
    let taker_keypair = Keypair::new();
    svm.airdrop(&taker_keypair.pubkey(), 1_000_000_000)
        .unwrap();
    let taker = taker_keypair.pubkey();

    // Create the escrow environment and execute Make
    let env = create_escrow_env(&mut svm, &maker_keypair, 123, 10, 10, 1_000_000_000);
    send_make_ix(&mut svm, &maker_keypair, &env);
    msg!("\n\nMake transaction sucessfull (before take)");

    // Create taker's ATA for Mint B and fund it
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker_keypair, &env.mint_b)
        .owner(&taker)
        .send()
        .unwrap();

    MintTo::new(
        &mut svm,
        &maker_keypair,
        &env.mint_b,
        &taker_ata_b,
        1_000_000_000,
    )
    .send()
    .unwrap();

    // Derive the ATAs that the Take instruction will init_if_needed
    let taker_ata_a = associated_token::get_associated_token_address(&taker, &env.mint_a);
    let maker_ata_b = associated_token::get_associated_token_address(&maker, &env.mint_b);

    // Build and send the Take instruction
    let take_ix = Instruction {
        program_id: anchor_escrow_q2_2026::id(),
        accounts: anchor_escrow_q2_2026::accounts::Take {
            taker,
            maker,
            mint_a: env.mint_a,
            mint_b: env.mint_b,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            escrow: env.escrow,
            vault: env.vault,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: anchor_escrow_q2_2026::instruction::Take {}.data(),
    };

    let message = Message::new(&[take_ix], Some(&taker_keypair.pubkey()));
    let recent_blockhash = svm.latest_blockhash();
    let transaction = Transaction::new(&[&taker_keypair], message, recent_blockhash);
    let tx = svm.send_transaction(transaction).unwrap();

    msg!("\n\nTake transaction sucessfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify taker received 10 of Mint A
    let taker_ata_a_account = svm.get_account(&taker_ata_a).unwrap();
    let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
    assert_eq!(taker_ata_a_data.amount, 10);
    assert_eq!(taker_ata_a_data.owner, taker);
    assert_eq!(taker_ata_a_data.mint, env.mint_a);

    // Verify maker received 10 of Mint B
    let maker_ata_b_account = svm.get_account(&maker_ata_b).unwrap();
    let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
    assert_eq!(maker_ata_b_data.amount, 10);
    assert_eq!(maker_ata_b_data.owner, maker);
    assert_eq!(maker_ata_b_data.mint, env.mint_b);

    // Verify vault and escrow are closed
    assert!(
        svm.get_account(&env.vault).is_none(),
        "Vault should be closed after take"
    );
    assert!(
        svm.get_account(&env.escrow).is_none(),
        "Escrow should be closed after take"
    );
}
