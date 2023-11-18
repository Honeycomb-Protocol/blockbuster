use account_compression::AccountCompressionInstruction;
use bubblegum::BubblegumInstruction;
use candy_guard::CandyGuardAccountData;
use candy_machine::CandyMachineAccountData;
use candy_machine_core::CandyMachineCoreAccountData;
use token_account::TokenProgramAccount;
use token_metadata::TokenMetadataAccountState;

pub mod account_compression;
pub mod bubblegum;
pub mod candy_guard;
pub mod candy_machine;
pub mod candy_machine_core;
pub mod token_account;
pub mod token_metadata;

pub enum ProgramParseResult<'a> {
    Unknown,
    Bubblegum(&'a BubblegumInstruction),
    TokenMetadata(&'a TokenMetadataAccountState),
    TokenProgramAccount(&'a TokenProgramAccount),
    CandyGuard(&'a CandyGuardAccountData),
    CandyMachine(&'a CandyMachineAccountData),
    CandyMachineCore(&'a CandyMachineCoreAccountData),
    AccountCompression(&'a AccountCompressionInstruction),
}
