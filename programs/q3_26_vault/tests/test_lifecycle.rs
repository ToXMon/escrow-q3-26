use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    q3_26_vault::{STATE, VAULT_SEED},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn send(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) {
    svm.expire_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap();
}

fn ix<T: InstructionData, A: ToAccountMetas>(
    program_id: Pubkey,
    data: T,
    accounts: A,
) -> Instruction {
    Instruction::new_with_bytes(program_id, &data.data(), accounts.to_account_metas(None))
}

#[test]
fn vault_initialize_deposit_withdraw_close() {
    let program_id = q3_26_vault::id();
    let user = Keypair::new();
    let (state, _) = Pubkey::find_program_address(&[STATE, user.pubkey().as_ref()], &program_id);
    let (vault, _) =
        Pubkey::find_program_address(&[VAULT_SEED, user.pubkey().as_ref()], &program_id);
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/q3_26_vault.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&user.pubkey(), 2_000_000_000).unwrap();

    send(
        &mut svm,
        &user,
        ix(
            program_id,
            q3_26_vault::instruction::Initialize {},
            q3_26_vault::accounts::Initialize {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );
    let rent = svm.minimum_balance_for_rent_exemption(0);
    assert_eq!(svm.get_balance(&vault).unwrap(), rent);

    let deposit = 500_000_000;
    send(
        &mut svm,
        &user,
        ix(
            program_id,
            q3_26_vault::instruction::Deposit { amount: deposit },
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );
    assert_eq!(svm.get_balance(&vault).unwrap(), rent + deposit);

    send(
        &mut svm,
        &user,
        ix(
            program_id,
            q3_26_vault::instruction::Withdraw {
                amount: 100_000_000,
            },
            q3_26_vault::accounts::Withdraw {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );
    assert_eq!(
        svm.get_balance(&vault).unwrap(),
        rent + deposit - 100_000_000
    );

    let close = svm.send_transaction({
        let ix = ix(
            program_id,
            q3_26_vault::instruction::Close {},
            q3_26_vault::accounts::Close {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        );
        let msg = Message::new_with_blockhash(&[ix], Some(&user.pubkey()), &svm.latest_blockhash());
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap()
    });
    assert!(close.is_err(), "non-empty vault must not close");

    send(
        &mut svm,
        &user,
        ix(
            program_id,
            q3_26_vault::instruction::Withdraw {
                amount: deposit - 100_000_000,
            },
            q3_26_vault::accounts::Withdraw {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );
    send(
        &mut svm,
        &user,
        ix(
            program_id,
            q3_26_vault::instruction::Close {},
            q3_26_vault::accounts::Close {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );
    assert!(svm.get_account(&state).is_none());
}
