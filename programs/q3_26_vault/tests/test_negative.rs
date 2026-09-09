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

fn fails(svm: &mut LiteSVM, payer: &Keypair, ix: Instruction) -> bool {
    svm.expire_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &svm.latest_blockhash());
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).is_err()
}

fn ix<T: InstructionData, A: ToAccountMetas>(
    program_id: Pubkey,
    data: T,
    accounts: A,
) -> Instruction {
    Instruction::new_with_bytes(program_id, &data.data(), accounts.to_account_metas(None))
}

fn init_env(svm: &mut LiteSVM, user: &Keypair) -> (Pubkey, Pubkey) {
    let program_id = q3_26_vault::id();
    let state = vault_state(&user.pubkey());
    let vault = vault_addr(&user.pubkey());
    svm.airdrop(&user.pubkey(), 2_000_000_000).unwrap();
    send(
        svm,
        user,
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
    (state, vault)
}

fn vault_state(user: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[STATE, user.as_ref()], &q3_26_vault::id()).0
}

fn vault_addr(user: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[VAULT_SEED, user.as_ref()], &q3_26_vault::id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let program_id = q3_26_vault::id();
    let user = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/q3_26_vault.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    (svm, user)
}

#[test]
fn deposit_rejects_zero() {
    let (mut svm, user) = setup();
    let (state, vault) = init_env(&mut svm, &user);

    assert!(fails(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Deposit { amount: 0 },
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn withdraw_rejects_overdraw() {
    let (mut svm, user) = setup();
    let (state, vault) = init_env(&mut svm, &user);

    send(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Deposit {
                amount: 500_000_000,
            },
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );

    assert!(fails(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Withdraw {
                amount: 1_000_000_000
            },
            q3_26_vault::accounts::Withdraw {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn withdraw_rejects_zero() {
    let (mut svm, user) = setup();
    let (state, vault) = init_env(&mut svm, &user);

    send(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Deposit {
                amount: 500_000_000,
            },
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );

    assert!(fails(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Withdraw { amount: 0 },
            q3_26_vault::accounts::Withdraw {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn initialize_rejects_reuse() {
    let (mut svm, user) = setup();
    let (state, vault) = init_env(&mut svm, &user);

    assert!(fails(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Initialize {},
            q3_26_vault::accounts::Initialize {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn close_rejects_wrong_user() {
    let (mut svm, user) = setup();
    let attacker = Keypair::new();
    let (state, vault) = init_env(&mut svm, &user);
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    assert!(fails(
        &mut svm,
        &attacker,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Close {},
            q3_26_vault::accounts::Close {
                user: attacker.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn withdraw_rejects_wrong_user() {
    let (mut svm, user) = setup();
    let attacker = Keypair::new();
    let (state, vault) = init_env(&mut svm, &user);
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    send(
        &mut svm,
        &user,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Deposit {
                amount: 500_000_000,
            },
            q3_26_vault::accounts::Deposit {
                user: user.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    );

    assert!(fails(
        &mut svm,
        &attacker,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Withdraw {
                amount: 100_000_000
            },
            q3_26_vault::accounts::Withdraw {
                user: attacker.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}

#[test]
fn deposit_rejects_wrong_user() {
    let (mut svm, user) = setup();
    let attacker = Keypair::new();
    let (state, vault) = init_env(&mut svm, &user);
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    assert!(fails(
        &mut svm,
        &attacker,
        ix(
            q3_26_vault::id(),
            q3_26_vault::instruction::Deposit {
                amount: 100_000_000
            },
            q3_26_vault::accounts::Deposit {
                user: attacker.pubkey(),
                vault_state: state,
                vault,
                system_program: system_program::ID,
            },
        ),
    ));
}
