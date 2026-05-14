mod helpers;

use anchor_lang::{prelude::msg, solana_program::program_pack::Pack, AccountDeserialize};
use anchor_spl::token::spl_token;
use solana_signer::Signer;

use helpers::{create_escrow_env, send_make_ix, setup};

#[test]
fn test_make() {
    let (mut svm, maker_keypair) = setup();
    let maker = maker_keypair.pubkey();

    // Create the escrow environment: mints, ATAs, PDAs, and fund the maker
    let env = create_escrow_env(&mut svm, &maker_keypair, 123, 10, 10, 1_000_000_000);

    // Send the Make instruction
    let tx = send_make_ix(&mut svm, &maker_keypair, &env);

    msg!("\n\nMake transaction sucessfull");
    msg!("CUs Consumed: {}", tx.compute_units_consumed);
    msg!("Tx Signature: {}", tx.signature);

    // Verify the vault received the deposited tokens
    let vault_account = svm.get_account(&env.vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10);
    assert_eq!(vault_data.owner, env.escrow);
    assert_eq!(vault_data.mint, env.mint_a);

    // Verify the escrow account was initialized correctly
    let escrow_account = svm.get_account(&env.escrow).unwrap();
    let escrow_data = anchor_escrow_q2_2026::state::Escrow::try_deserialize(
        &mut escrow_account.data.as_ref(),
    )
    .unwrap();
    assert_eq!(escrow_data.seed, 123u64);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, env.mint_a);
    assert_eq!(escrow_data.mint_b, env.mint_b);
    assert_eq!(escrow_data.receive, 10);
}
