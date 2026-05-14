mod helpers;

use anchor_lang::{
    prelude::msg,
    solana_program::{instruction::Instruction, program_pack::Pack},
    system_program::ID as SYSTEM_PROGRAM_ID,
    InstructionData, ToAccountMetas,
};
use anchor_spl::token::spl_token;
use litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

use helpers::{create_escrow_env, send_make_ix, setup};

#[test]
fn test_refund() {
    let (mut svm, maker_keypair) = setup();
    let maker = maker_keypair.pubkey();

    // Create the escrow environment and execute Make
    let env = create_escrow_env(&mut svm, &maker_keypair, 123, 10, 10, 1_000_000_000);

    // Record maker's balance before Make
    let maker_ata_before = svm.get_account(&env.maker_ata_a).unwrap();
    let maker_balance_before =
        spl_token::state::Account::unpack(&maker_ata_before.data).unwrap().amount;
    msg!("Maker balance before make: {}", maker_balance_before);

    send_make_ix(&mut svm, &maker_keypair, &env);
    msg!("\n\nMake transaction sucessfull (before refund)");

    // Verify vault has the deposited tokens
    let vault_account = svm.get_account(&env.vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10);

    // Build and send the Refund instruction
    let refund_ix = Instruction {
        program_id: anchor_escrow_q2_2026::id(),
        accounts: anchor_escrow_q2_2026::accounts::Refund {
            maker,
            mint_a: env.mint_a,
            maker_ata_a: env.maker_ata_a,
            escrow: env.escrow,
            vault: env.vault,
            token_program: TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
        data: anchor_escrow_q2_2026::instruction::Refund {}.data(),
    };

    let message = Message::new(&[refund_ix], Some(&maker_keypair.pubkey()));
    let recent_blockhash = svm.latest_blockhash();
    let transaction = Transaction::new(&[&maker_keypair], message, recent_blockhash);
    let tx = svm.send_transaction(transaction).unwrap();

    msg!("\n\nRefund transaction sucessfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify maker got their tokens back
    let maker_ata_after = svm.get_account(&env.maker_ata_a).unwrap();
    let maker_balance_after =
        spl_token::state::Account::unpack(&maker_ata_after.data).unwrap().amount;
    assert_eq!(maker_balance_after, maker_balance_before);

    // Verify vault and escrow are closed
    assert!(
        svm.get_account(&env.vault).is_none(),
        "Vault should be closed after refund"
    );
    assert!(
        svm.get_account(&env.escrow).is_none(),
        "Escrow should be closed after refund"
    );
}
