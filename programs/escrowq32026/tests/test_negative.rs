use {
    anchor_lang::{
        prelude::msg,
        solana_program::{instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID},
        InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::Message,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    let program = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(escrowq32026::id(), program).unwrap();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 2_000_000_000).unwrap();
    (svm, maker)
}

fn send(svm: &mut LiteSVM, signers: &[&Keypair], instructions: Vec<Instruction>) {
    svm.expire_blockhash();
    let message = Message::new(&instructions, Some(&signers[0].pubkey()));
    let transaction = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(transaction).unwrap();
}

fn fails(svm: &mut LiteSVM, signers: &[&Keypair], instructions: Vec<Instruction>) -> bool {
    svm.expire_blockhash();
    let message = Message::new(&instructions, Some(&signers[0].pubkey()));
    let transaction = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(transaction).is_err()
}

fn account<T: InstructionData, A: ToAccountMetas>(
    program_id: Pubkey,
    data: T,
    accounts: A,
) -> Instruction {
    Instruction::new_with_bytes(program_id, &data.data(), accounts.to_account_metas(None))
}

fn escrow_pda(maker: &Pubkey, seed: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &seed.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0
}

fn make_env(
    svm: &mut LiteSVM,
    maker: &Keypair,
    seed: u64,
    deposit: u64,
    receive: u64,
    expiration: i64,
) -> (Pubkey, Pubkey, Pubkey, Pubkey) {
    let maker_key = maker.pubkey();
    let mint_a = CreateMint::new(svm, maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(svm, maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();

    let maker_ata_a = CreateAssociatedTokenAccount::new(svm, maker, &mint_a)
        .owner(&maker_key)
        .send()
        .unwrap();
    let maker_ata_b = CreateAssociatedTokenAccount::new(svm, maker, &mint_b)
        .owner(&maker_key)
        .send()
        .unwrap();

    MintTo::new(svm, maker, &mint_a, &maker_ata_a, 1_000_000_000)
        .send()
        .unwrap();
    MintTo::new(svm, maker, &mint_b, &maker_ata_b, 1_000_000_000)
        .send()
        .unwrap();

    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    send(
        svm,
        &[maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Make {
                seed,
                deposit,
                receive,
                expiration,
            },
            escrowq32026::accounts::Make {
                maker: maker_key,
                mint_a,
                mint_b,
                maker_ata_a,
                escrow,
                vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )],
    );

    (mint_a, mint_b, maker_ata_a, maker_ata_b)
}

#[test]
fn make_rejects_zero_deposit_or_receive() {
    let (mut svm, maker) = setup();
    let maker_key = maker.pubkey();
    let mint_a = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker_key)
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 1_000_000_000)
        .send()
        .unwrap();

    for (seed, deposit, receive) in [(1u64, 0u64, 10_000_000u64), (2u64, 10_000_000u64, 0u64)] {
        let escrow = escrow_pda(&maker_key, seed);
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        assert!(fails(
            &mut svm,
            &[&maker],
            vec![account(
                escrowq32026::id(),
                escrowq32026::instruction::Make {
                    seed,
                    deposit,
                    receive,
                    expiration: 0,
                },
                escrowq32026::accounts::Make {
                    maker: maker_key,
                    mint_a,
                    mint_b,
                    maker_ata_a,
                    escrow,
                    vault,
                    associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                    token_program: TOKEN_PROGRAM_ID,
                    system_program: SYSTEM_PROGRAM_ID,
                },
            )],
        ));
        assert!(
            svm.get_account(&escrow).is_none(),
            "escrow must not be created for invalid amount"
        );
    }
    msg!("make rejected zero deposit and zero receive");
}

#[test]
fn make_rejects_reused_seed() {
    let (mut svm, maker) = setup();
    let seed = 123u64;
    let _ = make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);
    let maker_key = maker.pubkey();
    let mint_a = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker_key)
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 1_000_000_000)
        .send()
        .unwrap();

    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    assert!(fails(
        &mut svm,
        &[&maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Make {
                seed,
                deposit: 5_000_000,
                receive: 5_000_000,
                expiration: 0,
            },
            escrowq32026::accounts::Make {
                maker: maker_key,
                mint_a,
                mint_b,
                maker_ata_a,
                escrow,
                vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )],
    ));
    msg!("make rejected reused seed");
}

#[test]
fn update_rejects_non_maker() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let escrow = escrow_pda(&maker_key, seed);
    make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);

    assert!(fails(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Update {
                receive: 20_000_000,
                expiration: 0,
            },
            escrowq32026::accounts::Update {
                maker: taker.pubkey(),
                escrow,
            },
        )],
    ));
    msg!("update rejected non-maker");
}

#[test]
fn update_rejects_zero_receive() {
    let (mut svm, maker) = setup();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let escrow = escrow_pda(&maker_key, seed);
    make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);

    assert!(fails(
        &mut svm,
        &[&maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Update {
                receive: 0,
                expiration: 0,
            },
            escrowq32026::accounts::Update {
                maker: maker_key,
                escrow,
            },
        )],
    ));
    msg!("update rejected zero receive");
}

#[test]
fn refund_rejects_non_maker() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let (mint_a, _, maker_ata_a, _) = make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);
    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    assert!(fails(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Refund {},
            escrowq32026::accounts::Refund {
                maker: taker.pubkey(),
                mint_a,
                maker_ata_a,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )],
    ));
    msg!("refund rejected non-maker");
}

#[test]
fn take_rejects_insufficient_taker_balance() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let (mint_a, mint_b, _maker_ata_a, maker_ata_b) =
        make_env(&mut svm, &maker, seed, 10_000_000, 20_000_000, 0);
    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, 5_000_000)
        .send()
        .unwrap();

    assert!(fails(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Take {},
            escrowq32026::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_key,
                mint_a,
                mint_b,
                taker_ata_a,
                taker_ata_b,
                maker_ata_b,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
            },
        )],
    ));

    let vault_account = svm.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, 10_000_000, "vault must stay untouched");
    msg!("take rejected insufficient taker balance");
}

#[test]
fn take_rejects_wrong_mint_b() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let (mint_a, _mint_b, _maker_ata_a, _maker_ata_b) =
        make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);
    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    let mint_c = CreateMint::new(&mut svm, &maker)
        .decimals(6)
        .authority(&maker_key)
        .send()
        .unwrap();
    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    let taker_ata_c = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_c)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    let maker_ata_c = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_c)
        .owner(&maker_key)
        .send()
        .unwrap();

    assert!(fails(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Take {},
            escrowq32026::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_key,
                mint_a,
                mint_b: mint_c,
                taker_ata_a,
                taker_ata_b: taker_ata_c,
                maker_ata_b: maker_ata_c,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
            },
        )],
    ));
    msg!("take rejected wrong mint_b");
}

#[test]
fn take_rejects_after_refund() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let maker_key = maker.pubkey();
    let seed = 1u64;
    let (mint_a, mint_b, maker_ata_a, maker_ata_b) =
        make_env(&mut svm, &maker, seed, 10_000_000, 10_000_000, 0);
    let escrow = escrow_pda(&maker_key, seed);
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    send(
        &mut svm,
        &[&maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Refund {},
            escrowq32026::accounts::Refund {
                maker: maker_key,
                mint_a,
                maker_ata_a,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            },
        )],
    );

    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, 10_000_000)
        .send()
        .unwrap();

    assert!(fails(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Take {},
            escrowq32026::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_key,
                mint_a,
                mint_b,
                taker_ata_a,
                taker_ata_b,
                maker_ata_b,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
            },
        )],
    ));
    msg!("take rejected after refund");
}
