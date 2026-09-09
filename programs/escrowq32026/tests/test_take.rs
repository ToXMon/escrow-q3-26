use {
    anchor_lang::{
        prelude::msg,
        solana_program::{instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID},
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

fn account(
    program_id: Pubkey,
    data: impl InstructionData,
    accounts: impl ToAccountMetas,
) -> Instruction {
    Instruction::new_with_bytes(program_id, &data.data(), accounts.to_account_metas(None))
}

#[test]
fn update_then_take_transfers_both_sides_and_closes_accounts() {
    let (mut svm, maker) = setup();
    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();

    let maker_key = maker.pubkey();
    let taker_key = taker.pubkey();
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
    let maker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&maker_key)
        .send()
        .unwrap();
    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&taker_key)
        .send()
        .unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&taker_key)
        .send()
        .unwrap();

    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 10_000_000)
        .send()
        .unwrap();
    MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, 20_000_000)
        .send()
        .unwrap();

    let seed = 77u64;
    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker_key.as_ref(), &seed.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    send(
        &mut svm,
        &[&maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Make {
                seed,
                deposit: 10_000_000,
                receive: 10_000_000,
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
    );

    send(
        &mut svm,
        &[&maker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Update {
                receive: 20_000_000,
                expiration: 0,
            },
            escrowq32026::accounts::Update {
                maker: maker_key,
                escrow,
            },
        )],
    );

    let escrow_account = svm.get_account(&escrow).unwrap();
    let escrow_state =
        escrowq32026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
    assert_eq!(escrow_state.receive, 20_000_000);

    send(
        &mut svm,
        &[&taker],
        vec![account(
            escrowq32026::id(),
            escrowq32026::instruction::Take {},
            escrowq32026::accounts::Take {
                taker: taker_key,
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
    );

    let taker_a =
        spl_token::state::Account::unpack(&svm.get_account(&taker_ata_a).unwrap().data).unwrap();
    let maker_b =
        spl_token::state::Account::unpack(&svm.get_account(&maker_ata_b).unwrap().data).unwrap();
    assert_eq!(taker_a.amount, 10_000_000);
    assert_eq!(maker_b.amount, 20_000_000);
    assert!(svm.get_account(&escrow).is_none());
    assert!(svm.get_account(&vault).is_none());
    msg!("update and take verified");
}
