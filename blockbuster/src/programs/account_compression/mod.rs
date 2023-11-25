use log::warn;

use crate::{
    error::BlockbusterError,
    instruction::InstructionBundle,
    program_handler::{ParseResult, ProgramParser},
};

use crate::{program_handler::NotUsed, programs::ProgramParseResult};
use borsh::de::BorshDeserialize;
use plerkle_serialization::AccountInfo;
use solana_sdk::pubkey::Pubkey;
pub use spl_account_compression::events::{
    AccountCompressionEvent::{self, ApplicationData, ChangeLog},
    ApplicationDataEvent, ApplicationDataEventV1, ChangeLogEvent, ChangeLogEventV1,
};

use spl_account_compression::id as program_id;
use spl_noop;

use anchor_lang::Discriminator;
use spl_account_compression::instruction::*;

fn get_instruction_type(full_bytes: &[u8]) -> Instruction {
    let (disc_slice, args_bytes) = full_bytes.split_at(8);
    let disc: [u8; 8] = {
        let mut disc = [0; 8];
        disc.copy_from_slice(&disc_slice);
        disc
    };

    match disc {
        InitEmptyMerkleTree::DISCRIMINATOR => {
            let init_empty_merkle_tree = InitEmptyMerkleTree::try_from_slice(&args_bytes).unwrap();
            Instruction::InitTree {
                max_depth: init_empty_merkle_tree.max_depth,
                max_buffer_size: init_empty_merkle_tree.max_buffer_size,
            }
        }
        ReplaceLeaf::DISCRIMINATOR => {
            let replace_leaf = ReplaceLeaf::try_from_slice(&args_bytes).unwrap();
            Instruction::ReplaceLeaf {
                root: replace_leaf.root,
                previous_leaf: replace_leaf.previous_leaf,
                new_leaf: replace_leaf.new_leaf,
                index: replace_leaf.index,
            }
        }
        TransferAuthority::DISCRIMINATOR => {
            let transfer_authority = TransferAuthority::try_from_slice(&args_bytes).unwrap();
            Instruction::TransferAuthority {
                new_authority: transfer_authority.new_authority,
            }
        }
        VerifyLeaf::DISCRIMINATOR => {
            let verify_leaf = VerifyLeaf::try_from_slice(&args_bytes).unwrap();
            Instruction::VerifyLeaf {
                root: verify_leaf.root,
                leaf: verify_leaf.leaf,
                index: verify_leaf.index,
            }
        }
        Append::DISCRIMINATOR => {
            let append = Append::try_from_slice(&args_bytes).unwrap();
            Instruction::Append { leaf: append.leaf }
        }
        InsertOrAppend::DISCRIMINATOR => {
            let insert_or_append = InsertOrAppend::try_from_slice(&args_bytes).unwrap();
            Instruction::InsertOrAppend {
                root: insert_or_append.root,
                leaf: insert_or_append.leaf,
                index: insert_or_append.index,
            }
        }
        CloseEmptyTree::DISCRIMINATOR => Instruction::CloseTree,
        _ => Instruction::Unknown,
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum Instruction {
    Unknown,
    InitTree {
        max_depth: u32,
        max_buffer_size: u32,
    },
    ReplaceLeaf {
        root: [u8; 32],
        previous_leaf: [u8; 32],
        new_leaf: [u8; 32],
        index: u32,
    },
    TransferAuthority {
        new_authority: Pubkey,
    },
    VerifyLeaf {
        root: [u8; 32],
        leaf: [u8; 32],
        index: u32,
    },
    Append {
        leaf: [u8; 32],
    },
    InsertOrAppend {
        root: [u8; 32],
        leaf: [u8; 32],
        index: u32,
    },
    CloseTree,
}
//TODO add more of the parsing here to minimize program transformer code
pub struct AccountCompressionInstruction {
    pub instruction: Instruction,
    pub tree_update: Option<ChangeLogEventV1>,
    pub leaf_update: Option<ApplicationDataEventV1>,
}

impl AccountCompressionInstruction {
    pub fn new(ix: Instruction) -> Self {
        AccountCompressionInstruction {
            instruction: ix,
            tree_update: None,
            leaf_update: None,
        }
    }
}

impl ParseResult for AccountCompressionInstruction {
    fn result_type(&self) -> ProgramParseResult {
        ProgramParseResult::AccountCompression(self)
    }
    fn result(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

pub struct AccountCompressionParser;

impl ProgramParser for AccountCompressionParser {
    fn key(&self) -> Pubkey {
        program_id()
    }

    fn key_match(&self, key: &Pubkey) -> bool {
        key == &program_id()
    }
    fn handles_account_updates(&self) -> bool {
        false
    }

    fn handles_instructions(&self) -> bool {
        true
    }
    fn handle_account(
        &self,
        _account_info: &AccountInfo,
    ) -> Result<Box<(dyn ParseResult + 'static)>, BlockbusterError> {
        Ok(Box::new(NotUsed::new()))
    }

    fn handle_instruction(
        &self,
        bundle: &InstructionBundle,
    ) -> Result<Box<(dyn ParseResult + 'static)>, BlockbusterError> {
        let InstructionBundle {
            txn_id,
            instruction,
            inner_ix,
            // keys,
            ..
        } = bundle;
        let outer_ix_data = match instruction {
            Some(compiled_ix) if compiled_ix.data().is_some() => {
                let data = compiled_ix.data().unwrap();
                data.iter().collect::<Vec<_>>()
            }
            _ => {
                return Err(BlockbusterError::DeserializationError);
            }
        };
        let ix_type = get_instruction_type(&outer_ix_data);
        let mut b_inst = AccountCompressionInstruction::new(ix_type);
        if let Some(ixs) = inner_ix {
            for ix in ixs {
                if ix.0 .0 == spl_noop::id().to_bytes() {
                    let cix = ix.1;
                    if let Some(inner_ix_data) = cix.data() {
                        let inner_ix_data = inner_ix_data.iter().collect::<Vec<_>>();
                        if !inner_ix_data.is_empty() {
                            match AccountCompressionEvent::try_from_slice(&inner_ix_data) {
                                Ok(result) => match result {
                                    ChangeLog(changelog_event) => {
                                        let ChangeLogEvent::V1(changelog_event) = changelog_event;
                                        b_inst.tree_update = Some(changelog_event);
                                    }
                                    ApplicationData(app_data) => {
                                        let ApplicationDataEvent::V1(app_data) = app_data;
                                        b_inst.leaf_update = Some(app_data);
                                    }
                                },
                                Err(e) => {
                                    warn!(
                                        "Error while deserializing txn {:?} with noop data: {:?}",
                                        txn_id, e
                                    );
                                }
                            }
                        }
                    } else {
                        return Err(BlockbusterError::InstructionParsingError);
                    }
                }
            }
        }

        Ok(Box::new(b_inst))
    }
}
